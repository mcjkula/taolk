use crate::types::{BlockRef, Pubkey};

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
        timestamp: crate::types::Timestamp,
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
        timestamp: crate::types::Timestamp,
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
        timestamp: crate::types::Timestamp,
    },
    LockedOutbound {
        sender: Pubkey,
        block_number: u32,
        ext_index: u16,
        timestamp: crate::types::Timestamp,
        remark_bytes: samp::RemarkBytes,
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
        remark: samp::RemarkBytes,
    },
    GapsRefreshed,
    FeeEstimated {
        fee_display: String,
        fee_raw: Option<u128>,
    },
    BalanceUpdated(u128),
    ChainSnapshotRefreshed {
        info: crate::extrinsic::ChainInfo,
        token_symbol: String,
        token_decimals: u32,
    },
    GenesisMismatch,
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
