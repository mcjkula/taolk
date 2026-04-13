use std::path::Path;

pub fn run_all(wallet_dir: &Path, chain_id: &str) {
    move_legacy_messages_db(wallet_dir, chain_id);
}

fn move_legacy_messages_db(wallet_dir: &Path, chain_id: &str) {
    let legacy_path = wallet_dir.join("messages.db");
    let chain_dir = wallet_dir.join(chain_id);
    let new_path = chain_dir.join("messages.db");

    if legacy_path.exists() && !new_path.exists() {
        let _ = std::fs::create_dir_all(&chain_dir);
        let _ = std::fs::rename(&legacy_path, &new_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn run_all_no_op_when_no_legacy_db() {
        let dir = TempDir::new().unwrap();
        run_all(dir.path(), "abcd1234");
        assert!(!dir.path().join("messages.db").exists());
        assert!(!dir.path().join("abcd1234").exists());
    }

    #[test]
    fn run_all_moves_legacy_db_into_chain_subdir() {
        let dir = TempDir::new().unwrap();
        let legacy = dir.path().join("messages.db");
        std::fs::write(&legacy, b"old data").unwrap();
        run_all(dir.path(), "abcd1234");
        assert!(!legacy.exists());
        let new_path = dir.path().join("abcd1234").join("messages.db");
        assert!(new_path.exists());
        assert_eq!(std::fs::read(&new_path).unwrap(), b"old data");
    }

    #[test]
    fn run_all_no_op_when_destination_already_exists() {
        let dir = TempDir::new().unwrap();
        let legacy = dir.path().join("messages.db");
        std::fs::write(&legacy, b"old").unwrap();
        std::fs::create_dir(dir.path().join("abcd1234")).unwrap();
        let dest = dir.path().join("abcd1234").join("messages.db");
        std::fs::write(&dest, b"new").unwrap();

        run_all(dir.path(), "abcd1234");
        assert!(legacy.exists());
        assert_eq!(std::fs::read(&dest).unwrap(), b"new");
    }
}
