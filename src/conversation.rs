use crate::types::{BlockRef, Pubkey};
use chrono::{DateTime, Utc};

/// A standalone message (public 0x01 or encrypted 0x02). Not threaded.
#[derive(Clone)]
pub struct InboxMessage {
    pub peer_ss58: String,
    pub timestamp: DateTime<Utc>,
    pub body: String,
    pub content_type: u8,
    pub is_mine: bool,
    pub block_number: u32,
    pub ext_index: u16,
}

/// A message in a threaded conversation (0x03 thread or 0x04 channel).
#[derive(Clone)]
pub struct ThreadMessage {
    pub sender_ss58: String,
    pub timestamp: DateTime<Utc>,
    pub body: String,
    pub is_mine: bool,
    pub reply_to: BlockRef,
    pub continues: BlockRef,
    pub block_number: u32,
    pub ext_index: u16,
    pub has_gap: bool,
}

/// Parameters for adding a new message to a thread, channel, or group.
pub struct NewMessage {
    pub sender_ss58: String,
    pub timestamp: DateTime<Utc>,
    pub body: String,
    pub reply_to: BlockRef,
    pub continues: BlockRef,
    pub block_number: u32,
    pub ext_index: u16,
}

// Shared DAG helpers for any Vec<ThreadMessage> (used by Thread and Channel).

pub fn last_ref(messages: &[ThreadMessage]) -> BlockRef {
    messages
        .last()
        .map(|m| BlockRef {
            block: m.block_number,
            index: m.ext_index,
        })
        .unwrap_or(BlockRef::ZERO)
}

pub fn my_last_ref(messages: &[ThreadMessage]) -> BlockRef {
    messages
        .iter()
        .rev()
        .find(|m| m.is_mine)
        .map(|m| BlockRef {
            block: m.block_number,
            index: m.ext_index,
        })
        .unwrap_or(BlockRef::ZERO)
}

pub fn gap_refs(messages: &[ThreadMessage]) -> Vec<BlockRef> {
    let mut refs = Vec::new();
    for m in messages {
        if m.has_gap {
            if m.reply_to != BlockRef::ZERO {
                refs.push(m.reply_to);
            }
            if m.continues != BlockRef::ZERO {
                refs.push(m.continues);
            }
        }
    }
    refs.sort_by(|a, b| (a.block, a.index).cmp(&(b.block, b.index)));
    refs.dedup();
    refs
}

/// An encrypted 1:1 threaded conversation (0x03 messages).
pub struct Thread {
    pub thread_ref: BlockRef,
    pub peer_ss58: String,
    pub peer_pubkey: Pubkey,
    pub messages: Vec<ThreadMessage>,
    pub draft: String,
    pub last_read: usize,
}

impl Thread {
    pub fn last_ref(&self) -> BlockRef {
        last_ref(&self.messages)
    }
    pub fn my_last_ref(&self) -> BlockRef {
        my_last_ref(&self.messages)
    }
    pub fn gap_refs(&self) -> Vec<BlockRef> {
        gap_refs(&self.messages)
    }
    pub fn unread(&self) -> usize {
        self.messages.len().saturating_sub(self.last_read)
    }
}

/// A discovered public channel (metadata only, not yet subscribed).
pub struct ChannelInfo {
    pub name: String,
    pub description: String,
    pub creator_ss58: String,
    pub channel_ref: BlockRef,
}

/// A subscribed public channel (0x14 messages).
pub struct Channel {
    pub name: String,
    pub description: String,
    pub creator_ss58: String,
    pub channel_ref: BlockRef,
    pub messages: Vec<ThreadMessage>,
    pub draft: String,
    pub last_read: usize,
}

impl Channel {
    pub fn last_ref(&self) -> BlockRef {
        last_ref(&self.messages)
    }
    pub fn my_last_ref(&self) -> BlockRef {
        my_last_ref(&self.messages)
    }
    pub fn gap_refs(&self) -> Vec<BlockRef> {
        gap_refs(&self.messages)
    }
    pub fn unread(&self) -> usize {
        self.messages.len().saturating_sub(self.last_read)
    }
}

/// An encrypted group (0x15 messages). Members are fixed at creation.
/// Auto-subscribed on discovery (you only see groups you're a member of).
pub struct Group {
    pub creator_pubkey: Pubkey,
    pub group_ref: BlockRef,
    pub members: Vec<Pubkey>,
    pub messages: Vec<ThreadMessage>,
    pub draft: String,
    pub last_read: usize,
}

impl Group {
    pub fn last_ref(&self) -> BlockRef {
        last_ref(&self.messages)
    }
    pub fn my_last_ref(&self) -> BlockRef {
        my_last_ref(&self.messages)
    }
    pub fn gap_refs(&self) -> Vec<BlockRef> {
        gap_refs(&self.messages)
    }
    pub fn unread(&self) -> usize {
        self.messages.len().saturating_sub(self.last_read)
    }
}
