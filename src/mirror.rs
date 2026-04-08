use std::collections::HashSet;
use std::sync::mpsc::Sender;

use samp::Remark;

use crate::chain;
use crate::error::ChainError;
use crate::event::Event;
use crate::reader::{self, RemarkSource};
use crate::secret::DecryptionKeys;
use crate::types::{BlockRef, Pubkey};

#[derive(serde::Deserialize)]
struct HealthResp {
    chain: String,
    ss58_prefix: u16,
}

#[derive(serde::Deserialize)]
struct Hint {
    #[serde(rename = "block")]
    b: u32,
    #[serde(rename = "index")]
    i: u16,
}

#[allow(clippy::too_many_arguments)]
pub async fn sync(
    mirror_urls: Vec<String>,
    node_url: &str,
    expected_chain: &samp::ChainName,
    expected_ss58_prefix: samp::Ss58Prefix,
    keys: &DecryptionKeys,
    my_pubkey: &Pubkey,
    subscribed_channels: Vec<BlockRef>,
    last_block: u64,
    tx: Sender<Event>,
) {
    let _ = tx.send(Event::Status("Catching up...".into()));
    if let Err(e) = sync_inner(
        mirror_urls,
        node_url,
        expected_chain,
        expected_ss58_prefix,
        keys,
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
    expected_chain: &samp::ChainName,
    expected_ss58_prefix: samp::Ss58Prefix,
    keys: &DecryptionKeys,
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

    let healthy = check_health_all(&client, &bases, expected_chain, expected_ss58_prefix).await;
    if healthy.is_empty() {
        let prefix = expected_ss58_prefix.get();
        return Err(ChainError::Http(format!(
            "no healthy mirrors serving '{}' (ss58 {prefix})",
            expected_chain.as_str()
        )));
    }

    let channel_hints = fetch_channel_directory_hints(&client, &healthy).await;
    let message_hints =
        fetch_message_hints(&client, &healthy, last_block, &subscribed_channels).await;

    resolve_channel_hints(node_url, channel_hints, tx).await;
    resolve_message_hints(node_url, message_hints, my_pubkey, keys, tx).await;

    let _ = tx.send(Event::Status("All caught up".into()));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn fetch_channel(
    mirror_urls: Vec<String>,
    node_url: &str,
    expected_chain: &samp::ChainName,
    expected_ss58_prefix: samp::Ss58Prefix,
    channel_ref: BlockRef,
    my_pubkey: &Pubkey,
    keys: &DecryptionKeys,
    tx: Sender<Event>,
) {
    let client = reqwest::Client::new();
    let bases: Vec<String> = mirror_urls
        .iter()
        .map(|u| u.trim_end_matches('/').to_string())
        .collect();
    let healthy = check_health_all(&client, &bases, expected_chain, expected_ss58_prefix).await;
    if healthy.is_empty() {
        let prefix = expected_ss58_prefix.get();
        let _ = tx.send(Event::Error(format!(
            "no healthy mirrors serving '{}' (ss58 {prefix})",
            expected_chain.as_str()
        )));
        return;
    }
    let (b, i) = (channel_ref.block().get(), channel_ref.index().get());
    let hints = fetch_per_channel_hints(&client, &healthy, b, i, 0).await;
    resolve_message_hints(node_url, hints, my_pubkey, keys, &tx).await;
    let _ = tx.send(Event::CatchupComplete);
}

async fn check_health_all(
    client: &reqwest::Client,
    bases: &[String],
    expected_chain: &samp::ChainName,
    expected_prefix: samp::Ss58Prefix,
) -> Vec<String> {
    let expected_chain_str = expected_chain.as_str().to_string();
    let expected_prefix_u16 = expected_prefix.get();
    let futures = bases.iter().map(|base| {
        let expected_chain_str = expected_chain_str.clone();
        async move {
            let resp: HealthResp = client
                .get(format!("{base}/v1/health"))
                .send()
                .await
                .ok()?
                .json()
                .await
                .ok()?;
            if resp.chain == expected_chain_str && resp.ss58_prefix == expected_prefix_u16 {
                Some(base.clone())
            } else {
                None
            }
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
            union.insert((h.b, h.i));
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
        for hint in fetch_per_channel_hints(
            client,
            bases,
            ch.block().get(),
            ch.index().get(),
            last_block,
        )
        .await
        {
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
                union.insert((h.b, h.i));
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
            union.insert((h.b, h.i));
        }
    }
    union
}

async fn resolve_message_hints(
    node_url: &str,
    hints: HashSet<(u32, u16)>,
    my_pubkey: &Pubkey,
    keys: &DecryptionKeys,
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
            reader::process_remark(&source, my_pubkey, keys, tx);
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
    let Remark::ChannelCreate { name, description } = &source.remark else {
        return;
    };
    let _ = tx.send(Event::ChannelDiscovered {
        name: name.as_str().to_string(),
        description: description.as_str().to_string(),
        creator_ss58: crate::util::ss58_short(&source.sender),
        channel_ref: source.at,
    });
}
