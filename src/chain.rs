use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc::Sender;
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use zeroize::Zeroizing;

use crate::error::ChainError;
use crate::event::{ConnState, Event};
use crate::reader;
use crate::types::Pubkey;

const PIPELINE_WINDOW: usize = 32;

pub struct BlockData {
    pub extrinsics: Vec<String>,
    pub timestamp_ms: u64,
}

enum BlockState {
    WaitingForHash { block_num: u32 },
    WaitingForBlock { block_num: u32 },
}

pub async fn fetch_blocks(
    node_url: &str,
    block_nums: &[u32],
) -> Result<HashMap<u32, BlockData>, ChainError> {
    if block_nums.is_empty() {
        return Ok(HashMap::new());
    }

    let (ws, _) = connect_async(node_url)
        .await
        .map_err(|e| ChainError::Connect(e.to_string()))?;
    let (mut write, mut read) = ws.split();

    let mut in_flight: BTreeMap<u64, BlockState> = BTreeMap::new();
    let mut out: HashMap<u32, BlockData> = HashMap::new();
    let mut next_input: usize = 0;
    let mut request_id: u64 = 0;
    let total = block_nums.len();

    while out.len() < total {
        while in_flight.len() < PIPELINE_WINDOW && next_input < total {
            request_id += 1;
            let block_num = block_nums[next_input];
            let req = json!({
                "jsonrpc": "2.0", "id": request_id,
                "method": "chain_getBlockHash", "params": [block_num]
            });
            write
                .send(WsMessage::Text(req.to_string().into()))
                .await
                .map_err(|e| ChainError::Send(e.to_string()))?;
            in_flight.insert(request_id, BlockState::WaitingForHash { block_num });
            next_input += 1;
        }

        if in_flight.is_empty() {
            break;
        }

        let text = next_text(&mut read).await?;
        let v: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let Some(resp_id) = v["id"].as_u64() else {
            continue;
        };
        let Some(state) = in_flight.remove(&resp_id) else {
            continue;
        };

        if v.get("error").is_some() {
            continue;
        }

        match state {
            BlockState::WaitingForHash { block_num } => {
                if let Some(hash) = v["result"].as_str() {
                    request_id += 1;
                    let req = json!({
                        "jsonrpc": "2.0", "id": request_id,
                        "method": "chain_getBlock", "params": [hash]
                    });
                    write
                        .send(WsMessage::Text(req.to_string().into()))
                        .await
                        .map_err(|e| ChainError::Send(e.to_string()))?;
                    in_flight.insert(request_id, BlockState::WaitingForBlock { block_num });
                }
            }
            BlockState::WaitingForBlock { block_num } => {
                if let Some(block) = v["result"].get("block")
                    && let Some(exts) = block["extrinsics"].as_array()
                {
                    let extrinsics: Vec<String> = exts
                        .iter()
                        .filter_map(|e| e.as_str().map(String::from))
                        .collect();
                    let timestamp_ms = reader::extract_block_timestamp(exts);
                    out.insert(
                        block_num,
                        BlockData {
                            extrinsics,
                            timestamp_ms,
                        },
                    );
                }
            }
        }
    }

    Ok(out)
}

pub async fn fetch_block(node_url: &str, block_num: u32) -> Result<BlockData, ChainError> {
    let mut blocks = fetch_blocks(node_url, &[block_num]).await?;
    blocks.remove(&block_num).ok_or(ChainError::BadShape)
}

pub async fn fetch_and_process_extrinsic(
    node_url: &str,
    block_num: u32,
    ext_index: u16,
    my_pubkey: Pubkey,
    seed: Zeroizing<[u8; 32]>,
    tx: Sender<Event>,
) {
    let result =
        fetch_extrinsic_inner(node_url, block_num, ext_index, &my_pubkey, &seed, &tx).await;
    if let Err(e) = result {
        let _ = tx.send(Event::Error(format!(
            "Load block {block_num}:{ext_index}: {e}"
        )));
    }
}

async fn fetch_extrinsic_inner(
    node_url: &str,
    block_num: u32,
    ext_index: u16,
    my_pubkey: &Pubkey,
    seed: &[u8; 32],
    tx: &Sender<Event>,
) -> Result<(), ChainError> {
    let block = fetch_block(node_url, block_num).await?;
    if let Some(ext_hex) = block.extrinsics.get(usize::from(ext_index)) {
        let ctx = reader::ReadContext {
            my_pubkey,
            seed,
            tx,
        };
        reader::read_extrinsic(ext_hex, &ctx, block_num, ext_index, block.timestamp_ms);
    }
    Ok(())
}

async fn next_text(
    ws: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
) -> Result<String, ChainError> {
    loop {
        match ws.next().await {
            Some(Ok(WsMessage::Text(t))) => return Ok(t.to_string()),
            Some(Ok(WsMessage::Ping(_) | WsMessage::Pong(_))) => continue,
            Some(Ok(_)) => return Err(ChainError::BadShape),
            Some(Err(e)) => return Err(ChainError::Ws(e.to_string())),
            None => return Err(ChainError::WsClosed),
        }
    }
}

pub async fn subscribe_blocks(
    node_url: &str,
    my_pubkey: Pubkey,
    seed: Zeroizing<[u8; 32]>,
    tx: Sender<Event>,
) {
    let mut delay: u32 = 1;
    loop {
        let _ = tx.send(Event::ConnectionStatus(ConnState::Connected));
        match run_subscription(node_url, &my_pubkey, &seed, &tx).await {
            Ok(()) => return,
            Err(e) => {
                let _ = tx.send(Event::Status(format!("Chain disconnected: {e}")));
            }
        }
        for remaining in (1..=delay).rev() {
            let _ = tx.send(Event::ConnectionStatus(ConnState::Reconnecting {
                in_secs: remaining,
            }));
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        delay = (delay * 2).min(60);
    }
}

async fn run_subscription(
    node_url: &str,
    my_pubkey: &Pubkey,
    seed: &[u8; 32],
    tx: &Sender<Event>,
) -> Result<(), ChainError> {
    let (mut ws, _) = connect_async(node_url)
        .await
        .map_err(|e| ChainError::Connect(e.to_string()))?;

    let sub_msg = json!({
        "jsonrpc": "2.0", "id": 1,
        "method": "chain_subscribeNewHeads", "params": []
    });
    ws.send(WsMessage::Text(sub_msg.to_string().into()))
        .await
        .map_err(|e| ChainError::Send(e.to_string()))?;

    let mut request_id: u64 = 100;

    while let Some(frame) = ws.next().await {
        let msg = match frame {
            Ok(m) => m,
            Err(e) => return Err(ChainError::Ws(e.to_string())),
        };
        let text = match &msg {
            WsMessage::Text(t) => t.to_string(),
            _ => continue,
        };

        let v: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(number_hex) = v["params"]["result"]["number"].as_str() {
            let block_num =
                u64::from_str_radix(number_hex.trim_start_matches("0x"), 16).unwrap_or(0);
            let _ = tx.send(Event::BlockUpdate(block_num));

            request_id += 1;
            let hash_req = json!({
                "jsonrpc": "2.0", "id": request_id,
                "method": "chain_getBlockHash", "params": [block_num]
            });
            ws.send(WsMessage::Text(hash_req.to_string().into()))
                .await
                .map_err(|e| ChainError::Send(e.to_string()))?;
            continue;
        }

        if let Some(result) = v.get("result") {
            if let Some(block_hash) = result.as_str() {
                request_id += 1;
                let block_req = json!({
                    "jsonrpc": "2.0", "id": request_id,
                    "method": "chain_getBlock", "params": [block_hash]
                });
                ws.send(WsMessage::Text(block_req.to_string().into()))
                    .await
                    .map_err(|e| ChainError::Send(e.to_string()))?;
            } else if let Some(block) = result.get("block") {
                let ctx = reader::ReadContext {
                    my_pubkey,
                    seed,
                    tx,
                };
                reader::read_block(block, &ctx);
            }
        }
    }

    Err(ChainError::WsClosed)
}
