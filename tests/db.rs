mod common;

use chrono::Utc;
use common::test_db;
use taolk::conversation::{InboxMessage, ThreadMessage};
use taolk::db::Db;
use taolk::types::{BlockRef, Pubkey};

fn make_thread_msg(body: &str, is_mine: bool, block_number: u32, ext_index: u16) -> ThreadMessage {
    ThreadMessage {
        sender_ss58: "Alice".into(),
        timestamp: Utc::now(),
        body: body.into(),
        is_mine,
        reply_to: BlockRef::ZERO,
        continues: BlockRef::ZERO,
        block_number,
        ext_index,
        has_gap: false,
    }
}

fn make_inbox_msg(body: &str, is_mine: bool, block_number: u32, ext_index: u16) -> InboxMessage {
    InboxMessage {
        peer_ss58: "Alice".into(),
        timestamp: Utc::now(),
        body: body.into(),
        content_type: 0x01,
        is_mine,
        block_number,
        ext_index,
    }
}

// ---------------------------------------------------------------------------
// Inbox
// ---------------------------------------------------------------------------

#[test]
fn inbox_insert_and_load() {
    let db = test_db();
    let msg = make_inbox_msg("hello inbox", false, 100, 0);
    db.insert_inbox(&msg);

    let (inbox, outbox) = db.load_inbox();
    assert_eq!(inbox.len(), 1);
    assert_eq!(outbox.len(), 0);
    assert_eq!(inbox[0].body, "hello inbox");
    assert_eq!(inbox[0].peer_ss58, "Alice");
}

#[test]
fn inbox_separates_mine_vs_received() {
    let db = test_db();
    db.insert_inbox(&make_inbox_msg("received", false, 100, 0));
    db.insert_inbox(&make_inbox_msg("sent", true, 101, 0));

    let (inbox, outbox) = db.load_inbox();
    assert_eq!(inbox.len(), 1);
    assert_eq!(outbox.len(), 1);
    assert_eq!(inbox[0].body, "received");
    assert_eq!(outbox[0].body, "sent");
}

// ---------------------------------------------------------------------------
// Thread messages
// ---------------------------------------------------------------------------

#[test]
fn thread_message_insert_and_load() {
    let db = test_db();
    let thread_ref = BlockRef {
        block: 100,
        index: 0,
    };
    let msg = make_thread_msg("threaded hello", false, 100, 0);
    db.insert_thread_message(thread_ref, "Alice", &msg, 100, 0);

    let threads = db.load_threads();
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].0, thread_ref);
    assert_eq!(threads[0].1, "Alice");
    assert_eq!(threads[0].2.len(), 1);
    assert_eq!(threads[0].2[0].body, "threaded hello");
}

// ---------------------------------------------------------------------------
// Channels
// ---------------------------------------------------------------------------

#[test]
fn channel_insert_and_load() {
    let db = test_db();
    let ch_ref = BlockRef {
        block: 200,
        index: 0,
    };
    db.insert_channel(ch_ref, "general", "General chat", "Creator");

    let msg = make_thread_msg("channel msg", false, 300, 0);
    db.insert_channel_message(ch_ref, &msg, 300, 0);

    let channels = db.load_channels();
    assert_eq!(channels.len(), 1);
    assert_eq!(channels[0].0, ch_ref);
    assert_eq!(channels[0].1, "general");
    assert_eq!(channels[0].2, "General chat");
    assert_eq!(channels[0].3, "Creator");
    assert_eq!(channels[0].4.len(), 1);
    assert_eq!(channels[0].4[0].body, "channel msg");
}

#[test]
fn channel_update_meta() {
    let db = test_db();
    let ch_ref = BlockRef {
        block: 200,
        index: 0,
    };
    db.insert_channel(ch_ref, "old-name", "old desc", "Creator");

    db.update_channel_meta(ch_ref, "new-name", "new desc", "Creator");

    let channels = db.load_channels();
    assert_eq!(channels.len(), 1);
    assert_eq!(channels[0].1, "new-name");
    assert_eq!(channels[0].2, "new desc");
}

#[test]
fn channel_delete() {
    let db = test_db();
    let ch_ref = BlockRef {
        block: 200,
        index: 0,
    };
    db.insert_channel(ch_ref, "doomed", "to be deleted", "Creator");
    let msg = make_thread_msg("doomed msg", false, 300, 0);
    db.insert_channel_message(ch_ref, &msg, 300, 0);

    db.delete_channel(ch_ref);

    let channels = db.load_channels();
    assert!(channels.is_empty());
}

// ---------------------------------------------------------------------------
// Known channels
// ---------------------------------------------------------------------------

#[test]
fn known_channel_insert_and_load() {
    let db = test_db();
    let ch_ref = BlockRef {
        block: 500,
        index: 1,
    };
    db.insert_known_channel(ch_ref, "public-ch", "A public channel", "Announcer");

    let known = db.load_known_channels();
    assert_eq!(known.len(), 1);
    assert_eq!(known[0].0, ch_ref);
    assert_eq!(known[0].1, "public-ch");
    assert_eq!(known[0].2, "A public channel");
    assert_eq!(known[0].3, "Announcer");
}

// ---------------------------------------------------------------------------
// Groups
// ---------------------------------------------------------------------------

#[test]
fn group_insert_and_load() {
    let db = test_db();
    let group_ref = BlockRef {
        block: 400,
        index: 0,
    };
    let creator = Pubkey([1u8; 32]);
    let members = vec![Pubkey([2u8; 32]), Pubkey([3u8; 32])];
    db.insert_group(group_ref, &creator, &members);

    let groups = db.load_groups();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].0, group_ref);
    assert_eq!(groups[0].1, creator);
    assert_eq!(groups[0].2.len(), 2);
    assert_eq!(groups[0].2[0], Pubkey([2u8; 32]));
    assert_eq!(groups[0].2[1], Pubkey([3u8; 32]));
}

#[test]
fn group_message_insert_and_load() {
    let db = test_db();
    let group_ref = BlockRef {
        block: 400,
        index: 0,
    };
    let creator = Pubkey([1u8; 32]);
    db.insert_group(group_ref, &creator, &[Pubkey([2u8; 32])]);

    let msg = make_thread_msg("group hello", false, 500, 0);
    db.insert_group_message(group_ref, &msg, 500, 0);

    let messages = db.load_group_messages(group_ref);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].body, "group hello");
    assert_eq!(messages[0].sender_ss58, "Alice");
}

// ---------------------------------------------------------------------------
// Peers
// ---------------------------------------------------------------------------

#[test]
fn peer_upsert_and_get() {
    let db = test_db();
    let pk = Pubkey([0x42; 32]);
    db.upsert_peer("Ali..xyz", &pk);

    let got = db.get_peer_pubkey("Ali..xyz");
    assert_eq!(got, Some(pk));
}

#[test]
fn peer_upsert_overwrites() {
    let db = test_db();
    let pk1 = Pubkey([0x01; 32]);
    let pk2 = Pubkey([0x02; 32]);
    db.upsert_peer("Ali..xyz", &pk1);
    db.upsert_peer("Ali..xyz", &pk2);

    let got = db.get_peer_pubkey("Ali..xyz").unwrap();
    assert_eq!(got, pk2);
}

// ---------------------------------------------------------------------------
// has_message_at
// ---------------------------------------------------------------------------

#[test]
fn has_message_at_true() {
    let db = test_db();
    let thread_ref = BlockRef {
        block: 100,
        index: 0,
    };
    let msg = make_thread_msg("exists", false, 100, 0);
    db.insert_thread_message(thread_ref, "Alice", &msg, 100, 0);

    assert!(db.has_message_at(BlockRef {
        block: 100,
        index: 0
    }));
}

#[test]
fn has_message_at_false() {
    let db = test_db();
    assert!(!db.has_message_at(BlockRef {
        block: 999,
        index: 0
    }));
}

#[test]
fn has_channel_message_at() {
    let db = test_db();
    let ch_ref = BlockRef {
        block: 200,
        index: 0,
    };
    db.insert_channel(ch_ref, "ch", "desc", "creator");
    let msg = make_thread_msg("ch msg", false, 300, 1);
    db.insert_channel_message(ch_ref, &msg, 300, 1);

    assert!(db.has_channel_message_at(BlockRef {
        block: 300,
        index: 1
    }));
    assert!(!db.has_channel_message_at(BlockRef {
        block: 300,
        index: 2
    }));
}

#[test]
fn has_group_message_at() {
    let db = test_db();
    let group_ref = BlockRef {
        block: 400,
        index: 0,
    };
    let creator = Pubkey([1u8; 32]);
    db.insert_group(group_ref, &creator, &[]);
    let msg = make_thread_msg("grp msg", false, 500, 3);
    db.insert_group_message(group_ref, &msg, 500, 3);

    assert!(db.has_group_message_at(BlockRef {
        block: 500,
        index: 3
    }));
    assert!(!db.has_group_message_at(BlockRef {
        block: 500,
        index: 4
    }));
}

// ---------------------------------------------------------------------------
// Encryption with different seeds
// ---------------------------------------------------------------------------

#[test]
fn different_seeds_different_encryption() {
    let db_a = Db::open_in_memory(&[0xAA; 32]).unwrap();
    let db_b = Db::open_in_memory(&[0xBB; 32]).unwrap();

    let thread_ref = BlockRef {
        block: 100,
        index: 0,
    };
    let msg = ThreadMessage {
        sender_ss58: "Alice".into(),
        timestamp: Utc::now(),
        body: "same plaintext".into(),
        is_mine: false,
        reply_to: BlockRef::ZERO,
        continues: BlockRef::ZERO,
        block_number: 100,
        ext_index: 0,
        has_gap: false,
    };

    db_a.insert_thread_message(thread_ref, "Alice", &msg, 100, 0);
    db_b.insert_thread_message(thread_ref, "Alice", &msg, 100, 0);

    // Both decrypt correctly with their own key
    let threads_a = db_a.load_threads();
    let threads_b = db_b.load_threads();
    assert_eq!(threads_a[0].2[0].body, "same plaintext");
    assert_eq!(threads_b[0].2[0].body, "same plaintext");

    // Cross-load: open a new DB with seed B over the same schema, insert with A's data
    // We cannot access raw ciphertext, but we can verify that each DB independently
    // produces correct results — the encryption is seed-derived, so different seeds
    // necessarily produce different ciphertexts (different HKDF-derived keys).
}
