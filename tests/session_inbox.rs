use chrono::Utc;
use taolk::db::Db;
use taolk::extrinsic::ChainInfo;
use taolk::metadata::AccountInfoLayout;
use taolk::secret::{Seed, SigningKey};
use taolk::session::Session;
use taolk::types::{BlockRef, Pubkey};
use zeroize::Zeroizing;

const ALICE_PUB: Pubkey = Pubkey([1u8; 32]);
const BOB_SEED: [u8; 32] = [2u8; 32];

fn signing_from_seed(seed: &[u8; 32]) -> SigningKey {
    Seed::from_bytes(*seed).derive_signing_key()
}

fn bob_pub() -> Pubkey {
    signing_from_seed(&BOB_SEED).public_key()
}

fn ci() -> ChainInfo {
    ChainInfo {
        genesis_hash: [0; 32],
        spec_version: 1,
        tx_version: 1,
        account_info_layout: AccountInfoLayout {
            free_offset: 16,
            free_width: 8,
        },
        errors: Default::default(),
        chain_name: "test".into(),
    }
}

fn bob_session() -> Session {
    let db = Db::open_in_memory(&BOB_SEED).unwrap();
    Session::new(
        signing_from_seed(&BOB_SEED),
        Zeroizing::new(BOB_SEED),
        "ws://test".into(),
        ci(),
        db,
    )
}

fn now() -> chrono::DateTime<Utc> {
    Utc::now()
}

fn br(block: u32, index: u16) -> BlockRef {
    BlockRef { block, index }
}

// ---------------------------------------------------------------------------
// Inbox / Outbox tests
// ---------------------------------------------------------------------------

#[test]
fn received_message_goes_to_inbox() {
    let mut s = bob_session();
    s.add_inbox_message(
        ALICE_PUB,
        bob_pub(),
        now(),
        "Hello".into(),
        0x11,
        br(100, 0),
    );
    assert_eq!(s.inbox.len(), 1);
    assert_eq!(s.outbox.len(), 0);
}

#[test]
fn sent_message_goes_to_outbox() {
    let mut s = bob_session();
    s.add_inbox_message(
        bob_pub(),
        ALICE_PUB,
        now(),
        "Hello".into(),
        0x11,
        br(100, 0),
    );
    assert_eq!(s.outbox.len(), 1);
    assert_eq!(s.inbox.len(), 0);
}

#[test]
fn inbox_dedup_by_block_ref() {
    let mut s = bob_session();
    s.add_inbox_message(
        ALICE_PUB,
        bob_pub(),
        now(),
        "First".into(),
        0x11,
        br(100, 0),
    );
    s.add_inbox_message(
        ALICE_PUB,
        bob_pub(),
        now(),
        "Duplicate".into(),
        0x11,
        br(100, 0),
    );
    assert_eq!(s.inbox.len(), 1);
}

#[test]
fn inbox_message_persisted() {
    let mut s = bob_session();
    s.add_inbox_message(
        ALICE_PUB,
        bob_pub(),
        now(),
        "Persisted".into(),
        0x11,
        br(100, 0),
    );
    assert_eq!(s.inbox.len(), 1);
    assert_eq!(s.inbox[0].body, "Persisted");
    assert_eq!(s.inbox[0].block_number, 100);
    assert_eq!(s.inbox[0].ext_index, 0);
}
