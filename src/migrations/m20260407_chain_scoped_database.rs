use std::path::Path;

/// v1.0.2: Move legacy messages.db from wallet root into chain-scoped subdirectory.
///
/// Before: <wallet>/messages.db
/// After:  <wallet>/<chain_id>/messages.db
pub fn run(wallet_dir: &Path, chain_id: &str) {
    let legacy_path = wallet_dir.join("messages.db");
    let chain_dir = wallet_dir.join(chain_id);
    let new_path = chain_dir.join("messages.db");

    if legacy_path.exists() && !new_path.exists() {
        let _ = std::fs::create_dir_all(&chain_dir);
        let _ = std::fs::rename(&legacy_path, &new_path);
    }
}
