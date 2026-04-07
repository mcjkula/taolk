use crate::types::{BlockRef, Pubkey};

/// All `timestamp` fields in events are Unix seconds (block timestamp milliseconds / 1000).
pub enum Event {
    NewMessage {
        sender: Pubkey,
        content_type: u8,
        recipient: Pubkey,
        decrypted_body: Option<String>,
        thread_ref: BlockRef,
        reply_to: BlockRef,
        continues: BlockRef,
        block_number: u32,
        ext_index: u16,
        timestamp: u64,
    },
    NewChannelMessage {
        sender: Pubkey,
        sender_ss58: String,
        channel_ref: BlockRef,
        body: String,
        reply_to: BlockRef,
        continues: BlockRef,
        block_number: u32,
        ext_index: u16,
        timestamp: u64,
    },
    ChannelDiscovered {
        name: String,
        description: String,
        creator_ss58: String,
        channel_ref: BlockRef,
    },
    GroupDiscovered {
        creator_pubkey: Pubkey,
        group_ref: BlockRef,
        members: Vec<Pubkey>,
    },
    NewGroupMessage {
        sender: Pubkey,
        sender_ss58: String,
        group_ref: BlockRef,
        body: String,
        reply_to: BlockRef,
        continues: BlockRef,
        block_number: u32,
        ext_index: u16,
        timestamp: u64,
    },
    MessageSent,
    BlockUpdate(u64),
    FetchBlock {
        block_ref: BlockRef,
    },
    FetchChannelMirror {
        channel_ref: BlockRef,
    },
    SubmitRemark {
        remark: Vec<u8>,
    },
    GapsRefreshed,
    FeeEstimated {
        fee_display: String,
        fee_raw: Option<u128>,
    },
    BalanceUpdated(u128),
    ConnectionStatus(ConnState),
    Status(String),
    Error(String),
    CatchupComplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnState {
    Connected,
    Reconnecting { in_secs: u32 },
}
