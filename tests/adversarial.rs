use taolk::error::MetadataError;
use taolk::metadata::Metadata;
use taolk::secret::{Password, Phrase, PhraseError, Seed};
use taolk::wallet;
use tempfile::TempDir;

fn pw(s: &str) -> Password {
    Password::new(s.into())
}

fn wallet_path(dir: &TempDir, name: &str) -> std::path::PathBuf {
    dir.path().join(format!("{name}.key"))
}

fn make_wallet(dir: &TempDir, name: &str, password: &str) -> std::path::PathBuf {
    let path = wallet_path(dir, name);
    let seed = Seed::from_bytes([0xAB; 32]);
    wallet::create_at(&path, &pw(password), &seed).unwrap();
    path
}

#[test]
fn wrong_password_returns_wrong_password_variant() {
    let dir = TempDir::new().unwrap();
    let path = make_wallet(&dir, "alice", "correct");
    let result = wallet::open_at(&path, &pw("wrong"));
    assert!(matches!(result, Err(wallet::WalletError::WrongPassword)));
}

#[test]
fn ten_consecutive_wrong_passwords_all_return_same_variant() {
    let dir = TempDir::new().unwrap();
    let path = make_wallet(&dir, "alice", "correct");
    for i in 0..10 {
        let attempt = format!("wrong{i}");
        match wallet::open_at(&path, &pw(&attempt)) {
            Err(wallet::WalletError::WrongPassword) => {}
            Err(e) => panic!("attempt {i}: expected WrongPassword, got {e:?}"),
            Ok(_) => panic!("attempt {i}: expected error, got Ok"),
        }
    }
}

#[test]
fn correct_password_still_works_after_failed_attempts() {
    let dir = TempDir::new().unwrap();
    let path = make_wallet(&dir, "alice", "correct");
    for _ in 0..5 {
        let _ = wallet::open_at(&path, &pw("wrong"));
    }
    let opened = wallet::open_at(&path, &pw("correct"));
    assert!(opened.is_ok());
}

#[test]
fn tampered_wallet_byte_returns_corrupt_file_or_wrong_password() {
    let dir = TempDir::new().unwrap();
    let path = make_wallet(&dir, "alice", "pw");
    let mut bytes = std::fs::read(&path).unwrap();
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0xFF;
    std::fs::write(&path, &bytes).unwrap();

    // A flipped ciphertext byte fails AEAD verification → WrongPassword (decrypt fails).
    // A flipped header byte → CorruptFile.
    match wallet::open_at(&path, &pw("pw")) {
        Err(wallet::WalletError::WrongPassword) | Err(wallet::WalletError::CorruptFile) => {}
        Err(e) => panic!("expected WrongPassword or CorruptFile, got {e:?}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn truncated_wallet_returns_corrupt_file() {
    let dir = TempDir::new().unwrap();
    let path = make_wallet(&dir, "alice", "pw");
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, &bytes[..50]).unwrap();
    let result = wallet::open_at(&path, &pw("pw"));
    assert!(matches!(result, Err(wallet::WalletError::CorruptFile)));
}

#[test]
fn empty_wallet_file_returns_corrupt_file() {
    let dir = TempDir::new().unwrap();
    let path = wallet_path(&dir, "empty");
    std::fs::write(&path, b"").unwrap();
    let result = wallet::open_at(&path, &pw("anything"));
    assert!(matches!(result, Err(wallet::WalletError::CorruptFile)));
}

#[test]
fn ten_kb_password_succeeds() {
    let dir = TempDir::new().unwrap();
    let path = wallet_path(&dir, "longpw");
    let big = "x".repeat(10_240);
    let seed = Seed::from_bytes([0xCD; 32]);
    wallet::create_at(&path, &pw(&big), &seed).unwrap();
    let opened = wallet::open_at(&path, &pw(&big));
    assert!(opened.is_ok());
}

#[test]
fn phrase_parse_eleven_words_rejected() {
    let eleven =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon";
    assert!(matches!(
        Phrase::parse(eleven),
        Err(PhraseError::Invalid(_))
    ));
}

#[test]
fn phrase_parse_thirteen_words_rejected() {
    let thirteen = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon";
    assert!(matches!(
        Phrase::parse(thirteen),
        Err(PhraseError::Invalid(_))
    ));
}

#[test]
fn phrase_parse_invalid_word_rejected() {
    let bogus = "zzzzzz abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    assert!(matches!(Phrase::parse(bogus), Err(PhraseError::Invalid(_))));
}

#[test]
fn phrase_parse_empty_rejected() {
    assert!(Phrase::parse("").is_err());
}

#[test]
fn metadata_garbage_bytes_returns_typed_error() {
    let garbage = vec![0xFFu8; 256];
    let err = Metadata::from_runtime_metadata(&garbage).unwrap_err();
    assert!(matches!(err, MetadataError::Scale(_)));
}

#[test]
fn metadata_only_magic_bytes_returns_typed_error() {
    let magic_only = vec![0x6du8, 0x65, 0x74, 0x61];
    let err = Metadata::from_runtime_metadata(&magic_only).unwrap_err();
    assert!(matches!(err, MetadataError::Scale(_)));
}

#[test]
fn metadata_magic_plus_v15_returns_version_error() {
    let mut bytes = vec![0x6du8, 0x65, 0x74, 0x61];
    bytes.push(15);
    let err = Metadata::from_runtime_metadata(&bytes).unwrap_err();
    let s = err.to_string();
    assert!(s.contains("version"));
    assert!(s.contains("15"));
}

#[test]
fn seed_from_hex_with_unicode_garbage_rejected() {
    let bytes = "🦀".repeat(32);
    assert!(Seed::from_hex(&bytes).is_err());
}

#[test]
fn seed_from_hex_with_whitespace_inside_rejected() {
    let s = format!("{} {}", "ab".repeat(16), "ab".repeat(16));
    assert!(Seed::from_hex(&s).is_err());
}
