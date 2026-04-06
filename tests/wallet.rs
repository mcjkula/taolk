use taolk::wallet;
use tempfile::TempDir;

fn wallet_path(dir: &TempDir, name: &str) -> std::path::PathBuf {
    dir.path().join(format!("{name}.key"))
}

#[test]
fn create_and_open_roundtrip() {
    let dir = TempDir::new().unwrap();
    let path = wallet_path(&dir, "test");
    let seed = [0xAA; 32];
    wallet::create_at(&path, "testpass", &seed).unwrap();
    let opened = wallet::open_at(&path, "testpass").unwrap();
    assert_eq!(*opened, seed);
}

#[test]
fn wrong_password_rejected() {
    let dir = TempDir::new().unwrap();
    let path = wallet_path(&dir, "test");
    wallet::create_at(&path, "testpass", &[0xAA; 32]).unwrap();
    let err = wallet::open_at(&path, "wrongpass").unwrap_err();
    assert!(matches!(err, wallet::WalletError::WrongPassword));
}

#[test]
fn corrupt_file_detected() {
    let dir = TempDir::new().unwrap();
    let path = wallet_path(&dir, "corrupt");
    std::fs::write(&path, b"garbage bytes here").unwrap();
    let err = wallet::open_at(&path, "testpass").unwrap_err();
    assert!(matches!(err, wallet::WalletError::CorruptFile));
}

#[test]
fn seed_from_mnemonic_deterministic() {
    let mnemonic = wallet::generate_mnemonic();
    let seed_a = wallet::seed_from_mnemonic(&mnemonic);
    let seed_b = wallet::seed_from_mnemonic(&mnemonic);
    assert_eq!(seed_a, seed_b);
}

#[test]
fn seed_from_mnemonic_different_phrases() {
    let m1 = wallet::generate_mnemonic();
    let m2 = wallet::generate_mnemonic();
    let s1 = wallet::seed_from_mnemonic(&m1);
    let s2 = wallet::seed_from_mnemonic(&m2);
    assert_ne!(s1, s2);
}

#[test]
fn seed_from_hex_valid() {
    let hex_str = "aa".repeat(32);
    let seed = wallet::seed_from_hex(&hex_str).unwrap();
    assert_eq!(seed, [0xAA; 32]);
}

#[test]
fn seed_from_hex_with_0x_prefix() {
    let hex_str = format!("0x{}", "bb".repeat(32));
    let seed = wallet::seed_from_hex(&hex_str).unwrap();
    let expected = wallet::seed_from_hex(&"bb".repeat(32)).unwrap();
    assert_eq!(seed, expected);
}

#[test]
fn seed_from_hex_invalid_length() {
    let hex_str = "aa".repeat(31); // 62 chars = 31 bytes
    assert!(wallet::seed_from_hex(&hex_str).is_err());
}

#[test]
fn seed_from_hex_invalid_chars() {
    let hex_str = "gg".repeat(32);
    assert!(wallet::seed_from_hex(&hex_str).is_err());
}

#[test]
fn generate_mnemonic_12_words() {
    let mnemonic = wallet::generate_mnemonic();
    assert_eq!(mnemonic.word_count(), 12);
}

#[test]
fn open_returns_zeroizing() {
    let dir = TempDir::new().unwrap();
    let path = wallet_path(&dir, "zero");
    wallet::create_at(&path, "testpass", &[0xAA; 32]).unwrap();
    let seed: zeroize::Zeroizing<[u8; 32]> = wallet::open_at(&path, "testpass").unwrap();
    assert_eq!(seed.len(), 32);
}

#[cfg(unix)]
#[test]
fn file_permissions_restrictive() {
    use std::os::unix::fs::MetadataExt;

    let dir = TempDir::new().unwrap();
    let path = wallet_path(&dir, "perms");
    wallet::create_at(&path, "testpass", &[0xAA; 32]).unwrap();
    let mode = std::fs::metadata(&path).unwrap().mode() & 0o777;
    assert_eq!(mode, 0o600);
}
