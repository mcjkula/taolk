use bip39::Mnemonic;
use hmac::Hmac;
use schnorrkel::keys::{ExpansionMode, MiniSecretKey};
use sha2::Sha512;
use zeroize::{Zeroize, Zeroizing};

use crate::types::Pubkey;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SeedError {
    #[error("invalid hex: {0}")]
    InvalidHex(#[from] hex::FromHexError),
    #[error("seed must be 32 bytes (64 hex chars), got {0}")]
    WrongLength(usize),
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PhraseError {
    #[error("invalid mnemonic: {0}")]
    Invalid(String),
    #[error("entropy generation failed: {0}")]
    Entropy(#[from] getrandom::Error),
}

pub struct Seed(Zeroizing<[u8; 32]>);

pub struct Password(Zeroizing<String>);

pub struct Phrase(Zeroizing<String>);

pub struct SigningKey {
    inner: schnorrkel::Keypair,
}

impl Seed {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    pub fn from_phrase(phrase: &Phrase) -> Self {
        let mnemonic = Mnemonic::parse_normalized(&phrase.0)
            .expect("Phrase only constructed via validated paths");
        let mut entropy = mnemonic.to_entropy();
        let mut pbkdf_out = Zeroizing::new([0u8; 64]);
        pbkdf2::pbkdf2::<Hmac<Sha512>>(&entropy, b"mnemonic", 2048, pbkdf_out.as_mut())
            .expect("pbkdf2");
        entropy.zeroize();
        let mut mini_secret = [0u8; 32];
        mini_secret.copy_from_slice(&pbkdf_out[..32]);
        Self(Zeroizing::new(mini_secret))
    }

    pub fn from_hex(hex_str: &str) -> Result<Self, SeedError> {
        let bytes = hex::decode(hex_str.trim_start_matches("0x"))?;
        let len = bytes.len();
        let arr: [u8; 32] = bytes.try_into().map_err(|_| SeedError::WrongLength(len))?;
        Ok(Self(Zeroizing::new(arr)))
    }

    pub fn derive_signing_key(&self) -> SigningKey {
        let msk = MiniSecretKey::from_bytes(self.0.as_ref()).expect("32-byte seed");
        SigningKey {
            inner: msk.expand_to_keypair(ExpansionMode::Ed25519),
        }
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn ct_eq(&self, other: &Self) -> bool {
        let mut diff = 0u8;
        for i in 0..32 {
            diff |= self.0[i] ^ other.0[i];
        }
        diff == 0
    }
}

impl Password {
    pub fn new(s: String) -> Self {
        Self(Zeroizing::new(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Phrase {
    pub fn generate() -> Result<Self, PhraseError> {
        let mut entropy = Zeroizing::new([0u8; 16]);
        getrandom::fill(entropy.as_mut())?;
        let mnemonic = Mnemonic::from_entropy(entropy.as_ref()).expect("16-byte entropy");
        Ok(Self(Zeroizing::new(mnemonic.to_string())))
    }

    pub fn parse(s: &str) -> Result<Self, PhraseError> {
        let mnemonic =
            Mnemonic::parse_normalized(s).map_err(|e| PhraseError::Invalid(e.to_string()))?;
        Ok(Self(Zeroizing::new(mnemonic.to_string())))
    }

    pub fn words(&self) -> &str {
        &self.0
    }
}

impl SigningKey {
    pub fn sign(&self, msg: &[u8]) -> [u8; 64] {
        let context = schnorrkel::signing_context(b"substrate");
        self.inner.sign(context.bytes(msg)).to_bytes()
    }

    pub fn public_key(&self) -> Pubkey {
        Pubkey(self.inner.public.to_bytes())
    }

    pub fn keypair(&self) -> &schnorrkel::Keypair {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_from_bytes_round_trip() {
        let bytes = [0xAA; 32];
        let seed = Seed::from_bytes(bytes);
        assert_eq!(seed.as_bytes(), &bytes);
    }

    #[test]
    fn seed_from_hex_with_prefix() {
        let hex = "0x".to_string() + &"aa".repeat(32);
        let seed = Seed::from_hex(&hex).unwrap();
        assert_eq!(seed.as_bytes(), &[0xAA; 32]);
    }

    #[test]
    fn seed_from_hex_without_prefix() {
        let seed = Seed::from_hex(&"bb".repeat(32)).unwrap();
        assert_eq!(seed.as_bytes(), &[0xBB; 32]);
    }

    #[test]
    fn seed_from_hex_wrong_length() {
        assert!(matches!(
            Seed::from_hex("aabbcc"),
            Err(SeedError::WrongLength(3))
        ));
    }

    #[test]
    fn seed_from_hex_invalid_chars() {
        assert!(matches!(
            Seed::from_hex(&"zz".repeat(32)),
            Err(SeedError::InvalidHex(_))
        ));
    }

    #[test]
    fn phrase_generate_is_valid_mnemonic() {
        let phrase = Phrase::generate().unwrap();
        assert_eq!(phrase.words().split_whitespace().count(), 12);
        let _ = Phrase::parse(phrase.words()).unwrap();
    }

    #[test]
    fn phrase_parse_rejects_garbage() {
        assert!(Phrase::parse("not a real mnemonic phrase at all here please").is_err());
    }

    #[test]
    fn seed_from_phrase_deterministic() {
        let phrase = Phrase::parse(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        )
        .unwrap();
        let s1 = Seed::from_phrase(&phrase);
        let s2 = Seed::from_phrase(&phrase);
        assert_eq!(s1.as_bytes(), s2.as_bytes());
    }

    #[test]
    fn signing_key_public_key_stable() {
        let seed = Seed::from_bytes([0x42; 32]);
        let sk1 = seed.derive_signing_key();
        let sk2 = seed.derive_signing_key();
        assert_eq!(sk1.public_key(), sk2.public_key());
    }

    #[test]
    fn signing_key_signs_and_verifies() {
        let seed = Seed::from_bytes([0x33; 32]);
        let sk = seed.derive_signing_key();
        let msg = b"hello taolk";
        let sig_bytes = sk.sign(msg);
        let sig = schnorrkel::Signature::from_bytes(&sig_bytes).unwrap();
        let context = schnorrkel::signing_context(b"substrate");
        assert!(sk.keypair().public.verify(context.bytes(msg), &sig).is_ok());
    }

    #[test]
    fn password_round_trip() {
        let p = Password::new("hunter2".to_string());
        assert_eq!(p.as_str(), "hunter2");
    }
}
