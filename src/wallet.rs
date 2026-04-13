use argon2::Argon2;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use std::path::PathBuf;
use zeroize::Zeroize;

pub use crate::error::WalletError;
use crate::secret::{Password, Seed};

const WALLET_VERSION: u8 = 0x01;
const WALLET_FILE_LEN: usize = 93;
const SALT_LEN: usize = 32;
const NONCE_LEN: usize = 12;

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

fn derive_key(password: &Password, salt: &[u8; SALT_LEN]) -> [u8; 32] {
    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(65536, 3, 1, Some(32)).expect("constant argon2 params"),
    );
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(password.as_str().as_bytes(), salt, &mut key)
        .expect("argon2 hash");
    key
}

#[must_use = "create returns Err if the wallet file cannot be written"]
pub fn create(name: &str, password: &Password, seed: &Seed) -> Result<(), WalletError> {
    create_at(&wallet_path(name), password, seed)
}

pub fn create_at(
    path: &std::path::Path,
    password: &Password,
    seed: &Seed,
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
    getrandom::fill(&mut salt).expect("OS RNG");
    let mut nonce_bytes = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce_bytes).expect("OS RNG");

    let mut key = derive_key(password, &salt);
    let cipher = ChaCha20Poly1305::new((&key).into());
    key.zeroize();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, seed.as_bytes().as_slice())
        .expect("encrypt");

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

#[must_use = "the returned Seed must be consumed"]
pub fn open(name: &str, password: &Password) -> Result<Seed, WalletError> {
    open_at(&wallet_path(name), password)
}

pub fn open_at(path: &std::path::Path, password: &Password) -> Result<Seed, WalletError> {
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

    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&plaintext);
    plaintext.zeroize();
    Ok(Seed::from_bytes(bytes))
}
