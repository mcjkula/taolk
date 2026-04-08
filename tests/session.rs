mod common;

use common::{bob_pubkey as bob_pub, bob_session, br, charlie_session, dave_pubkey, now};
use taolk::conversation::NewMessage;
use taolk::types::{BlockRef, Pubkey};
use taolk::util;

const ALICE_PUB: Pubkey = Pubkey::from_bytes([1u8; 32]);
const CHARLIE_PUB: Pubkey = Pubkey::from_bytes([3u8; 32]);

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
    s.refresh_gaps(taolk::db::ConversationKind::Channel, 0);
    assert!(!s.channels[0].messages[1].has_gap);
}

#[test]
fn create_pending_channel_adopted() {
    let mut s = bob_session();
    s.create_pending_channel("my-channel".into(), "Bob".into());
    assert_eq!(s.channels.len(), 1);
    assert_eq!(s.channels[0].channel_ref, BlockRef::ZERO);

    s.discover_channel(
        "my-channel".into(),
        "Description".into(),
        "Bob".into(),
        br(300, 0),
    );
    assert_eq!(s.channels.len(), 1);
    assert_eq!(s.channels[0].channel_ref, br(300, 0));
}

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

#[test]
fn session_known_contacts_after_thread() {
    let mut session = charlie_session();
    let peer = dave_pubkey();
    session.create_thread(peer).unwrap();
    let contacts = session.known_contacts();
    assert!(contacts.iter().any(|(_, pk)| *pk == peer));
}

#[test]
fn session_cleanup_pending_thread() {
    let mut session = charlie_session();
    let peer = dave_pubkey();
    session.create_thread(peer).unwrap();
    assert_eq!(session.threads.len(), 1);
    assert_eq!(session.threads[0].thread_ref, BlockRef::ZERO);

    let result = session.cleanup_pending();
    assert!(result.is_some());
    assert!(result.unwrap().removed_thread.is_some());
    assert_eq!(session.threads.len(), 0);
}

#[test]
fn session_cleanup_pending_channel() {
    let mut session = charlie_session();
    session.create_pending_channel("test-chan".into(), "creator".into());
    assert_eq!(session.channels.len(), 1);
    assert_eq!(session.channels[0].channel_ref, BlockRef::ZERO);

    let result = session.cleanup_pending();
    assert!(result.is_some());
    assert!(result.unwrap().removed_channel.is_some());
    assert_eq!(session.channels.len(), 0);
}

#[test]
fn session_cleanup_pending_group() {
    let mut session = charlie_session();
    let peer = dave_pubkey();
    session.create_pending_group(session.pubkey(), vec![session.pubkey(), peer]);
    assert_eq!(session.groups.len(), 1);
    assert_eq!(session.groups[0].group_ref, BlockRef::ZERO);

    let result = session.cleanup_pending();
    assert!(result.is_some());
    assert!(result.unwrap().removed_group.is_some());
    assert_eq!(session.groups.len(), 0);
}

#[test]
fn session_cleanup_nothing_pending() {
    let mut session = charlie_session();
    let result = session.cleanup_pending();
    assert!(result.is_none());
}
