use argon2::Argon2;
use bip39::Mnemonic;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use hmac::Hmac;
use sha2::Sha512;
use std::fmt;
use std::path::PathBuf;
use zeroize::Zeroize;

const WALLET_VERSION: u8 = 0x01;
const WALLET_FILE_LEN: usize = 93; // 1 + 32 + 12 + 48
const SALT_LEN: usize = 32;
const NONCE_LEN: usize = 12;

#[derive(Debug)]
pub enum WalletError {
    WrongPassword,
    CorruptFile,
    Io(std::io::Error),
}

impl fmt::Display for WalletError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongPassword => write!(f, "Wrong password"),
            Self::CorruptFile => write!(f, "Wallet file is corrupt"),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for WalletError {}

impl From<std::io::Error> for WalletError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

fn wallet_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".samp")
        .join("wallets")
}

pub fn wallet_path(name: &str) -> PathBuf {
    wallet_dir().join(format!("{name}.key"))
}

pub fn wallet_exists(name: &str) -> bool {
    wallet_path(name).exists()
}

pub fn list_wallets() -> Vec<String> {
    let dir = wallet_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.strip_suffix(".key").map(|n| n.to_string())
        })
        .collect();
    names.sort();
    names
}

// ---------------------------------------------------------------------------
// Mnemonic / seed derivation (Substrate-compatible)
// ---------------------------------------------------------------------------

pub fn generate_mnemonic() -> Mnemonic {
    let mut entropy = [0u8; 16];
    getrandom::fill(&mut entropy).expect("getrandom");
    let mnemonic = Mnemonic::from_entropy(&entropy).expect("mnemonic from entropy");
    entropy.zeroize();
    mnemonic
}

pub fn parse_mnemonic(phrase: &str) -> Result<Mnemonic, String> {
    Mnemonic::parse_normalized(phrase).map_err(|e| format!("Invalid mnemonic: {e}"))
}

pub fn seed_from_mnemonic(mnemonic: &Mnemonic) -> [u8; 32] {
    let mut entropy = mnemonic.to_entropy();
    let mut seed = [0u8; 64];
    pbkdf2::pbkdf2::<Hmac<Sha512>>(&entropy, b"mnemonic", 2048, &mut seed).expect("pbkdf2");
    entropy.zeroize();
    let mut mini_secret = [0u8; 32];
    mini_secret.copy_from_slice(&seed[..32]);
    seed.zeroize();
    mini_secret
}

pub fn seed_from_hex(hex_str: &str) -> Result<[u8; 32], String> {
    let bytes =
        hex::decode(hex_str.trim_start_matches("0x")).map_err(|e| format!("Invalid hex: {e}"))?;
    bytes
        .try_into()
        .map_err(|_| "Seed must be 32 bytes (64 hex chars)".to_string())
}

// ---------------------------------------------------------------------------
// Wallet file encryption
// ---------------------------------------------------------------------------

fn derive_key(password: &str, salt: &[u8; SALT_LEN]) -> [u8; 32] {
    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(65536, 3, 1, Some(32)).expect("constant argon2 params"),
    );
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .expect("argon2 hash");
    key
}

pub fn create(name: &str, password: &str, seed: &[u8; 32]) -> Result<(), WalletError> {
    create_at(&wallet_path(name), password, seed)
}

pub fn create_at(
    path: &std::path::Path,
    password: &str,
    seed: &[u8; 32],
) -> Result<(), WalletError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
        }
    }

    let mut salt = [0u8; SALT_LEN];
    getrandom::fill(&mut salt).expect("getrandom");
    let mut nonce_bytes = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce_bytes).expect("getrandom");

    let mut key = derive_key(password, &salt);
    let cipher = ChaCha20Poly1305::new((&key).into());
    key.zeroize();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, seed.as_slice()).expect("encryption");

    let mut file_data = Vec::with_capacity(WALLET_FILE_LEN);
    file_data.push(WALLET_VERSION);
    file_data.extend_from_slice(&salt);
    file_data.extend_from_slice(&nonce_bytes);
    file_data.extend_from_slice(&ciphertext);

    std::fs::write(path, &file_data)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn open(name: &str, password: &str) -> Result<zeroize::Zeroizing<[u8; 32]>, WalletError> {
    open_at(&wallet_path(name), password)
}

pub fn open_at(
    path: &std::path::Path,
    password: &str,
) -> Result<zeroize::Zeroizing<[u8; 32]>, WalletError> {
    let data = std::fs::read(path)?;
    if data.len() != WALLET_FILE_LEN || data[0] != WALLET_VERSION {
        return Err(WalletError::CorruptFile);
    }

    let salt: [u8; SALT_LEN] = data[1..33]
        .try_into()
        .map_err(|_| WalletError::CorruptFile)?;
    let nonce_bytes: [u8; NONCE_LEN] = data[33..45]
        .try_into()
        .map_err(|_| WalletError::CorruptFile)?;
    let ciphertext = &data[45..];

    let mut key = derive_key(password, &salt);
    let cipher = ChaCha20Poly1305::new((&key).into());
    key.zeroize();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let mut plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| WalletError::WrongPassword)?;

    let mut seed = [0u8; 32];
    seed.copy_from_slice(&plaintext);
    plaintext.zeroize();
    Ok(zeroize::Zeroizing::new(seed))
}
