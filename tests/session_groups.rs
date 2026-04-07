mod common;

use common::{bob_pubkey as bob_pub, bob_session, br, now};
use taolk::conversation::NewMessage;
use taolk::types::{BlockRef, Pubkey};
use taolk::util;

const ALICE_PUB: Pubkey = Pubkey([1u8; 32]);
const CHARLIE_PUB: Pubkey = Pubkey([3u8; 32]);

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
