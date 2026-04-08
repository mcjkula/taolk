use std::collections::HashSet;
use std::sync::mpsc::Sender;

use samp::{ContentType, decode_channel_create};

use crate::chain;
use crate::error::ChainError;
use crate::event::Event;
use crate::reader::{self, RemarkSource};
use crate::types::{BlockRef, Pubkey};

#[derive(serde::Deserialize)]
struct HealthResp {
    ss58_prefix: u16,
}

#[derive(serde::Deserialize)]
struct Hint {
    block: u32,
    index: u16,
}

#[allow(clippy::too_many_arguments)]
pub async fn sync(
    mirror_urls: Vec<String>,
    node_url: &str,
    expected_ss58_prefix: u16,
    seed: &[u8; 32],
    my_pubkey: &Pubkey,
    subscribed_channels: Vec<BlockRef>,
    last_block: u64,
    tx: Sender<Event>,
) {
    let _ = tx.send(Event::Status("Catching up...".into()));
    if let Err(e) = sync_inner(
        mirror_urls,
        node_url,
        expected_ss58_prefix,
        seed,
        my_pubkey,
        subscribed_channels,
        last_block,
        &tx,
    )
    .await
    {
        let _ = tx.send(Event::Error(format!("Could not reach mirrors: {e}")));
    }
    let _ = tx.send(Event::CatchupComplete);
}

#[allow(clippy::too_many_arguments)]
async fn sync_inner(
    mirror_urls: Vec<String>,
    node_url: &str,
    expected_ss58_prefix: u16,
    seed: &[u8; 32],
    my_pubkey: &Pubkey,
    subscribed_channels: Vec<BlockRef>,
    last_block: u64,
    tx: &Sender<Event>,
) -> Result<(), ChainError> {
    let client = reqwest::Client::new();
    let bases: Vec<String> = mirror_urls
        .iter()
        .map(|u| u.trim_end_matches('/').to_string())
        .collect();

    let healthy = check_health_all(&client, &bases, expected_ss58_prefix).await;
    if healthy.is_empty() {
        return Err(ChainError::Http("no healthy mirrors".into()));
    }

    let channel_hints = fetch_channel_directory_hints(&client, &healthy).await;
    let message_hints =
        fetch_message_hints(&client, &healthy, last_block, &subscribed_channels).await;

    resolve_channel_hints(node_url, channel_hints, tx).await;
    resolve_message_hints(node_url, message_hints, my_pubkey, seed, tx).await;

    let _ = tx.send(Event::Status("All caught up".into()));
    Ok(())
}

pub async fn fetch_channel(
    mirror_urls: Vec<String>,
    node_url: &str,
    channel_ref: BlockRef,
    my_pubkey: &Pubkey,
    seed: &[u8; 32],
    tx: Sender<Event>,
) {
    let client = reqwest::Client::new();
    let bases: Vec<String> = mirror_urls
        .iter()
        .map(|u| u.trim_end_matches('/').to_string())
        .collect();
    let (b, i) = (channel_ref.block, channel_ref.index);
    let hints = fetch_per_channel_hints(&client, &bases, b, i, 0).await;
    resolve_message_hints(node_url, hints, my_pubkey, seed, &tx).await;
}

async fn check_health_all(
    client: &reqwest::Client,
    bases: &[String],
    expected_prefix: u16,
) -> Vec<String> {
    let futures = bases.iter().map(|base| async move {
        let resp: HealthResp = client
            .get(format!("{base}/v1/health"))
            .send()
            .await
            .ok()?
            .json()
            .await
            .ok()?;
        if resp.ss58_prefix == expected_prefix {
            Some(base.clone())
        } else {
            None
        }
    });
    futures_util::future::join_all(futures)
        .await
        .into_iter()
        .flatten()
        .collect()
}

async fn fetch_channel_directory_hints(
    client: &reqwest::Client,
    bases: &[String],
) -> HashSet<(u32, u16)> {
    let futures = bases.iter().map(|base| async move {
        let hints: Vec<Hint> = client
            .get(format!("{base}/v1/channels"))
            .send()
            .await
            .ok()?
            .json()
            .await
            .ok()?;
        Some(hints)
    });
    let mut union = HashSet::new();
    for hints in futures_util::future::join_all(futures)
        .await
        .into_iter()
        .flatten()
    {
        for h in hints {
            union.insert((h.block, h.index));
        }
    }
    union
}

async fn fetch_message_hints(
    client: &reqwest::Client,
    bases: &[String],
    last_block: u64,
    subscribed_channels: &[BlockRef],
) -> HashSet<(u32, u16)> {
    let mut union = HashSet::new();

    for ch in subscribed_channels {
        for hint in fetch_per_channel_hints(client, bases, ch.block, ch.index, last_block).await {
            union.insert(hint);
        }
    }

    for type_byte in [0x10u8, 0x11, 0x12, 0x15] {
        let futures = bases.iter().map(|base| async move {
            let hints: Vec<Hint> = client
                .get(format!(
                    "{base}/v1/remarks?type=0x{type_byte:02x}&after={last_block}"
                ))
                .send()
                .await
                .ok()?
                .json()
                .await
                .ok()?;
            Some(hints)
        });
        for hints in futures_util::future::join_all(futures)
            .await
            .into_iter()
            .flatten()
        {
            for h in hints {
                union.insert((h.block, h.index));
            }
        }
    }

    union
}

async fn fetch_per_channel_hints(
    client: &reqwest::Client,
    bases: &[String],
    ch_block: u32,
    ch_index: u16,
    last_block: u64,
) -> HashSet<(u32, u16)> {
    let futures = bases.iter().map(|base| async move {
        let hints: Vec<Hint> = client
            .get(format!(
                "{base}/v1/channels/{ch_block}/{ch_index}/messages?after={last_block}"
            ))
            .send()
            .await
            .ok()?
            .json()
            .await
            .ok()?;
        Some(hints)
    });
    let mut union = HashSet::new();
    for hints in futures_util::future::join_all(futures)
        .await
        .into_iter()
        .flatten()
    {
        for h in hints {
            union.insert((h.block, h.index));
        }
    }
    union
}

async fn resolve_message_hints(
    node_url: &str,
    hints: HashSet<(u32, u16)>,
    my_pubkey: &Pubkey,
    seed: &[u8; 32],
    tx: &Sender<Event>,
) {
    if hints.is_empty() {
        return;
    }
    let block_nums: Vec<u32> = hints
        .iter()
        .map(|&(b, _)| b)
        .collect::<HashSet<u32>>()
        .into_iter()
        .collect();
    let blocks = match chain::fetch_blocks(node_url, &block_nums).await {
        Ok(b) => b,
        Err(e) => {
            let _ = tx.send(Event::Error(format!("Resolve hints: {e}")));
            return;
        }
    };
    for (block_num, ext_index) in hints {
        let Some(block) = blocks.get(&block_num) else {
            continue;
        };
        let Some(ext_hex) = block.extrinsics.get(usize::from(ext_index)) else {
            continue;
        };
        if let Some(source) =
            reader::source_from_extrinsic(ext_hex, block_num, ext_index, block.timestamp_ms)
        {
            reader::process_remark(&source, my_pubkey, seed, tx);
        }
    }
}

async fn resolve_channel_hints(node_url: &str, hints: HashSet<(u32, u16)>, tx: &Sender<Event>) {
    if hints.is_empty() {
        return;
    }
    let block_nums: Vec<u32> = hints
        .iter()
        .map(|&(b, _)| b)
        .collect::<HashSet<u32>>()
        .into_iter()
        .collect();
    let blocks = match chain::fetch_blocks(node_url, &block_nums).await {
        Ok(b) => b,
        Err(_) => return,
    };
    for (block_num, ext_index) in hints {
        let Some(block) = blocks.get(&block_num) else {
            continue;
        };
        let Some(ext_hex) = block.extrinsics.get(usize::from(ext_index)) else {
            continue;
        };
        let Some(source) =
            reader::source_from_extrinsic(ext_hex, block_num, ext_index, block.timestamp_ms)
        else {
            continue;
        };
        emit_channel_create(&source, tx);
    }
}

fn emit_channel_create(source: &RemarkSource, tx: &Sender<Event>) {
    if !matches!(source.remark.content_type, ContentType::ChannelCreate) {
        return;
    }
    let Ok((name, description)) = decode_channel_create(&source.remark.content) else {
        return;
    };
    let _ = tx.send(Event::ChannelDiscovered {
        name: name.to_string(),
        description: description.to_string(),
        creator_ss58: crate::util::ss58_short(&source.sender),
        channel_ref: source.block,
    });
}
