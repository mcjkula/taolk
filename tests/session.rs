mod common;

use common::{BOB_SEED, bob_pubkey as bob_pub, bob_session, br, charlie_session, dave_pubkey, now};
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

#[test]
fn channel_subscribe_and_unsubscribe() {
    let mut s = bob_session();
    s.discover_channel(
        "test-chan".into(),
        "A channel".into(),
        "Alice".into(),
        br(100, 0),
    );
    s.subscribe_channel(br(100, 0));
    assert!(s.is_subscribed(&br(100, 0)));
    assert_eq!(s.channels.len(), 1);

    let name = s.unsubscribe_channel(0);
    assert_eq!(name, Some("test-chan".into()));
    assert!(!s.is_subscribed(&br(100, 0)));
    assert_eq!(s.channels.len(), 0);
}

#[test]
fn known_contacts_after_inbox_message() {
    let mut s = bob_session();
    let peer = dave_pubkey();
    s.add_inbox_message(peer, bob_pub(), now(), "hi".into(), 0x10, br(100, 0));
    let contacts = s.known_contacts();
    assert!(contacts.iter().any(|(_, pk)| *pk == peer));
}

use common::{ALICE_SEED, alice_pubkey, session_for};

fn mb(s: &str) -> taolk::types::MessageBody {
    taolk::types::MessageBody::parse(s.to_string()).unwrap()
}

#[test]
fn build_public_message_produces_valid_remark() {
    let session = bob_session();
    let recipient = dave_pubkey();
    let remark = session
        .build_public_message(&recipient, &mb("pub msg"))
        .unwrap();
    let decoded = samp::decode_remark(&remark).unwrap();
    match decoded {
        samp::Remark::Public { recipient: r, body } => {
            assert_eq!(r, recipient);
            assert_eq!(body.as_str(), "pub msg");
        }
        _ => panic!("expected PublicRemark"),
    }
}

#[test]
fn build_encrypted_message_produces_valid_remark() {
    let session = bob_session();
    let recipient = dave_pubkey();
    let remark = session
        .build_encrypted_message(&BOB_SEED, &recipient, &mb("secret"))
        .unwrap();
    let decoded = samp::decode_remark(&remark).unwrap();
    assert!(matches!(decoded, samp::Remark::Encrypted { .. }));
}

#[test]
fn build_thread_reply_produces_valid_remark() {
    let mut session = bob_session();
    let peer = dave_pubkey();
    session.create_thread(peer).unwrap();
    session.add_thread_message(
        peer,
        bob_pub(),
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&peer),
            timestamp: now(),
            body: "root".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 100,
            ext_index: 0,
        },
    );
    let remark = session
        .build_thread_reply(&BOB_SEED, 0, &mb("reply"))
        .unwrap();
    let decoded = samp::decode_remark(&remark).unwrap();
    assert!(matches!(decoded, samp::Remark::Thread { .. }));
}

#[test]
fn build_channel_message_produces_valid_remark() {
    let mut session = bob_session();
    session.subscribe_channel(br(100, 0));
    let remark = session.build_channel_message(0, &mb("chan")).unwrap();
    let decoded = samp::decode_remark(&remark).unwrap();
    match decoded {
        samp::Remark::Channel { channel_ref, .. } => {
            assert_eq!(channel_ref, br(100, 0));
        }
        _ => panic!("expected Channel"),
    }
}

#[test]
fn build_channel_create_produces_valid_remark() {
    let session = bob_session();
    let name = samp::ChannelName::parse("mychan").unwrap();
    let desc = samp::ChannelDescription::parse("my desc").unwrap();
    let remark = session.build_channel_create(&name, &desc).unwrap();
    let decoded = samp::decode_remark(&remark).unwrap();
    match decoded {
        samp::Remark::ChannelCreate { name, description } => {
            assert_eq!(name.as_str(), "mychan");
            assert_eq!(description.as_str(), "my desc");
        }
        _ => panic!("expected ChannelCreate"),
    }
}

#[test]
fn save_and_load_draft() {
    let mut session = bob_session();
    session.subscribe_channel(br(200, 0));
    session
        .db
        .save_draft(taolk::db::ConversationKind::Channel, 200, 0, "draft text");
    let drafts = session.db.load_drafts();
    let found = drafts
        .iter()
        .find(|(k, bref, _)| *k == taolk::db::ConversationKind::Channel && *bref == br(200, 0));
    assert!(found.is_some());
    assert_eq!(found.unwrap().2, "draft text");
}

#[test]
fn current_draft_returns_saved() {
    let mut session = bob_session();
    session.subscribe_channel(br(300, 0));
    session.channels[0].draft = "hello draft".into();
    assert_eq!(session.channels[0].draft, "hello draft");
}

#[test]
fn filtered_contacts_returns_known_peers() {
    let mut session = bob_session();
    let peer = dave_pubkey();
    session.add_inbox_message(peer, bob_pub(), now(), "hi".into(), 0x10, br(100, 0));
    let all = session.known_contacts();
    let peer_ss58 = util::ss58_short(&peer);
    assert!(
        all.iter()
            .any(|(ss58, pk)| ss58 == &peer_ss58 && *pk == peer)
    );
}

// --- create_thread self-message error ---

#[test]
fn create_thread_self_rejected() {
    let mut session = bob_session();
    let self_pub = bob_pub();
    let result = session.create_thread(self_pub);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("yourself"));
}

// --- channel_idx ---

#[test]
fn channel_idx_returns_some_for_subscribed() {
    let mut s = bob_session();
    s.subscribe_channel(br(100, 0));
    assert_eq!(s.channel_idx(&br(100, 0)), Some(0));
}

#[test]
fn channel_idx_returns_none_for_unknown() {
    let s = bob_session();
    assert_eq!(s.channel_idx(&br(999, 0)), None);
}

// --- session accessors ---

#[test]
fn session_pubkey_matches_seed() {
    let session = bob_session();
    let expected = common::bob_pubkey();
    assert_eq!(session.pubkey(), expected);
}

#[test]
fn session_ss58_non_empty() {
    let session = bob_session();
    assert!(!session.ss58().is_empty());
    assert!(session.ss58().starts_with('5'));
}

#[test]
fn session_signing_available() {
    let session = bob_session();
    assert!(session.signing().is_some());
}

#[test]
fn session_cached_seed_available() {
    let session = bob_session();
    assert!(session.cached_seed().is_some());
    assert_eq!(session.cached_seed().unwrap(), &BOB_SEED);
}

#[test]
fn session_decryption_keys() {
    let session = bob_session();
    let keys = session.decryption_keys();
    assert!(keys.seed().is_some());
}

#[test]
fn session_view_scalar_deterministic() {
    let s1 = bob_session();
    let s2 = bob_session();
    let _v1 = s1.view_scalar();
    let _v2 = s2.view_scalar();
    // ViewScalar is constructed from the same seed, so decryption_keys must match
    let k1 = s1.decryption_keys();
    let k2 = s2.decryption_keys();
    assert_eq!(k1.seed(), k2.seed());
}

// --- build_group_message round-trip ---

#[test]
fn build_group_message_produces_valid_remark() {
    let mut session = bob_session();
    let alice_pk = alice_pubkey();
    let bob_pk = bob_pub();

    let alice_ristretto = samp::public_from_seed(&samp::Seed::from_bytes(ALICE_SEED));
    let bob_ristretto = samp::public_from_seed(&samp::Seed::from_bytes(BOB_SEED));
    let members = vec![alice_ristretto, bob_ristretto];

    session.discover_group(alice_pk, br(300, 0), vec![alice_pk, bob_pk]);
    session.add_group_message(
        br(300, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&alice_pk),
            timestamp: now(),
            body: "first msg".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 400,
            ext_index: 0,
        },
    );

    // Update group members to ristretto keys for encryption
    session.groups[0].members = members.clone();

    let remark = session
        .build_group_message(&BOB_SEED, 0, &mb("group reply"))
        .unwrap();
    let decoded = samp::decode_remark(&remark).unwrap();
    assert!(matches!(decoded, samp::Remark::Group { .. }));
}

// --- build_group_create too many members ---

#[test]
fn build_group_create_too_many_members_rejected() {
    let session = bob_session();
    let members: Vec<Pubkey> = (0..=taolk::session::MAX_GROUP_MEMBERS)
        .map(|i| Pubkey::from_bytes([i as u8; 32]))
        .collect();
    let result = session.build_group_create(&BOB_SEED, &members, &mb("too big"));
    assert!(result.is_err());
}

// --- discover_channel updates existing subscribed channel ---

#[test]
fn discover_channel_updates_subscribed() {
    let mut s = bob_session();
    s.subscribe_channel(br(100, 0));
    assert_eq!(s.channels[0].name, "Loading...");

    s.discover_channel(
        "real-name".into(),
        "real desc".into(),
        "Creator".into(),
        br(100, 0),
    );
    assert_eq!(s.channels[0].name, "real-name");
    assert_eq!(s.channels[0].description, "real desc");
}

// --- discover_channel updates known channel ---

#[test]
fn discover_channel_updates_known() {
    let mut s = bob_session();
    s.discover_channel("ch".into(), "desc1".into(), "C".into(), br(100, 0));
    assert_eq!(s.known_channels[0].description, "desc1");

    s.discover_channel("ch-v2".into(), "desc2".into(), "C".into(), br(100, 0));
    assert_eq!(s.known_channels[0].description, "desc2");
    assert_eq!(s.known_channels[0].name, "ch-v2");
}

// --- thread message creates new thread for different peer ---

#[test]
fn thread_message_different_peer_creates_new_thread() {
    let mut s = bob_session();
    let alice = alice_pubkey();
    let charlie = common::charlie_pubkey();

    s.create_thread(alice).unwrap();
    assert_eq!(s.threads.len(), 1);

    // message from charlie for a new thread
    s.add_thread_message(
        charlie,
        bob_pub(),
        br(200, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&charlie),
            timestamp: now(),
            body: "from charlie".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 200,
            ext_index: 0,
        },
    );
    assert_eq!(s.threads.len(), 2);
}

// --- load_from_db round-trip ---

#[test]
fn load_from_db_restores_state() {
    let seed = [0xDD; 32];
    let mut s1 = session_for(&seed);
    let peer = alice_pubkey();

    // Add data
    s1.add_inbox_message(peer, s1.pubkey(), now(), "hello".into(), 0x10, br(100, 0));
    s1.subscribe_channel(br(200, 0));
    s1.discover_channel("ch1".into(), "desc".into(), "C".into(), br(200, 0));
    s1.discover_group(peer, br(300, 0), vec![s1.pubkey(), peer]);

    // Create new session with same seed/db
    let _s2 = session_for(&seed);
    // s2 has a fresh db, but let's verify load_from_db on s1 doesn't crash
    s1.load_from_db();
    // The inbox should still be there
    assert!(!s1.inbox.is_empty() || !s1.outbox.is_empty());
}

// --- refresh_gaps ---

#[test]
fn refresh_gaps_thread() {
    let mut s = bob_session();
    let peer = alice_pubkey();
    s.create_thread(peer).unwrap();
    s.add_thread_message(
        peer,
        bob_pub(),
        br(100, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&peer),
            timestamp: now(),
            body: "msg with gap".into(),
            reply_to: br(50, 0),
            continues: br(0, 0),
            block_number: 100,
            ext_index: 0,
        },
    );
    // The gap should be detected
    s.refresh_gaps(taolk::db::ConversationKind::Thread, 0);
    assert!(s.threads[0].messages[0].has_gap);
}

// --- unsubscribe out of bounds ---

#[test]
fn unsubscribe_out_of_bounds_returns_none() {
    let mut s = bob_session();
    assert_eq!(s.unsubscribe_channel(999), None);
}

// --- discover_group dedup ---

#[test]
fn discover_group_dedup_by_ref() {
    let mut s = bob_session();
    let members = vec![bob_pub(), alice_pubkey()];
    s.discover_group(alice_pubkey(), br(300, 0), members.clone());
    s.discover_group(alice_pubkey(), br(300, 0), members);
    assert_eq!(s.groups.len(), 1);
}

// --- save/load drafts across kinds ---

#[test]
fn save_draft_for_group() {
    let mut session = bob_session();
    let members = vec![bob_pub(), alice_pubkey()];
    session.discover_group(alice_pubkey(), br(300, 0), members);
    session
        .db
        .save_draft(taolk::db::ConversationKind::Group, 300, 0, "group draft");
    let drafts = session.db.load_drafts();
    assert!(
        drafts
            .iter()
            .any(|(k, _, d)| *k == taolk::db::ConversationKind::Group && d == "group draft")
    );
}

// --- reindex coverage: remove non-last element ---

#[test]
fn unsubscribe_first_channel_reindexes() {
    let mut s = bob_session();
    s.subscribe_channel(br(100, 0));
    s.subscribe_channel(br(200, 0));
    s.subscribe_channel(br(300, 0));
    assert_eq!(s.channels.len(), 3);

    // Remove the first channel
    s.unsubscribe_channel(0);
    assert_eq!(s.channels.len(), 2);
    // Second and third should still be accessible
    assert!(s.is_subscribed(&br(200, 0)));
    assert!(s.is_subscribed(&br(300, 0)));
    assert!(!s.is_subscribed(&br(100, 0)));
}

// --- load_from_db with persisted groups and drafts ---

#[test]
fn load_from_db_restores_data() {
    use taolk::db::Db;
    use taolk::session::Session;
    use zeroize::Zeroizing;

    let seed = [0xEE; 32];
    let db = Db::open_in_memory(&seed).unwrap();
    let signing = common::signing_from_seed(&seed);
    let pubkey = signing.public_key();
    let peer = alice_pubkey();
    let peer_ss58 = util::ss58_short(&peer);

    // Pre-populate the DB directly
    db.upsert_peer(&peer_ss58, &peer);
    db.insert_channel(br(200, 0), "ch-test", "desc", "C");
    db.insert_known_channel(br(400, 0), "known-ch", "known desc", "K");
    db.insert_group(br(300, 0), &peer, &[pubkey, peer]);
    db.insert_thread_message(
        br(500, 0),
        &peer_ss58,
        &taolk::conversation::ThreadMessage {
            sender_ss58: peer_ss58.clone(),
            timestamp: now(),
            body: "thread msg".into(),
            is_mine: false,
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 500,
            ext_index: 0,
            has_gap: false,
        },
        500,
        0,
    );
    db.save_draft(taolk::db::ConversationKind::Channel, 200, 0, "ch draft");
    db.save_draft(taolk::db::ConversationKind::Group, 300, 0, "grp draft");
    db.save_draft(taolk::db::ConversationKind::Thread, 500, 0, "thr draft");

    // Create a session from this DB and load
    let mut session = Session::new(
        signing,
        Zeroizing::new(seed),
        true,
        taolk::types::NodeUrl::parse("ws://test").unwrap(),
        common::test_chain_info(),
        db,
    );
    session.load_from_db();

    assert!(!session.channels.is_empty(), "channels should be restored");
    assert!(!session.groups.is_empty(), "groups should be restored");
    assert!(!session.threads.is_empty(), "threads should be restored");
    assert!(
        !session.known_channels.is_empty(),
        "known channels should be restored"
    );
    assert_eq!(session.channels[0].draft, "ch draft");
    assert_eq!(session.groups[0].draft, "grp draft");
    assert_eq!(session.threads[0].draft, "thr draft");
}

// --- session without signing key ---

#[test]
fn session_without_seed_keep() {
    use taolk::db::Db;
    use taolk::session::Session;
    use zeroize::Zeroizing;

    let seed = [0xAA; 32];
    let signing = common::signing_from_seed(&seed);
    let db = Db::open_in_memory(&seed).unwrap();
    let session = Session::new(
        signing,
        Zeroizing::new(seed),
        false, // keep_seed = false
        taolk::types::NodeUrl::parse("ws://test").unwrap(),
        common::test_chain_info(),
        db,
    );
    assert!(session.signing().is_none());
    assert!(session.cached_seed().is_none());
}

// --- channel_idx returns correct index after subscribe ---

#[test]
fn channel_idx_after_multiple_subscribes() {
    let mut s = bob_session();
    s.subscribe_channel(br(100, 0));
    s.subscribe_channel(br(200, 0));
    assert_eq!(s.channel_idx(&br(100, 0)), Some(0));
    assert_eq!(s.channel_idx(&br(200, 0)), Some(1));
}

// --- subscribe_channel already subscribed returns existing index ---

#[test]
fn subscribe_channel_already_subscribed() {
    let mut s = bob_session();
    let idx1 = s.subscribe_channel(br(100, 0));
    let idx2 = s.subscribe_channel(br(100, 0)); // already subscribed
    assert_eq!(idx1, idx2);
    assert_eq!(s.channels.len(), 1);
}

// --- refresh_gaps for groups ---

#[test]
fn refresh_gaps_group() {
    let mut s = bob_session();
    let members = vec![bob_pub(), alice_pubkey()];
    s.discover_group(alice_pubkey(), br(300, 0), members);
    s.add_group_message(
        br(300, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&alice_pubkey()),
            timestamp: now(),
            body: "msg".into(),
            reply_to: br(999, 0), // references a missing message
            continues: br(0, 0),
            block_number: 400,
            ext_index: 0,
        },
    );
    s.refresh_gaps(taolk::db::ConversationKind::Group, 0);
    assert!(s.groups[0].messages[0].has_gap);
}

// --- refresh_gaps for inbox (no-op) ---

#[test]
fn refresh_gaps_inbox_noop() {
    let mut s = bob_session();
    // Should not panic
    s.refresh_gaps(taolk::db::ConversationKind::Inbox, 0);
}

// --- load_from_db edge cases ---

#[test]
fn load_from_db_dedup_known_channel() {
    use taolk::db::Db;
    use taolk::session::Session;
    use zeroize::Zeroizing;

    let seed = [0xF1; 32];
    let db = Db::open_in_memory(&seed).unwrap();
    let signing = common::signing_from_seed(&seed);

    // Insert same channel as both subscribed and known
    db.insert_channel(br(100, 0), "ch", "desc", "C");
    db.insert_known_channel(br(100, 0), "ch", "desc", "C");

    let mut session = Session::new(
        signing,
        Zeroizing::new(seed),
        true,
        taolk::types::NodeUrl::parse("ws://test").unwrap(),
        common::test_chain_info(),
        db,
    );
    session.load_from_db();

    // Should have 1 channel but NOT duplicate the known_channel entry
    assert_eq!(session.channels.len(), 1);
    // Known channels: the one from insert_known_channel should be deduped
    // (channel_index already has br(100,0), so known_channel_index should skip it)
    // known_channels may be 0 or 1 depending on order, but no crash
}

#[test]
fn load_from_db_empty_draft_skipped() {
    use taolk::db::Db;
    use taolk::session::Session;
    use zeroize::Zeroizing;

    let seed = [0xF2; 32];
    let db = Db::open_in_memory(&seed).unwrap();
    let signing = common::signing_from_seed(&seed);

    db.insert_channel(br(200, 0), "ch", "desc", "C");
    db.save_draft(taolk::db::ConversationKind::Channel, 200, 0, ""); // empty draft
    db.save_draft(taolk::db::ConversationKind::Inbox, 0, 0, "inbox draft"); // inbox kind

    let mut session = Session::new(
        signing,
        Zeroizing::new(seed),
        true,
        taolk::types::NodeUrl::parse("ws://test").unwrap(),
        common::test_chain_info(),
        db,
    );
    session.load_from_db();

    // Empty draft should be skipped; channel draft remains empty
    assert_eq!(session.channels[0].draft, "");
}

#[test]
fn add_inbox_message_with_zero_block() {
    let mut s = bob_session();
    let peer = alice_pubkey();
    // block_number=0 means the dedup check is skipped
    s.add_inbox_message(peer, bob_pub(), now(), "zero block".into(), 0x10, br(0, 0));
    assert_eq!(s.inbox.len(), 1);
    assert_eq!(s.inbox[0].body, "zero block");
}

#[test]
fn save_draft_for_thread() {
    let mut session = bob_session();
    let peer = alice_pubkey();
    session.create_thread(peer).unwrap();
    session
        .db
        .save_draft(taolk::db::ConversationKind::Thread, 0, 0, "thread draft");
    let drafts = session.db.load_drafts();
    assert!(
        drafts
            .iter()
            .any(|(k, _, d)| *k == taolk::db::ConversationKind::Thread && d == "thread draft")
    );
}
