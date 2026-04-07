use std::path::Path;

pub fn run_all(wallet_dir: &Path, chain_id: &str) {
    move_legacy_messages_db(wallet_dir, chain_id);
}

// v1.0.2: <wallet>/messages.db → <wallet>/<chain_id>/messages.db
fn move_legacy_messages_db(wallet_dir: &Path, chain_id: &str) {
    let legacy_path = wallet_dir.join("messages.db");
    let chain_dir = wallet_dir.join(chain_id);
    let new_path = chain_dir.join("messages.db");

    if legacy_path.exists() && !new_path.exists() {
        let _ = std::fs::create_dir_all(&chain_dir);
        let _ = std::fs::rename(&legacy_path, &new_path);
    }
}
