mod common;

use common::{bob_session, br, now};
use taolk::conversation::NewMessage;
use taolk::types::{BlockRef, Pubkey};
use taolk::util;

const ALICE_PUB: Pubkey = Pubkey([1u8; 32]);

// ---------------------------------------------------------------------------
// Channel discovery
// ---------------------------------------------------------------------------

#[test]
fn discover_channel_adds_to_known() {
    let mut s = bob_session();
    s.discover_channel(
        "general".into(),
        "Main channel".into(),
        "Alice".into(),
        br(100, 0),
    );
    assert_eq!(s.known_channels.len(), 1);
    assert_eq!(s.known_channels[0].name, "general");
}

#[test]
fn discover_channel_updates_existing() {
    let mut s = bob_session();
    s.discover_channel(
        "general".into(),
        "Old desc".into(),
        "Alice".into(),
        br(100, 0),
    );
    s.discover_channel(
        "general-v2".into(),
        "New desc".into(),
        "Alice".into(),
        br(100, 0),
    );
    assert_eq!(s.known_channels.len(), 1);
    assert_eq!(s.known_channels[0].name, "general-v2");
    assert_eq!(s.known_channels[0].description, "New desc");
}

// ---------------------------------------------------------------------------
// Channel subscription
// ---------------------------------------------------------------------------

#[test]
fn subscribe_channel_creates_entry() {
    let mut s = bob_session();
    s.subscribe_channel(br(100, 0));
    assert_eq!(s.channels.len(), 1);
    assert!(s.is_subscribed(&br(100, 0)));
}

#[test]
fn subscribe_channel_copies_known_meta() {
    let mut s = bob_session();
    s.discover_channel(
        "general".into(),
        "Main channel".into(),
        "Alice".into(),
        br(100, 0),
    );
    s.subscribe_channel(br(100, 0));
    assert_eq!(s.channels.len(), 1);
    assert_eq!(s.channels[0].name, "general");
    assert_eq!(s.channels[0].description, "Main channel");
}

#[test]
fn unsubscribe_removes_channel() {
    let mut s = bob_session();
    s.subscribe_channel(br(100, 0));
    assert_eq!(s.channels.len(), 1);
    assert!(s.is_subscribed(&br(100, 0)));
    s.unsubscribe_channel(0);
    assert_eq!(s.channels.len(), 0);
    assert!(!s.is_subscribed(&br(100, 0)));
}

// ---------------------------------------------------------------------------
// Channel messages
// ---------------------------------------------------------------------------

#[test]
fn channel_message_adds_to_subscribed() {
    let mut s = bob_session();
    s.subscribe_channel(br(100, 0));
    s.add_channel_message(
        br(100, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Hello channel".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 200,
            ext_index: 0,
        },
    );
    assert_eq!(s.channels[0].messages.len(), 1);
}

#[test]
fn channel_message_rejected_if_not_subscribed() {
    let mut s = bob_session();
    s.add_channel_message(
        br(100, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Ignored".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 200,
            ext_index: 0,
        },
    );
    assert!(s.channels.is_empty());
}

#[test]
fn channel_message_dedup() {
    let mut s = bob_session();
    s.subscribe_channel(br(100, 0));
    let msg = || NewMessage {
        sender_ss58: util::ss58_short(&ALICE_PUB),
        timestamp: now(),
        body: "Msg".into(),
        reply_to: br(0, 0),
        continues: br(0, 0),
        block_number: 200,
        ext_index: 0,
    };
    s.add_channel_message(br(100, 0), msg());
    s.add_channel_message(br(100, 0), msg());
    assert_eq!(s.channels[0].messages.len(), 1);
}

// ---------------------------------------------------------------------------
// Channel gap detection
// ---------------------------------------------------------------------------

#[test]
fn channel_gap_detection() {
    let mut s = bob_session();
    s.subscribe_channel(br(100, 0));
    s.add_channel_message(
        br(100, 0),
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
    assert!(s.channels[0].messages[0].has_gap);

    // Fill the gap
    s.add_channel_message(
        br(100, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "The missing one".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 500,
            ext_index: 2,
        },
    );
    s.refresh_channel_gaps(0);
    assert!(
        !s.channels[0].messages[1].has_gap,
        "Message at 600 should no longer have gap"
    );
}

// ---------------------------------------------------------------------------
// Pending channel adoption
// ---------------------------------------------------------------------------

#[test]
fn create_pending_channel_adopted() {
    let mut s = bob_session();
    s.create_pending_channel("my-channel".into(), "Bob".into());
    assert_eq!(s.channels.len(), 1);
    assert_eq!(s.channels[0].channel_ref, BlockRef::ZERO);

    // Discover on-chain with same name -> adopts pending
    s.discover_channel(
        "my-channel".into(),
        "Description".into(),
        "Bob".into(),
        br(300, 0),
    );
    assert_eq!(s.channels.len(), 1);
    assert_eq!(s.channels[0].channel_ref, br(300, 0));
}
