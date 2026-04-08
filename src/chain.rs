use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::sync::mpsc::Sender;
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use zeroize::Zeroizing;

use crate::error::ChainError;
use crate::event::{ConnState, Event};
use crate::extrinsic::ChainInfo;
use crate::reader;
use crate::types::Pubkey;

pub async fn fetch_and_process_extrinsic(
    node_url: &str,
    block_num: u32,
    ext_index: u16,
    my_pubkey: Pubkey,
    seed: Zeroizing<[u8; 32]>,
    chain_info: ChainInfo,
    tx: Sender<Event>,
) {
    let result = fetch_extrinsic_inner(
        node_url,
        block_num,
        ext_index,
        &my_pubkey,
        &seed,
        &chain_info,
        &tx,
    )
    .await;
    if let Err(e) = result {
        let _ = tx.send(Event::Error(format!(
            "Load block {block_num}:{ext_index}: {e}"
        )));
    }
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

async fn fetch_extrinsic_inner(
    node_url: &str,
    block_num: u32,
    ext_index: u16,
    my_pubkey: &Pubkey,
    seed: &[u8; 32],
    chain_info: &ChainInfo,
    tx: &Sender<Event>,
) -> Result<(), ChainError> {
    let (ws, _) = connect_async(node_url)
        .await
        .map_err(|e| ChainError::Connect(e.to_string()))?;

    let (mut write, mut read) = ws.split();

    let hash_req = json!({
        "jsonrpc": "2.0", "id": 1,
        "method": "chain_getBlockHash", "params": [block_num]
    });
    write
        .send(WsMessage::Text(hash_req.to_string().into()))
        .await
        .map_err(|e| ChainError::Send(e.to_string()))?;

    let hash_resp = next_text(&mut read).await?;
    let v: Value =
        serde_json::from_str(&hash_resp).map_err(|e| ChainError::Parse(e.to_string()))?;
    let block_hash = v["result"]
        .as_str()
        .ok_or(ChainError::MissingField("block hash"))?
        .to_string();

    let block_req = json!({
        "jsonrpc": "2.0", "id": 2,
        "method": "chain_getBlock", "params": [block_hash]
    });
    write
        .send(WsMessage::Text(block_req.to_string().into()))
        .await
        .map_err(|e| ChainError::Send(e.to_string()))?;

    let block_resp = next_text(&mut read).await?;
    let v: Value =
        serde_json::from_str(&block_resp).map_err(|e| ChainError::Parse(e.to_string()))?;
    let block = v["result"]
        .get("block")
        .ok_or(ChainError::MissingField("block"))?;

    if let Some(exts) = block["extrinsics"].as_array() {
        let block_ts = reader::extract_block_timestamp(exts);
        if let Some(ext) = exts.get(usize::from(ext_index))
            && let Some(ext_hex) = ext.as_str()
        {
            let ctx = reader::ReadContext {
                my_pubkey,
                seed,
                tx,
                chain_info,
            };
            reader::read_extrinsic(ext_hex, &ctx, block_num, ext_index, block_ts);
        }
    }
    Ok(())
}

pub async fn subscribe_blocks(
    node_url: &str,
    my_pubkey: Pubkey,
    seed: Zeroizing<[u8; 32]>,
    chain_info: ChainInfo,
    tx: Sender<Event>,
) {
    let mut delay: u32 = 1;
    loop {
        let _ = tx.send(Event::ConnectionStatus(ConnState::Connected));
        match run_subscription(node_url, &my_pubkey, &seed, &chain_info, &tx).await {
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
    chain_info: &ChainInfo,
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
                    chain_info,
                };
                reader::read_block(block, &ctx);
            }
        }
    }

    Err(ChainError::WsClosed)
}
