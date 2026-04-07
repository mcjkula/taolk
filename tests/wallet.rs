use taolk::secret::{Password, Seed};
use taolk::wallet;
use tempfile::TempDir;

fn wallet_path(dir: &TempDir, name: &str) -> std::path::PathBuf {
    dir.path().join(format!("{name}.key"))
}

fn pw(s: &str) -> Password {
    Password::new(s.into())
}

fn fixed_seed(byte: u8) -> Seed {
    Seed::from_bytes([byte; 32])
}

#[test]
fn create_and_open_roundtrip() {
    let dir = TempDir::new().unwrap();
    let path = wallet_path(&dir, "test");
    let original = fixed_seed(0xAA);
    wallet::create_at(&path, &pw("testpass"), &original).unwrap();
    let opened = wallet::open_at(&path, &pw("testpass")).unwrap();
    assert!(original.ct_eq(&opened));
}

#[test]
fn wrong_password_rejected() {
    let dir = TempDir::new().unwrap();
    let path = wallet_path(&dir, "test");
    wallet::create_at(&path, &pw("testpass"), &fixed_seed(0xAA)).unwrap();
    let result = wallet::open_at(&path, &pw("wrongpass"));
    match result {
        Err(wallet::WalletError::WrongPassword) => {}
        Err(e) => panic!("expected WrongPassword, got {e:?}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn corrupt_file_detected() {
    let dir = TempDir::new().unwrap();
    let path = wallet_path(&dir, "corrupt");
    std::fs::write(&path, b"garbage bytes here").unwrap();
    let result = wallet::open_at(&path, &pw("testpass"));
    match result {
        Err(wallet::WalletError::CorruptFile) => {}
        Err(e) => panic!("expected CorruptFile, got {e:?}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn open_returns_seed() {
    let dir = TempDir::new().unwrap();
    let path = wallet_path(&dir, "zero");
    wallet::create_at(&path, &pw("testpass"), &fixed_seed(0xAA)).unwrap();
    let _ = wallet::open_at(&path, &pw("testpass")).unwrap();
}

#[cfg(unix)]
#[test]
fn file_permissions_restrictive() {
    use std::os::unix::fs::MetadataExt;

    let dir = TempDir::new().unwrap();
    let path = wallet_path(&dir, "perms");
    wallet::create_at(&path, &pw("testpass"), &fixed_seed(0xAA)).unwrap();
    let mode = std::fs::metadata(&path).unwrap().mode() & 0o777;
    assert_eq!(mode, 0o600);
}
