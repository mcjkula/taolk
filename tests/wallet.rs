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

#[test]
fn wallet_list_empty_dir() {
    let dir = TempDir::new().unwrap();
    let entries = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.strip_suffix(".key").map(|n| n.to_string())
        })
        .collect::<Vec<_>>();
    assert!(entries.is_empty());
}

#[test]
fn wallet_list_after_create() {
    let dir = TempDir::new().unwrap();
    let path = wallet_path(&dir, "alice");
    wallet::create_at(&path, &pw("pass"), &fixed_seed(0xAA)).unwrap();

    let mut names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.strip_suffix(".key").map(|n| n.to_string())
        })
        .collect();
    names.sort();
    assert_eq!(names, vec!["alice"]);
}

#[test]
fn wallet_list_multiple_after_create() {
    let dir = TempDir::new().unwrap();
    wallet::create_at(&wallet_path(&dir, "bob"), &pw("p1"), &fixed_seed(0xBB)).unwrap();
    wallet::create_at(&wallet_path(&dir, "alice"), &pw("p2"), &fixed_seed(0xCC)).unwrap();

    let mut names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.strip_suffix(".key").map(|n| n.to_string())
        })
        .collect();
    names.sort();
    assert_eq!(names, vec!["alice", "bob"]);
}

#[test]
fn different_passwords_produce_different_ciphertext() {
    let dir = TempDir::new().unwrap();
    let p1 = wallet_path(&dir, "w1");
    let p2 = wallet_path(&dir, "w2");
    let seed = fixed_seed(0xDD);
    wallet::create_at(&p1, &pw("pass1"), &seed).unwrap();
    wallet::create_at(&p2, &pw("pass2"), &seed).unwrap();
    let d1 = std::fs::read(&p1).unwrap();
    let d2 = std::fs::read(&p2).unwrap();
    assert_ne!(d1, d2, "different passwords should produce different files");
}

#[test]
fn open_nonexistent_file() {
    let dir = TempDir::new().unwrap();
    let path = wallet_path(&dir, "ghost");
    let result = wallet::open_at(&path, &pw("pass"));
    assert!(result.is_err());
}

#[test]
fn create_overwrites_existing() {
    let dir = TempDir::new().unwrap();
    let path = wallet_path(&dir, "overwrite");
    wallet::create_at(&path, &pw("pass1"), &fixed_seed(0xAA)).unwrap();
    wallet::create_at(&path, &pw("pass2"), &fixed_seed(0xBB)).unwrap();
    let opened = wallet::open_at(&path, &pw("pass2")).unwrap();
    assert!(fixed_seed(0xBB).ct_eq(&opened));
}
