use chrono::Utc;
use schnorrkel::keys::{ExpansionMode, MiniSecretKey};
use taolk::conversation::NewMessage;
use taolk::db::Db;
use taolk::extrinsic::ChainInfo;
use taolk::metadata::AccountInfoLayout;
use taolk::session::Session;
use taolk::types::{BlockRef, Pubkey};
use taolk::util;
use zeroize::Zeroizing;

const ALICE_PUB: Pubkey = Pubkey([1u8; 32]);
const CHARLIE_PUB: Pubkey = Pubkey([3u8; 32]);
const BOB_SEED: [u8; 32] = [2u8; 32];

fn keypair_from_seed(seed: &[u8; 32]) -> schnorrkel::Keypair {
    MiniSecretKey::from_bytes(seed)
        .unwrap()
        .expand_to_keypair(ExpansionMode::Ed25519)
}

fn bob_pub() -> Pubkey {
    Pubkey(keypair_from_seed(&BOB_SEED).public.to_bytes())
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
        keypair_from_seed(&BOB_SEED),
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
// Group creation
// ---------------------------------------------------------------------------

#[test]
fn create_pending_group() {
    let mut s = bob_session();
    let members = vec![bob_pub(), ALICE_PUB, CHARLIE_PUB];
    s.create_pending_group(bob_pub(), members);
    assert_eq!(s.groups.len(), 1);
    assert_eq!(s.groups[0].group_ref, BlockRef::ZERO);
}

#[test]
fn discover_group_adopts_pending() {
    let mut s = bob_session();
    let members = vec![bob_pub(), ALICE_PUB, CHARLIE_PUB];
    s.create_pending_group(bob_pub(), members.clone());
    assert_eq!(s.groups[0].group_ref, BlockRef::ZERO);

    s.discover_group(bob_pub(), br(200, 0), members);
    assert_eq!(s.groups.len(), 1);
    assert_eq!(s.groups[0].group_ref, br(200, 0));
}

#[test]
fn discover_group_creates_new() {
    let mut s = bob_session();
    let members = vec![bob_pub(), ALICE_PUB];
    s.discover_group(ALICE_PUB, br(300, 0), members);
    assert_eq!(s.groups.len(), 1);
    assert_eq!(s.groups[0].group_ref, br(300, 0));
    assert_eq!(s.groups[0].creator_pubkey, ALICE_PUB);
}

// ---------------------------------------------------------------------------
// Group messages
// ---------------------------------------------------------------------------

#[test]
fn group_message_adds_to_group() {
    let mut s = bob_session();
    let members = vec![bob_pub(), ALICE_PUB];
    s.discover_group(ALICE_PUB, br(300, 0), members);
    s.add_group_message(
        br(300, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Hello group".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 400,
            ext_index: 0,
        },
    );
    assert_eq!(s.groups[0].messages.len(), 1);
}

#[test]
fn group_message_rejected_unknown_ref() {
    let mut s = bob_session();
    s.add_group_message(
        br(999, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Ignored".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 400,
            ext_index: 0,
        },
    );
    assert!(s.groups.is_empty());
}

#[test]
fn group_message_dedup() {
    let mut s = bob_session();
    let members = vec![bob_pub(), ALICE_PUB];
    s.discover_group(ALICE_PUB, br(300, 0), members);
    let msg = || NewMessage {
        sender_ss58: util::ss58_short(&ALICE_PUB),
        timestamp: now(),
        body: "Msg".into(),
        reply_to: br(0, 0),
        continues: br(0, 0),
        block_number: 400,
        ext_index: 0,
    };
    s.add_group_message(br(300, 0), msg());
    s.add_group_message(br(300, 0), msg());
    assert_eq!(s.groups[0].messages.len(), 1);
}

// ---------------------------------------------------------------------------
// Group gap detection
// ---------------------------------------------------------------------------

#[test]
fn group_gap_detection() {
    let mut s = bob_session();
    let members = vec![bob_pub(), ALICE_PUB];
    s.discover_group(ALICE_PUB, br(300, 0), members);
    s.add_group_message(
        br(300, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "References missing".into(),
            reply_to: br(500, 2),
            continues: br(0, 0),
            block_number: 600,
            ext_index: 0,
        },
    );
    assert!(s.groups[0].messages[0].has_gap);
}
