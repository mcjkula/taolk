#![allow(dead_code)]

use chrono::{DateTime, Utc};
use samp::extrinsic::ChainParams;
use samp::metadata::StorageLayout;
use taolk::db::Db;
use taolk::extrinsic::ChainInfo;
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
        name: "Test".into(),
        ss58_prefix: 42,
        chain_params: ChainParams {
            genesis_hash: [0; 32],
            spec_version: 1,
            tx_version: 1,
        },
        account_storage: StorageLayout {
            offset: 16,
            width: 8,
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
        true,
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

pub fn build_remark_ext(remark: &[u8], sk: &SigningKey, nonce: u32) -> Vec<u8> {
    let mut args = Vec::new();
    samp::scale::encode_compact(remark.len() as u64, &mut args);
    args.extend_from_slice(remark);
    let ci = test_chain_info();
    let pk = *sk.public_key();
    samp::extrinsic::build_signed_extrinsic(
        0,
        7,
        &args,
        &pk,
        |msg| sk.sign(msg),
        nonce,
        &ci.chain_params,
    )
    .expect("build extrinsic")
}
