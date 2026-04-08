#![allow(dead_code)]

use chrono::{DateTime, Utc};
use taolk::db::Db;
use taolk::extrinsic::ChainInfo;
use taolk::metadata::AccountInfoLayout;
use taolk::secret::{Seed, SigningKey};
use taolk::session::Session;
use taolk::types::{BlockRef, Pubkey};
use zeroize::Zeroizing;

pub const ALICE_SEED: [u8; 32] = [0xAA; 32];
pub const BOB_SEED: [u8; 32] = [2u8; 32];
pub const CHARLIE_SEED: [u8; 32] = [0xCC; 32];
pub const DAVE_SEED: [u8; 32] = [0xDD; 32];

pub fn signing_from_seed(seed: &[u8; 32]) -> SigningKey {
    Seed::from_bytes(*seed).derive_signing_key()
}

pub fn alice_pubkey() -> Pubkey {
    signing_from_seed(&ALICE_SEED).public_key()
}

pub fn bob_pubkey() -> Pubkey {
    signing_from_seed(&BOB_SEED).public_key()
}

pub fn charlie_pubkey() -> Pubkey {
    signing_from_seed(&CHARLIE_SEED).public_key()
}

pub fn dave_pubkey() -> Pubkey {
    signing_from_seed(&DAVE_SEED).public_key()
}

pub fn test_chain_info() -> ChainInfo {
    ChainInfo {
        genesis_hash: [0; 32],
        spec_version: 1,
        tx_version: 1,
        account_info_layout: AccountInfoLayout {
            free_offset: 16,
            free_width: 8,
        },
        errors: Default::default(),
    }
}

pub fn test_db() -> Db {
    Db::open_in_memory(&BOB_SEED).expect("in-memory db")
}

pub fn session_for(seed: &[u8; 32]) -> Session {
    let db = Db::open_in_memory(seed).expect("in-memory db");
    Session::new(
        signing_from_seed(seed),
        Zeroizing::new(*seed),
        "ws://test".into(),
        test_chain_info(),
        db,
    )
}

pub fn alice_session() -> Session {
    session_for(&ALICE_SEED)
}

pub fn bob_session() -> Session {
    session_for(&BOB_SEED)
}

pub fn charlie_session() -> Session {
    session_for(&CHARLIE_SEED)
}

pub fn now() -> DateTime<Utc> {
    Utc::now()
}

pub fn br(block: u32, index: u16) -> BlockRef {
    BlockRef { block, index }
}
