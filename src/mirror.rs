use std::sync::mpsc::Sender;

use samp::decode_remark;

use crate::error::ChainError;
use crate::event::Event;
use crate::reader::{RemarkSource, process_remark};
use crate::types::{BlockRef, Pubkey};

#[derive(serde::Deserialize)]
struct HealthResp {
    chain: String,
    ss58_prefix: u16,
}

#[derive(serde::Deserialize)]
struct ChannelResp {
    block: u32,
    index: u16,
    creator: String,
    name: String,
    description: String,
}

#[derive(serde::Deserialize)]
struct RemarkResp {
    block: u32,
    index: u16,
    sender: String,
    timestamp: u64,
    remark: String,
}

pub async fn sync(
    mirror_url: &str,
    expected_ss58_prefix: u16,
    seed: &[u8; 32],
    my_pubkey: &Pubkey,
    subscribed_channels: Vec<BlockRef>,
    last_block: u64,
    tx: Sender<Event>,
) {
    if let Err(e) = sync_inner(
        mirror_url,
        expected_ss58_prefix,
        seed,
        my_pubkey,
        subscribed_channels,
        last_block,
        &tx,
    )
    .await
    {
        let _ = tx.send(Event::Error(format!("Could not reach mirror: {e}")));
    }
    let _ = tx.send(Event::CatchupComplete);
}

async fn sync_inner(
    mirror_url: &str,
    expected_ss58_prefix: u16,
    seed: &[u8; 32],
    my_pubkey: &Pubkey,
    subscribed_channels: Vec<BlockRef>,
    last_block: u64,
    tx: &Sender<Event>,
) -> Result<(), ChainError> {
    let _ = tx.send(Event::Status("Catching up...".into()));
    let client = reqwest::Client::new();
    let base = mirror_url.trim_end_matches('/');

    let health: HealthResp = client
        .get(format!("{base}/v1/health"))
        .send()
        .await
        .map_err(|e| ChainError::Http(format!("health: {e}")))?
        .json()
        .await
        .map_err(|e| ChainError::Parse(format!("health json: {e}")))?;

    if health.ss58_prefix != expected_ss58_prefix {
        return Err(ChainError::MirrorChainMismatch {
            chain: health.chain,
            got: health.ss58_prefix,
            expected: expected_ss58_prefix,
        });
    }

    let channels: Vec<ChannelResp> = client
        .get(format!("{base}/v1/channels"))
        .send()
        .await
        .map_err(|e| ChainError::Http(format!("channels: {e}")))?
        .json()
        .await
        .map_err(|e| ChainError::Parse(format!("channels json: {e}")))?;

    for ch in &channels {
        let _ = tx.send(Event::ChannelDiscovered {
            name: ch.name.clone(),
            description: ch.description.clone(),
            creator_ss58: ch.creator.clone(),
            channel_ref: BlockRef {
                block: ch.block,
                index: ch.index,
            },
        });
    }

    for ch in &subscribed_channels {
        let (ch_block, ch_index) = (ch.block, ch.index);
        let remarks: Vec<RemarkResp> = client
            .get(format!(
                "{base}/v1/channels/{ch_block}/{ch_index}/messages?after={last_block}"
            ))
            .send()
            .await
            .map_err(|e| ChainError::Http(format!("channel messages: {e}")))?
            .json()
            .await
            .map_err(|e| ChainError::Parse(format!("channel messages json: {e}")))?;

        process_remarks(&remarks, my_pubkey, seed, tx);
    }

    for type_byte in [0x10u8, 0x11, 0x12, 0x15] {
        let label = match type_byte {
            0x10 => "public",
            0x11 => "encrypted",
            0x12 => "thread",
            0x15 => "group",
            _ => unreachable!(),
        };
        let remarks = fetch_remarks(&client, base, type_byte, last_block, label).await?;
        process_remarks(&remarks, my_pubkey, seed, tx);
    }

    let _ = tx.send(Event::Status("All caught up".into()));
    Ok(())
}

fn process_remarks(
    remarks: &[RemarkResp],
    my_pubkey: &Pubkey,
    seed: &[u8; 32],
    tx: &Sender<Event>,
) {
    for r in remarks {
        if let Some(source) = source_from_resp(r) {
            process_remark(&source, my_pubkey, seed, tx);
        }
    }
}

fn source_from_resp(r: &RemarkResp) -> Option<RemarkSource> {
    let bytes = hex::decode(&r.remark).ok()?;
    let remark = decode_remark(&bytes).ok()?;
    let sender = crate::util::pubkey_from_ss58(&r.sender)?;
    Some(RemarkSource {
        sender,
        remark,
        block: BlockRef {
            block: r.block,
            index: r.index,
        },
        timestamp_secs: r.timestamp,
    })
}

async fn fetch_remarks(
    client: &reqwest::Client,
    base: &str,
    type_byte: u8,
    after: u64,
    label: &str,
) -> Result<Vec<RemarkResp>, ChainError> {
    client
        .get(format!(
            "{base}/v1/remarks?type=0x{type_byte:02x}&after={after}"
        ))
        .send()
        .await
        .map_err(|e| ChainError::Http(format!("{label}: {e}")))?
        .json()
        .await
        .map_err(|e| ChainError::Parse(format!("{label} json: {e}")))
}

pub async fn fetch_channel(
    mirror_url: &str,
    channel_ref: BlockRef,
    my_pubkey: &Pubkey,
    seed: &[u8; 32],
    tx: Sender<Event>,
) {
    let base = mirror_url.trim_end_matches('/');
    let client = reqwest::Client::new();
    let (ch_block, ch_index) = (channel_ref.block, channel_ref.index);

    let remarks: Vec<RemarkResp> = match client
        .get(format!(
            "{base}/v1/channels/{ch_block}/{ch_index}/messages?after=0"
        ))
        .send()
        .await
    {
        Ok(resp) => match resp.json().await {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(Event::Error(format!("Could not load messages: {e}")));
                return;
            }
        },
        Err(e) => {
            let _ = tx.send(Event::Error(format!("Could not load messages: {e}")));
            return;
        }
    };

    process_remarks(&remarks, my_pubkey, seed, &tx);
}
