use chrono::Utc;
use taolk::conversation::{Channel, Group, Thread, ThreadMessage, gap_refs, last_ref, my_last_ref};
use taolk::types::{BlockRef, Pubkey};

fn msg(block: u32, index: u16, is_mine: bool) -> ThreadMessage {
    ThreadMessage {
        sender_ss58: "X".into(),
        timestamp: Utc::now(),
        body: String::new(),
        is_mine,
        reply_to: BlockRef::ZERO,
        continues: BlockRef::ZERO,
        block_number: block,
        ext_index: index,
        has_gap: false,
    }
}

fn msg_with_gap(block: u32, reply_to: BlockRef) -> ThreadMessage {
    ThreadMessage {
        sender_ss58: "X".into(),
        timestamp: Utc::now(),
        body: String::new(),
        is_mine: false,
        reply_to,
        continues: BlockRef::ZERO,
        block_number: block,
        ext_index: 0,
        has_gap: true,
    }
}

#[test]
fn last_ref_empty_returns_zero() {
    let messages: Vec<ThreadMessage> = vec![];
    assert_eq!(last_ref(&messages), BlockRef::ZERO);
}

#[test]
fn last_ref_returns_final_message_position() {
    let messages = vec![msg(100, 0, false), msg(200, 3, true), msg(150, 1, false)];
    let r = last_ref(&messages);
    assert_eq!(r.block, 150);
    assert_eq!(r.index, 1);
}

#[test]
fn my_last_ref_skips_received_messages() {
    let messages = vec![
        msg(100, 0, true),
        msg(200, 0, false),
        msg(300, 0, false),
        msg(400, 0, true),
        msg(500, 0, false),
    ];
    let r = my_last_ref(&messages);
    assert_eq!(r.block, 400);
}

#[test]
fn my_last_ref_returns_zero_when_no_owned_messages() {
    let messages = vec![msg(100, 0, false), msg(200, 0, false)];
    assert_eq!(my_last_ref(&messages), BlockRef::ZERO);
}

#[test]
fn gap_refs_returns_unique_sorted_references() {
    let messages = vec![
        msg(100, 0, false),
        msg_with_gap(
            200,
            BlockRef {
                block: 50,
                index: 0,
            },
        ),
        msg_with_gap(
            300,
            BlockRef {
                block: 50,
                index: 0,
            },
        ),
        msg_with_gap(
            400,
            BlockRef {
                block: 75,
                index: 1,
            },
        ),
    ];
    let refs = gap_refs(&messages);
    assert_eq!(refs.len(), 2);
    assert_eq!(
        refs[0],
        BlockRef {
            block: 50,
            index: 0
        }
    );
    assert_eq!(
        refs[1],
        BlockRef {
            block: 75,
            index: 1
        }
    );
}

#[test]
fn gap_refs_ignores_non_gap_messages() {
    let messages = vec![msg(100, 0, false), msg(200, 0, false)];
    assert!(gap_refs(&messages).is_empty());
}

#[test]
fn gap_refs_skips_zero_reply_to() {
    let messages = vec![msg_with_gap(100, BlockRef::ZERO)];
    assert!(gap_refs(&messages).is_empty());
}

#[test]
fn thread_unread_counts_messages_after_last_read() {
    let thread = Thread {
        thread_ref: BlockRef::ZERO,
        peer_ss58: "peer".into(),
        peer_pubkey: Pubkey::ZERO,
        messages: vec![
            msg(1, 0, false),
            msg(2, 0, false),
            msg(3, 0, false),
            msg(4, 0, false),
            msg(5, 0, false),
        ],
        draft: String::new(),
        last_read: 2,
    };
    assert_eq!(thread.unread(), 3);
}

#[test]
fn thread_unread_zero_when_all_read() {
    let thread = Thread {
        thread_ref: BlockRef::ZERO,
        peer_ss58: "peer".into(),
        peer_pubkey: Pubkey::ZERO,
        messages: vec![msg(1, 0, false), msg(2, 0, false)],
        draft: String::new(),
        last_read: 2,
    };
    assert_eq!(thread.unread(), 0);
}

#[test]
fn thread_unread_saturates_when_last_read_exceeds_len() {
    let thread = Thread {
        thread_ref: BlockRef::ZERO,
        peer_ss58: "peer".into(),
        peer_pubkey: Pubkey::ZERO,
        messages: vec![msg(1, 0, false)],
        draft: String::new(),
        last_read: 999,
    };
    assert_eq!(thread.unread(), 0);
}

#[test]
fn channel_last_ref_uses_messages() {
    let channel = Channel {
        name: "n".into(),
        description: "d".into(),
        creator_ss58: "c".into(),
        channel_ref: BlockRef::ZERO,
        messages: vec![msg(10, 0, false), msg(20, 5, false)],
        draft: String::new(),
        last_read: 0,
    };
    assert_eq!(
        channel.last_ref(),
        BlockRef {
            block: 20,
            index: 5
        }
    );
    assert_eq!(channel.unread(), 2);
}

#[test]
fn group_last_ref_uses_messages() {
    let group = Group {
        creator_pubkey: Pubkey::ZERO,
        group_ref: BlockRef::ZERO,
        members: vec![Pubkey::ZERO],
        messages: vec![msg(10, 0, true), msg(20, 5, true)],
        draft: String::new(),
        last_read: 0,
    };
    assert_eq!(
        group.last_ref(),
        BlockRef {
            block: 20,
            index: 5
        }
    );
    assert_eq!(
        group.my_last_ref(),
        BlockRef {
            block: 20,
            index: 5
        }
    );
}
