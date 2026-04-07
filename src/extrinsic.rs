use blake2::Digest;
use futures_util::{SinkExt, StreamExt};
use parity_scale_codec::{Compact, Encode};
use schnorrkel::signing_context;
use serde_json::{Value, json};
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

use crate::metadata::AccountInfoLayout;
use crate::types::Pubkey;

const PALLET_SYSTEM: u8 = 0x00;
const CALL_REMARK_WITH_EVENT: u8 = 0x07;
const ERA_IMMORTAL: u8 = 0x00;
const METADATA_HASH_DISABLED: u8 = 0x00;
const EXT_VERSION_SIGNED: u8 = 0x84;
const ADDR_TYPE_ID: u8 = 0x00;
const SIG_TYPE_SR25519: u8 = 0x01;

/// Chain parameters fetched once at startup.
#[derive(Clone)]
pub struct ChainInfo {
    pub genesis_hash: [u8; 32],
    pub spec_version: u32,
    pub tx_version: u32,
    pub account_info_layout: AccountInfoLayout,
}

/// Fetch genesis hash, runtime version, and `System.Account` layout from the chain.
pub async fn fetch_chain_info(node_url: &str) -> Result<ChainInfo, String> {
    let (mut ws, _) = connect_async(node_url)
        .await
        .map_err(|e| format!("connect: {e}"))?;

    // Genesis hash
    let req = json!({"jsonrpc":"2.0","id":1,"method":"chain_getBlockHash","params":[0]});
    ws.send(WsMessage::Text(req.to_string().into()))
        .await
        .map_err(|e| format!("send: {e}"))?;
    let genesis_hash = read_text_result(&mut ws).await?;
    let genesis_bytes = hex_to_32(genesis_hash.as_str().ok_or("no genesis hash")?)?;

    // Runtime version
    let req = json!({"jsonrpc":"2.0","id":2,"method":"state_getRuntimeVersion","params":[]});
    ws.send(WsMessage::Text(req.to_string().into()))
        .await
        .map_err(|e| format!("send: {e}"))?;
    let rv = read_text_result(&mut ws).await?;
    let spec_version = rv["specVersion"].as_u64().ok_or("no specVersion")? as u32;
    let tx_version = rv["transactionVersion"].as_u64().ok_or("no txVersion")? as u32;

    // Runtime metadata -> System.Account layout
    let req = json!({"jsonrpc":"2.0","id":3,"method":"state_getMetadata","params":[]});
    ws.send(WsMessage::Text(req.to_string().into()))
        .await
        .map_err(|e| format!("send: {e}"))?;
    let metadata_result = read_text_result(&mut ws).await?;
    let metadata_hex = metadata_result
        .as_str()
        .ok_or("state_getMetadata returned no result")?;
    let metadata_bytes = hex::decode(metadata_hex.trim_start_matches("0x"))
        .map_err(|e| format!("metadata hex decode: {e}"))?;
    let account_info_layout = AccountInfoLayout::from_runtime_metadata(&metadata_bytes)
        .map_err(|e| format!("parse runtime metadata: {e}"))?;

    Ok(ChainInfo {
        genesis_hash: genesis_bytes,
        spec_version,
        tx_version,
        account_info_layout,
    })
}

/// Build a signed extrinsic for system.remark_with_event(remark).
pub fn build_remark_extrinsic(
    remark: &[u8],
    keypair: &schnorrkel::Keypair,
    nonce: u32,
    chain_info: &ChainInfo,
) -> Vec<u8> {
    let account_id = keypair.public.to_bytes();

    let mut call_data = Vec::new();
    call_data.push(PALLET_SYSTEM);
    call_data.push(CALL_REMARK_WITH_EVENT);
    Compact(remark.len() as u32).encode_to(&mut call_data);
    call_data.extend_from_slice(remark);

    let tip: u8 = 0x00;

    // Signing payload: call || extensions || implicit (spec, tx, genesis, block_hash, metadata_hash)
    let mut signing_payload = Vec::new();
    signing_payload.extend_from_slice(&call_data);
    signing_payload.push(ERA_IMMORTAL);
    Compact(nonce).encode_to(&mut signing_payload);
    signing_payload.push(tip);
    signing_payload.push(METADATA_HASH_DISABLED);
    signing_payload.extend_from_slice(&chain_info.spec_version.to_le_bytes());
    signing_payload.extend_from_slice(&chain_info.tx_version.to_le_bytes());
    signing_payload.extend_from_slice(&chain_info.genesis_hash);
    signing_payload.extend_from_slice(&chain_info.genesis_hash);
    signing_payload.push(0x00); // CheckMetadataHash additional_signed: None

    let to_sign = if signing_payload.len() > 256 {
        let mut hasher = blake2::Blake2b::<blake2::digest::typenum::U32>::new();
        hasher.update(&signing_payload);
        hasher.finalize().to_vec()
    } else {
        signing_payload
    };

    let ctx = signing_context(b"substrate");
    let signature = keypair.sign(ctx.bytes(&to_sign));

    let mut extrinsic_payload = Vec::new();
    extrinsic_payload.push(EXT_VERSION_SIGNED);
    extrinsic_payload.push(ADDR_TYPE_ID);
    extrinsic_payload.extend_from_slice(&account_id);
    extrinsic_payload.push(SIG_TYPE_SR25519);
    extrinsic_payload.extend_from_slice(&signature.to_bytes());
    extrinsic_payload.push(ERA_IMMORTAL);
    Compact(nonce).encode_to(&mut extrinsic_payload);
    extrinsic_payload.push(tip);
    extrinsic_payload.push(METADATA_HASH_DISABLED);
    extrinsic_payload.extend_from_slice(&call_data);

    // Wrap with compact length prefix
    let mut full = Vec::new();
    Compact(extrinsic_payload.len() as u32).encode_to(&mut full);
    full.extend_from_slice(&extrinsic_payload);

    full
}

async fn refresh_signing_params(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    base: &ChainInfo,
) -> Result<ChainInfo, String> {
    let req = json!({"jsonrpc":"2.0","id":91,"method":"chain_getBlockHash","params":[0]});
    ws.send(WsMessage::Text(req.to_string().into()))
        .await
        .map_err(|e| format!("send: {e}"))?;
    let g = read_text_result(ws).await?;
    let genesis_hash = hex_to_32(g.as_str().ok_or("no genesis hash")?)?;

    let req = json!({"jsonrpc":"2.0","id":92,"method":"state_getRuntimeVersion","params":[]});
    ws.send(WsMessage::Text(req.to_string().into()))
        .await
        .map_err(|e| format!("send: {e}"))?;
    let rv = read_text_result(ws).await?;
    let spec_version = rv["specVersion"].as_u64().ok_or("no specVersion")? as u32;
    let tx_version = rv["transactionVersion"].as_u64().ok_or("no txVersion")? as u32;

    Ok(ChainInfo {
        genesis_hash,
        spec_version,
        tx_version,
        account_info_layout: base.account_info_layout.clone(),
    })
}

async fn read_text_result(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Result<Value, String> {
    loop {
        match ws.next().await {
            Some(Ok(WsMessage::Text(t))) => {
                let v: Value =
                    serde_json::from_str(t.as_ref()).map_err(|e| format!("parse: {e}"))?;
                if let Some(err) = v.get("error") {
                    return Err(format!("RPC error: {err}"));
                }
                return Ok(v["result"].clone());
            }
            Some(Ok(WsMessage::Ping(_) | WsMessage::Pong(_))) => continue,
            Some(Err(e)) => return Err(format!("ws: {e}")),
            None => return Err("connection closed".into()),
            _ => continue,
        }
    }
}

/// Read the next JSON-RPC message from WebSocket, returning the full JSON Value.
async fn read_text_result_raw(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Result<Value, String> {
    loop {
        match ws.next().await {
            Some(Ok(WsMessage::Text(t))) => {
                return serde_json::from_str(t.as_ref()).map_err(|e| format!("parse: {e}"));
            }
            Some(Ok(WsMessage::Ping(_) | WsMessage::Pong(_))) => continue,
            Some(Err(e)) => return Err(format!("ws: {e}")),
            None => return Err("connection closed".into()),
            _ => continue,
        }
    }
}

/// Estimate the fee for a remark extrinsic via TransactionPaymentApi runtime call.
/// Fetches the nonce, builds the extrinsic, and calls state_call on one connection.
pub async fn estimate_fee(
    node_url: &str,
    remark: &[u8],
    keypair: &schnorrkel::Keypair,
    ss58: &str,
    chain_info: &ChainInfo,
) -> Result<u128, String> {
    let (mut ws, _) = connect_async(node_url)
        .await
        .map_err(|e| format!("connect: {e}"))?;

    let chain_info = refresh_signing_params(&mut ws, chain_info).await?;

    let nonce_req =
        json!({"jsonrpc":"2.0","id":1,"method":"system_accountNextIndex","params":[ss58]});
    ws.send(WsMessage::Text(nonce_req.to_string().into()))
        .await
        .map_err(|e| format!("send: {e}"))?;
    let nonce_result = read_text_result(&mut ws).await?;
    let nonce = nonce_result
        .as_u64()
        .map(|n| n as u32)
        .ok_or_else(|| format!("unexpected nonce response: {nonce_result}"))?;

    let ext = build_remark_extrinsic(remark, keypair, nonce, &chain_info);

    // 3. Call TransactionPaymentApi_query_info
    let mut params = Vec::new();
    params.extend_from_slice(&ext);
    (ext.len() as u32).encode_to(&mut params);

    let params_hex = format!("0x{}", hex::encode(&params));
    let req = json!({
        "jsonrpc": "2.0", "id": 2,
        "method": "state_call",
        "params": ["TransactionPaymentApi_query_info", params_hex]
    });
    ws.send(WsMessage::Text(req.to_string().into()))
        .await
        .map_err(|e| format!("send: {e}"))?;

    let result = read_text_result(&mut ws).await?;
    let result_hex = result
        .as_str()
        .ok_or_else(|| format!("unexpected response: {result}"))?;

    // 4. Decode SCALE RuntimeDispatchInfo
    //    Weight { ref_time: Compact<u64>, proof_size: Compact<u64> }
    //    DispatchClass: u8
    //    partial_fee: remaining bytes as LE integer
    let bytes =
        hex::decode(result_hex.trim_start_matches("0x")).map_err(|e| format!("hex decode: {e}"))?;

    let mut offset = 0;
    offset += scale_compact_len(&bytes, offset)?;
    offset += scale_compact_len(&bytes, offset)?;
    offset += 1;

    if offset >= bytes.len() {
        return Err("no fee data in response".into());
    }

    let fee_bytes = &bytes[offset..];
    let mut buf = [0u8; 16];
    let copy_len = fee_bytes.len().min(16);
    buf[..copy_len].copy_from_slice(&fee_bytes[..copy_len]);
    Ok(u128::from_le_bytes(buf))
}

/// Fetch token symbol and decimals from chain properties.
pub async fn fetch_token_info(node_url: &str) -> Result<(String, u32), String> {
    let (mut ws, _) = connect_async(node_url)
        .await
        .map_err(|e| format!("connect: {e}"))?;

    let req = json!({"jsonrpc":"2.0","id":1,"method":"system_properties","params":[]});
    ws.send(WsMessage::Text(req.to_string().into()))
        .await
        .map_err(|e| format!("send: {e}"))?;

    let result = read_text_result(&mut ws).await?;
    let symbol = result["tokenSymbol"]
        .as_str()
        .or_else(|| {
            result["tokenSymbol"]
                .as_array()
                .and_then(|a| a.first()?.as_str())
        })
        .unwrap_or("UNIT")
        .to_string();
    let decimals = result["tokenDecimals"]
        .as_u64()
        .or_else(|| {
            result["tokenDecimals"]
                .as_array()
                .and_then(|a| a.first()?.as_u64())
        })
        .unwrap_or(0) as u32;

    Ok((symbol, decimals))
}

/// Fetch the free balance for an account using the metadata-derived layout.
pub async fn fetch_balance(
    node_url: &str,
    pubkey: &Pubkey,
    layout: &AccountInfoLayout,
) -> Result<u128, String> {
    let (mut ws, _) = connect_async(node_url)
        .await
        .map_err(|e| format!("connect: {e}"))?;

    // System.Account key: twox128("System") ++ twox128("Account") ++ blake2_128_concat(pubkey)
    let mut key = Vec::with_capacity(64);
    key.extend_from_slice(&twox128(b"System"));
    key.extend_from_slice(&twox128(b"Account"));
    let mut hasher = blake2::Blake2b::<blake2::digest::typenum::U16>::new();
    hasher.update(pubkey.0);
    key.extend_from_slice(&hasher.finalize());
    key.extend_from_slice(&pubkey.0);

    let req = json!({
        "jsonrpc":"2.0","id":1,"method":"state_getStorage",
        "params":[format!("0x{}", hex::encode(&key))]
    });
    ws.send(WsMessage::Text(req.to_string().into()))
        .await
        .map_err(|e| format!("send: {e}"))?;

    let result = read_text_result(&mut ws).await?;
    if result.is_null() {
        return Err("account storage missing on this RPC backend".into());
    }
    let hex_str = result
        .as_str()
        .ok_or("unexpected state_getStorage response shape")?;
    let data = hex::decode(hex_str.trim_start_matches("0x")).map_err(|e| format!("hex: {e}"))?;
    layout.decode_free(&data)
}

fn twox128(data: &[u8]) -> [u8; 16] {
    use std::hash::Hasher;
    let mut h0 = twox_hash::XxHash64::with_seed(0);
    h0.write(data);
    let mut h1 = twox_hash::XxHash64::with_seed(1);
    h1.write(data);
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&h0.finish().to_le_bytes());
    out[8..].copy_from_slice(&h1.finish().to_le_bytes());
    out
}

/// Submit a remark extrinsic using a single WebSocket connection for nonce + submit.
pub async fn submit_remark(
    node_url: &str,
    remark: &[u8],
    keypair: &schnorrkel::Keypair,
    ss58: &str,
    chain_info: &ChainInfo,
) -> Result<String, String> {
    let (mut ws, _) = connect_async(node_url)
        .await
        .map_err(|e| format!("connect: {e}"))?;

    let chain_info = refresh_signing_params(&mut ws, chain_info).await?;

    let nonce_req =
        json!({"jsonrpc":"2.0","id":1,"method":"system_accountNextIndex","params":[ss58]});
    ws.send(WsMessage::Text(nonce_req.to_string().into()))
        .await
        .map_err(|e| format!("send: {e}"))?;
    let nonce_result = read_text_result(&mut ws).await?;
    let nonce = nonce_result
        .as_u64()
        .map(|n| n as u32)
        .ok_or_else(|| format!("unexpected nonce response: {nonce_result}"))?;

    let ext = build_remark_extrinsic(remark, keypair, nonce, &chain_info);
    let hex = format!("0x{}", hex::encode(&ext));
    let watch_req =
        json!({"jsonrpc":"2.0","id":2,"method":"author_submitAndWatchExtrinsic","params":[hex]});
    ws.send(WsMessage::Text(watch_req.to_string().into()))
        .await
        .map_err(|e| format!("send: {e}"))?;

    // Wait for inBlock or finalized status
    loop {
        let resp = read_text_result_raw(&mut ws).await?;
        // Subscription confirmation (returns subscription id)
        if resp.get("result").is_some() && resp.get("method").is_none() {
            continue;
        }
        // Status update from subscription
        if let Some(status) = resp.pointer("/params/result") {
            if let Some(block_hash) = status.get("inBlock").and_then(|v| v.as_str()) {
                return Ok(block_hash.to_string());
            }
            if let Some(block_hash) = status.get("finalized").and_then(|v| v.as_str()) {
                return Ok(block_hash.to_string());
            }
            if status.get("dropped").is_some()
                || status.get("invalid").is_some()
                || status.get("usurped").is_some()
            {
                return Err(format!("transaction failed: {status}"));
            }
            // Other statuses (ready, broadcast, future): keep waiting
            continue;
        }
        // RPC error
        if let Some(err) = resp.get("error") {
            return Err(format!("RPC error: {err}"));
        }
    }
}

/// Returns the byte length of a SCALE Compact-encoded value at the given offset.
fn scale_compact_len(data: &[u8], offset: usize) -> Result<usize, String> {
    if offset >= data.len() {
        return Err("unexpected end of data".into());
    }
    match data[offset] & 0b11 {
        0b00 => Ok(1),
        0b01 => Ok(2),
        0b10 => Ok(4),
        0b11 => {
            let extra = (data[offset] >> 2) as usize + 4;
            Ok(1 + extra)
        }
        _ => unreachable!(),
    }
}

fn hex_to_32(hex_str: &str) -> Result<[u8; 32], String> {
    let bytes = hex::decode(hex_str.trim_start_matches("0x")).map_err(|e| format!("hex: {e}"))?;
    bytes
        .try_into()
        .map_err(|_| "expected 32 bytes".to_string())
}
