use crate::types::{BlockRef, Pubkey};
use chrono::{DateTime, Utc};

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

impl ThreadMessage {
    pub fn from_new(msg: NewMessage, is_mine: bool, has_gap: bool) -> Self {
        Self {
            sender_ss58: msg.sender_ss58,
            timestamp: msg.timestamp,
            body: msg.body,
            is_mine,
            reply_to: msg.reply_to,
            continues: msg.continues,
            block_number: msg.block_number,
            ext_index: msg.ext_index,
            has_gap,
        }
    }
}

pub struct NewMessage {
    pub sender_ss58: String,
    pub timestamp: DateTime<Utc>,
    pub body: String,
    pub reply_to: BlockRef,
    pub continues: BlockRef,
    pub block_number: u32,
    pub ext_index: u16,
}

pub fn last_ref(messages: &[ThreadMessage]) -> BlockRef {
    messages
        .last()
        .map(|m| BlockRef::from_parts(m.block_number, m.ext_index))
        .unwrap_or(BlockRef::ZERO)
}

pub fn my_last_ref(messages: &[ThreadMessage]) -> BlockRef {
    messages
        .iter()
        .rev()
        .find(|m| m.is_mine)
        .map(|m| BlockRef::from_parts(m.block_number, m.ext_index))
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
    refs.sort_by_key(|r| (r.block().get(), r.index().get()));
    refs.dedup();
    refs
}

pub trait Conversation {
    fn messages(&self) -> &[ThreadMessage];
    fn last_read(&self) -> usize;

    fn last_ref(&self) -> BlockRef {
        last_ref(self.messages())
    }
    fn my_last_ref(&self) -> BlockRef {
        my_last_ref(self.messages())
    }
    fn gap_refs(&self) -> Vec<BlockRef> {
        gap_refs(self.messages())
    }
    fn unread(&self) -> usize {
        self.messages().len().saturating_sub(self.last_read())
    }
}

pub struct Thread {
    pub thread_ref: BlockRef,
    pub peer_ss58: String,
    pub peer_pubkey: Pubkey,
    pub messages: Vec<ThreadMessage>,
    pub draft: String,
    pub last_read: usize,
}

impl Conversation for Thread {
    fn messages(&self) -> &[ThreadMessage] {
        &self.messages
    }
    fn last_read(&self) -> usize {
        self.last_read
    }
}

pub struct ChannelInfo {
    pub name: String,
    pub description: String,
    pub creator_ss58: String,
    pub channel_ref: BlockRef,
}

pub struct Channel {
    pub name: String,
    pub description: String,
    pub creator_ss58: String,
    pub channel_ref: BlockRef,
    pub messages: Vec<ThreadMessage>,
    pub draft: String,
    pub last_read: usize,
}

impl Conversation for Channel {
    fn messages(&self) -> &[ThreadMessage] {
        &self.messages
    }
    fn last_read(&self) -> usize {
        self.last_read
    }
}

pub struct Group {
    pub creator_pubkey: Pubkey,
    pub group_ref: BlockRef,
    pub members: Vec<Pubkey>,
    pub messages: Vec<ThreadMessage>,
    pub draft: String,
    pub last_read: usize,
}

impl Conversation for Group {
    fn messages(&self) -> &[ThreadMessage] {
        &self.messages
    }
    fn last_read(&self) -> usize {
        self.last_read
    }
}
