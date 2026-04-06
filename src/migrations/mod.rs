mod m20260407_chain_scoped_database;

use std::path::Path;

pub fn run_all(wallet_dir: &Path, chain_id: &str) {
    m20260407_chain_scoped_database::run(wallet_dir, chain_id);
}
