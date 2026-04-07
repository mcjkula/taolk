mod common;

use common::{
    BOB_SEED, bob_pubkey as bob_pub, bob_session, br, now, signing_from_seed, test_chain_info as ci,
};
use taolk::conversation::NewMessage;
use taolk::db::Db;
use taolk::session::Session;
use taolk::types::Pubkey;
use taolk::util;
use zeroize::Zeroizing;

const ALICE_PUB: Pubkey = Pubkey([1u8; 32]);
const CHARLIE_PUB: Pubkey = Pubkey([3u8; 32]);

// ---------------------------------------------------------------------------
// Thread identity: messages with same thread_ref -> one thread
// ---------------------------------------------------------------------------

#[test]
fn two_messages_same_thread() {
    let mut s = bob_session();
    // Root message: thread_ref=ZERO -> becomes thread (100,0)
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Hi!".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 100,
            ext_index: 0,
        },
    );
    // Second message: thread_ref=(100,0) -> same thread
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(100, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Second".into(),
            reply_to: br(100, 0),
            continues: br(100, 0),
            block_number: 102,
            ext_index: 0,
        },
    );
    assert_eq!(s.threads.len(), 1);
    assert_eq!(s.threads[0].messages.len(), 2);
}

#[test]
fn sent_and_received_in_same_thread() {
    let mut s = bob_session();
    // Bob starts thread: root at (100,0)
    s.add_thread_message(
        bob_pub(),
        ALICE_PUB,
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&bob_pub()),
            timestamp: now(),
            body: "Hello Alice".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 100,
            ext_index: 0,
        },
    );
    // Alice replies in same thread
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(100, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Hello Bob".into(),
            reply_to: br(100, 0),
            continues: br(0, 0),
            block_number: 101,
            ext_index: 0,
        },
    );
    assert_eq!(s.threads.len(), 1);
    assert_eq!(s.threads[0].messages.len(), 2);
    assert_eq!(s.threads[0].peer_ss58, util::ss58_short(&ALICE_PUB));
}

#[test]
fn different_peers_different_threads() {
    let mut s = bob_session();
    // Thread with Alice
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "From Alice".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 100,
            ext_index: 0,
        },
    );
    // Thread with Charlie
    s.add_thread_message(
        CHARLIE_PUB,
        bob_pub(),
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&CHARLIE_PUB),
            timestamp: now(),
            body: "From Charlie".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 101,
            ext_index: 0,
        },
    );
    assert_eq!(s.threads.len(), 2);
}

#[test]
fn two_threads_same_peer() {
    let mut s = bob_session();
    // Thread 1 with Alice: root at (100,0)
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Thread 1".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 100,
            ext_index: 0,
        },
    );
    // Thread 2 with Alice: root at (200,0)
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Thread 2".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 200,
            ext_index: 0,
        },
    );
    assert_eq!(s.threads.len(), 2);
    assert_eq!(s.threads[0].thread_ref, br(100, 0));
    assert_eq!(s.threads[1].thread_ref, br(200, 0));
}

#[test]
fn own_message_uses_recipient_as_peer() {
    let mut s = bob_session();
    s.add_thread_message(
        bob_pub(),
        ALICE_PUB,
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&bob_pub()),
            timestamp: now(),
            body: "My message".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 100,
            ext_index: 0,
        },
    );
    assert_eq!(s.threads.len(), 1);
    assert_eq!(s.threads[0].peer_ss58, util::ss58_short(&ALICE_PUB));
}

// ---------------------------------------------------------------------------
// DB persistence roundtrip
// ---------------------------------------------------------------------------

#[test]
fn db_roundtrip_then_new_message_same_thread() {
    let db = Db::open_in_memory(&BOB_SEED).unwrap();
    let mut s = Session::new(
        signing_from_seed(&BOB_SEED),
        Zeroizing::new(BOB_SEED),
        "ws://test".into(),
        ci(),
        db,
    );

    // Root message stored in DB
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "First".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 100,
            ext_index: 0,
        },
    );
    assert_eq!(s.threads.len(), 1);

    // Simulate restart
    s.threads.clear();
    s.load_from_db();
    assert_eq!(s.threads.len(), 1);

    // New message in same thread
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(100, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Second".into(),
            reply_to: br(100, 0),
            continues: br(0, 0),
            block_number: 200,
            ext_index: 0,
        },
    );

    assert_eq!(
        s.threads.len(),
        1,
        "After reload + new message: expected 1 thread, got {}",
        s.threads.len()
    );
    assert_eq!(s.threads[0].messages.len(), 2);
}

#[test]
fn db_roundtrip_own_sent_then_receive_same_thread() {
    let db = Db::open_in_memory(&BOB_SEED).unwrap();
    let mut s = Session::new(
        signing_from_seed(&BOB_SEED),
        Zeroizing::new(BOB_SEED),
        "ws://test".into(),
        ci(),
        db,
    );

    // Bob's own sent message (root)
    s.add_thread_message(
        bob_pub(),
        ALICE_PUB,
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&bob_pub()),
            timestamp: now(),
            body: "My msg".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 100,
            ext_index: 0,
        },
    );

    // Restart
    s.threads.clear();
    s.load_from_db();
    assert_eq!(s.threads.len(), 1);

    // Alice replies in same thread
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(100, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Reply".into(),
            reply_to: br(100, 0),
            continues: br(0, 0),
            block_number: 101,
            ext_index: 0,
        },
    );

    assert_eq!(
        s.threads.len(),
        1,
        "Expected 1 thread after own msg + reply, got {}",
        s.threads.len()
    );
}

// ---------------------------------------------------------------------------
// Message ordering
// ---------------------------------------------------------------------------

#[test]
fn messages_sorted_by_block_position() {
    let mut s = bob_session();
    // All in same thread (100,0)
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "First".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 100,
            ext_index: 0,
        },
    );
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(100, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Third".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 300,
            ext_index: 0,
        },
    );
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(100, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Second".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 200,
            ext_index: 0,
        },
    );
    assert_eq!(s.threads[0].messages[0].body, "First");
    assert_eq!(s.threads[0].messages[1].body, "Second");
    assert_eq!(s.threads[0].messages[2].body, "Third");
}

#[test]
fn duplicate_rejected() {
    let mut s = bob_session();
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Once".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 100,
            ext_index: 0,
        },
    );
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Duplicate".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 100,
            ext_index: 0,
        },
    );
    assert_eq!(s.threads[0].messages.len(), 1);
}

// ---------------------------------------------------------------------------
// Gap detection
// ---------------------------------------------------------------------------

#[test]
fn gap_detected_for_missing_reference() {
    let mut s = bob_session();
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "References missing block".into(),
            reply_to: br(500, 2),
            continues: br(0, 0),
            block_number: 600,
            ext_index: 0,
        },
    );
    assert!(s.threads[0].messages[0].has_gap);
}

#[test]
fn gap_resolved_after_loading_reference() {
    let mut s = bob_session();
    // Message referencing a missing block
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "References 500:2".into(),
            reply_to: br(500, 2),
            continues: br(0, 0),
            block_number: 600,
            ext_index: 0,
        },
    );
    assert!(s.threads[0].messages[0].has_gap);

    // Load the referenced message (in same thread)
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(600, 0),
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
    s.refresh_gaps(0);

    assert!(
        !s.threads[0].messages[1].has_gap,
        "Loaded message at 600 should no longer have gap"
    );
}

#[test]
fn no_gap_for_zero_references() {
    let mut s = bob_session();
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "First message".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 100,
            ext_index: 0,
        },
    );
    assert!(!s.threads[0].messages[0].has_gap);
}

// ---------------------------------------------------------------------------
// reply_to / continues correctness
// ---------------------------------------------------------------------------

#[test]
fn last_ref_returns_latest_message() {
    let mut s = bob_session();
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "First".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 100,
            ext_index: 0,
        },
    );
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(100, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Second".into(),
            reply_to: br(100, 0),
            continues: br(100, 0),
            block_number: 200,
            ext_index: 1,
        },
    );
    assert_eq!(s.threads[0].last_ref(), br(200, 1));
}

#[test]
fn my_last_ref_returns_own_latest() {
    let mut s = bob_session();
    // Alice starts thread
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "From Alice".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 100,
            ext_index: 0,
        },
    );
    // Bob replies in same thread
    s.add_thread_message(
        bob_pub(),
        ALICE_PUB,
        br(100, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&bob_pub()),
            timestamp: now(),
            body: "From Bob".into(),
            reply_to: br(100, 0),
            continues: br(0, 0),
            block_number: 101,
            ext_index: 0,
        },
    );
    // Alice again
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(100, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Alice again".into(),
            reply_to: br(101, 0),
            continues: br(100, 0),
            block_number: 102,
            ext_index: 0,
        },
    );

    assert_eq!(s.threads[0].last_ref(), br(102, 0));
    assert_eq!(s.threads[0].my_last_ref(), br(101, 0));
}

// ---------------------------------------------------------------------------
// Thread identity with explicit thread_ref
// ---------------------------------------------------------------------------

#[test]
fn alice_sends_two_messages_bob_offline_between() {
    let mut s = bob_session();

    // Alice sends root message
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Hi!".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 100,
            ext_index: 0,
        },
    );

    // Bob goes offline (simulate restart)
    s.threads.clear();
    s.load_from_db();
    assert_eq!(s.threads.len(), 1, "After restart, should have 1 thread");

    // Alice sent second message in same thread while Bob was offline
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(100, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Hidden Message".into(),
            reply_to: br(100, 0),
            continues: br(100, 0),
            block_number: 102,
            ext_index: 0,
        },
    );

    assert_eq!(s.threads.len(), 1, "Both messages in same thread");
    assert_eq!(s.threads[0].messages.len(), 2);
}

#[test]
fn zero_thread_ref_creates_new_thread() {
    // Two messages with thread_ref=ZERO = two separate threads
    let mut s = bob_session();

    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Thread 1".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 100,
            ext_index: 0,
        },
    );
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Thread 2".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 200,
            ext_index: 0,
        },
    );

    assert_eq!(s.threads.len(), 2, "Two root messages = two threads");
}

#[test]
fn cannot_message_self() {
    let mut s = bob_session();
    assert!(s.create_thread(bob_pub()).is_err());
}

// ---------------------------------------------------------------------------
// Verify ss58_short determinism
// ---------------------------------------------------------------------------

#[test]
fn ss58_short_deterministic() {
    let a = util::ss58_short(&ALICE_PUB);
    let b = util::ss58_short(&ALICE_PUB);
    assert_eq!(a, b);

    let c = util::ss58_short(&bob_pub());
    assert_ne!(a, c);
}

#[test]
fn ss58_decode_roundtrip_preserves_pubkey() {
    let address = util::ss58_from_pubkey(&ALICE_PUB);
    let decoded = util::ss58_decode(&address).unwrap();
    assert_eq!(decoded, ALICE_PUB);
    assert_eq!(util::ss58_short(&decoded), util::ss58_short(&ALICE_PUB));
}

// ---------------------------------------------------------------------------
// Own message with real recipient
// ---------------------------------------------------------------------------

#[test]
fn own_message_with_real_recipient_correct_thread() {
    let mut s = bob_session();

    // Alice starts thread
    s.add_thread_message(
        ALICE_PUB,
        bob_pub(),
        br(0, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&ALICE_PUB),
            timestamp: now(),
            body: "Hi from Alice".into(),
            reply_to: br(0, 0),
            continues: br(0, 0),
            block_number: 100,
            ext_index: 0,
        },
    );

    // Bob replies in same thread (unsealed recipient)
    s.add_thread_message(
        bob_pub(),
        ALICE_PUB,
        br(100, 0),
        NewMessage {
            sender_ss58: util::ss58_short(&bob_pub()),
            timestamp: now(),
            body: "Hi from Bob".into(),
            reply_to: br(100, 0),
            continues: br(0, 0),
            block_number: 101,
            ext_index: 0,
        },
    );

    assert_eq!(s.threads.len(), 1, "Both messages should be in 1 thread");
    assert_eq!(s.threads[0].messages.len(), 2);
    assert_eq!(s.threads[0].peer_ss58, util::ss58_short(&ALICE_PUB));
}
