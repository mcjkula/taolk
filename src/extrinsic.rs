use std::sync::Arc;
use std::time::Duration;

use blake2::Digest;
use futures_util::{SinkExt, StreamExt};
use parity_scale_codec::{Compact, Encode};
use serde_json::{Value, json};
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

use crate::error::ChainError;
use crate::metadata::{AccountInfoLayout, ErrorTable, Metadata};
use crate::types::Pubkey;

const PALLET_SYSTEM: u8 = 0x00;
const CALL_REMARK_WITH_EVENT: u8 = 0x07;
const ERA_IMMORTAL: u8 = 0x00;
const METADATA_HASH_DISABLED: u8 = 0x00;
const EXT_VERSION_SIGNED: u8 = 0x84;
const ADDR_TYPE_ID: u8 = 0x00;
const SIG_TYPE_SR25519: u8 = 0x01;

#[derive(Clone)]
pub struct ChainInfo {
    pub genesis_hash: [u8; 32],
    pub spec_version: u32,
    pub tx_version: u32,
    pub account_info_layout: AccountInfoLayout,
    pub errors: Arc<ErrorTable>,
    pub chain_name: String,
}

pub async fn fetch_chain_info(node_url: &str) -> Result<ChainInfo, ChainError> {
    let (mut ws, _) = connect_async(node_url)
        .await
        .map_err(|e| ChainError::Connect(e.to_string()))?;

    let req = json!({"jsonrpc":"2.0","id":1,"method":"chain_getBlockHash","params":[0]});
    ws.send(WsMessage::Text(req.to_string().into()))
        .await
        .map_err(|e| ChainError::Send(e.to_string()))?;
    let genesis_hash = read_text_result(&mut ws).await?;
    let genesis_bytes = hex_to_32(
        genesis_hash
            .as_str()
            .ok_or(ChainError::MissingField("genesis hash"))?,
    )?;

    let req = json!({"jsonrpc":"2.0","id":2,"method":"state_getRuntimeVersion","params":[]});
    ws.send(WsMessage::Text(req.to_string().into()))
        .await
        .map_err(|e| ChainError::Send(e.to_string()))?;
    let rv = read_text_result(&mut ws).await?;
    let spec_version_raw = rv["specVersion"]
        .as_u64()
        .ok_or(ChainError::MissingField("specVersion"))?;
    let spec_version = u32::try_from(spec_version_raw)
        .map_err(|_| ChainError::SpecVersionOverflow(spec_version_raw))?;
    let tx_version_raw = rv["transactionVersion"]
        .as_u64()
        .ok_or(ChainError::MissingField("transactionVersion"))?;
    let tx_version = u32::try_from(tx_version_raw)
        .map_err(|_| ChainError::SpecVersionOverflow(tx_version_raw))?;

    let req = json!({"jsonrpc":"2.0","id":3,"method":"state_getMetadata","params":[]});
    ws.send(WsMessage::Text(req.to_string().into()))
        .await
        .map_err(|e| ChainError::Send(e.to_string()))?;
    let metadata_result = read_text_result(&mut ws).await?;
    let metadata_hex = metadata_result
        .as_str()
        .ok_or(ChainError::MissingField("state_getMetadata result"))?;
    let metadata_bytes = hex::decode(metadata_hex.trim_start_matches("0x"))?;
    let parsed = Metadata::from_runtime_metadata(&metadata_bytes)?;

    let req = json!({"jsonrpc":"2.0","id":4,"method":"system_chain","params":[]});
    ws.send(WsMessage::Text(req.to_string().into()))
        .await
        .map_err(|e| ChainError::Send(e.to_string()))?;
    let chain_name_raw = read_text_result(&mut ws).await?;
    let chain_name = canonical_chain_name(chain_name_raw.as_str().unwrap_or("unknown"));

    Ok(ChainInfo {
        genesis_hash: genesis_bytes,
        spec_version,
        tx_version,
        account_info_layout: parsed.layout,
        errors: Arc::new(parsed.errors),
        chain_name,
    })
}

fn canonical_chain_name(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("test") {
        "test".into()
    } else if lower == "bittensor" || lower == "finney" {
        "finney".into()
    } else {
        let mut s = raw.to_string();
        if s.chars().count() > 12 {
            s = s.chars().take(12).collect();
        }
        s
    }
}

pub fn build_remark_extrinsic(
    remark: &[u8],
    signing: &crate::secret::SigningKey,
    nonce: u32,
    chain_info: &ChainInfo,
) -> Result<Vec<u8>, ChainError> {
    let account_id = *signing.public_key();

    let mut call_data = Vec::new();
    call_data.push(PALLET_SYSTEM);
    call_data.push(CALL_REMARK_WITH_EVENT);
    let remark_len = u32::try_from(remark.len())
        .map_err(|_| ChainError::MessageTooLong { len: remark.len() })?;
    Compact(remark_len).encode_to(&mut call_data);
    call_data.extend_from_slice(remark);

    let tip: u8 = 0x00;

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

    let signature = signing.sign(&to_sign);

    let mut extrinsic_payload = Vec::new();
    extrinsic_payload.push(EXT_VERSION_SIGNED);
    extrinsic_payload.push(ADDR_TYPE_ID);
    extrinsic_payload.extend_from_slice(&account_id);
    extrinsic_payload.push(SIG_TYPE_SR25519);
    extrinsic_payload.extend_from_slice(&signature);
    extrinsic_payload.push(ERA_IMMORTAL);
    Compact(nonce).encode_to(&mut extrinsic_payload);
    extrinsic_payload.push(tip);
    extrinsic_payload.push(METADATA_HASH_DISABLED);
    extrinsic_payload.extend_from_slice(&call_data);

    let mut full = Vec::new();
    let payload_len =
        u32::try_from(extrinsic_payload.len()).map_err(|_| ChainError::MessageTooLong {
            len: extrinsic_payload.len(),
        })?;
    Compact(payload_len).encode_to(&mut full);
    full.extend_from_slice(&extrinsic_payload);

    Ok(full)
}

async fn refresh_signing_params(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    base: &ChainInfo,
) -> Result<ChainInfo, ChainError> {
    let req = json!({"jsonrpc":"2.0","id":91,"method":"chain_getBlockHash","params":[0]});
    ws.send(WsMessage::Text(req.to_string().into()))
        .await
        .map_err(|e| ChainError::Send(e.to_string()))?;
    let g = read_text_result(ws).await?;
    let genesis_hash = hex_to_32(g.as_str().ok_or(ChainError::MissingField("genesis hash"))?)?;

    let req = json!({"jsonrpc":"2.0","id":92,"method":"state_getRuntimeVersion","params":[]});
    ws.send(WsMessage::Text(req.to_string().into()))
        .await
        .map_err(|e| ChainError::Send(e.to_string()))?;
    let rv = read_text_result(ws).await?;
    let spec_version_raw = rv["specVersion"]
        .as_u64()
        .ok_or(ChainError::MissingField("specVersion"))?;
    let spec_version = u32::try_from(spec_version_raw)
        .map_err(|_| ChainError::SpecVersionOverflow(spec_version_raw))?;
    let tx_version_raw = rv["transactionVersion"]
        .as_u64()
        .ok_or(ChainError::MissingField("transactionVersion"))?;
    let tx_version = u32::try_from(tx_version_raw)
        .map_err(|_| ChainError::SpecVersionOverflow(tx_version_raw))?;

    Ok(ChainInfo {
        genesis_hash,
        spec_version,
        tx_version,
        account_info_layout: base.account_info_layout.clone(),
        errors: base.errors.clone(),
        chain_name: base.chain_name.clone(),
    })
}

async fn read_text_result(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Result<Value, ChainError> {
    loop {
        match ws.next().await {
            Some(Ok(WsMessage::Text(t))) => {
                let v: Value = serde_json::from_str(t.as_ref())
                    .map_err(|e| ChainError::Parse(e.to_string()))?;
                if let Some(err) = v.get("error") {
                    return Err(ChainError::Rpc(err.to_string()));
                }
                return Ok(v["result"].clone());
            }
            Some(Ok(WsMessage::Ping(_) | WsMessage::Pong(_))) => continue,
            Some(Err(e)) => return Err(ChainError::Ws(e.to_string())),
            None => return Err(ChainError::WsClosed),
            _ => continue,
        }
    }
}

async fn read_text_result_raw(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Result<Value, ChainError> {
    loop {
        match ws.next().await {
            Some(Ok(WsMessage::Text(t))) => {
                return serde_json::from_str(t.as_ref())
                    .map_err(|e| ChainError::Parse(e.to_string()));
            }
            Some(Ok(WsMessage::Ping(_) | WsMessage::Pong(_))) => continue,
            Some(Err(e)) => return Err(ChainError::Ws(e.to_string())),
            None => return Err(ChainError::WsClosed),
            _ => continue,
        }
    }
}

pub async fn estimate_fee(
    node_url: &str,
    remark: &[u8],
    signing: &crate::secret::SigningKey,
    ss58: &str,
    chain_info: &ChainInfo,
) -> Result<u128, ChainError> {
    let (mut ws, _) = connect_async(node_url)
        .await
        .map_err(|e| ChainError::Connect(e.to_string()))?;

    let chain_info = refresh_signing_params(&mut ws, chain_info).await?;

    let nonce_req =
        json!({"jsonrpc":"2.0","id":1,"method":"system_accountNextIndex","params":[ss58]});
    ws.send(WsMessage::Text(nonce_req.to_string().into()))
        .await
        .map_err(|e| ChainError::Send(e.to_string()))?;
    let nonce_result = read_text_result(&mut ws).await?;
    let nonce_raw = nonce_result.as_u64().ok_or(ChainError::BadShape)?;
    let nonce = u32::try_from(nonce_raw).map_err(|_| ChainError::SpecVersionOverflow(nonce_raw))?;

    let ext = build_remark_extrinsic(remark, signing, nonce, &chain_info)?;

    let mut params = Vec::new();
    params.extend_from_slice(&ext);
    let ext_len =
        u32::try_from(ext.len()).map_err(|_| ChainError::MessageTooLong { len: ext.len() })?;
    ext_len.encode_to(&mut params);

    let params_hex = format!("0x{}", hex::encode(&params));
    let req = json!({
        "jsonrpc": "2.0", "id": 2,
        "method": "state_call",
        "params": ["TransactionPaymentApi_query_info", params_hex]
    });
    ws.send(WsMessage::Text(req.to_string().into()))
        .await
        .map_err(|e| ChainError::Send(e.to_string()))?;

    let result = read_text_result(&mut ws).await?;
    let result_hex = result.as_str().ok_or(ChainError::BadShape)?;

    let bytes = hex::decode(result_hex.trim_start_matches("0x"))?;

    let mut offset = 0;
    offset += scale_compact_len(&bytes, offset)?;
    offset += scale_compact_len(&bytes, offset)?;
    offset += 1;

    if offset >= bytes.len() {
        return Err(ChainError::BadShape);
    }

    let fee_bytes = &bytes[offset..];
    let mut buf = [0u8; 16];
    let copy_len = fee_bytes.len().min(16);
    buf[..copy_len].copy_from_slice(&fee_bytes[..copy_len]);
    Ok(u128::from_le_bytes(buf))
}

pub async fn fetch_token_info(node_url: &str) -> Result<(String, u32), ChainError> {
    let (mut ws, _) = connect_async(node_url)
        .await
        .map_err(|e| ChainError::Connect(e.to_string()))?;

    let req = json!({"jsonrpc":"2.0","id":1,"method":"system_properties","params":[]});
    ws.send(WsMessage::Text(req.to_string().into()))
        .await
        .map_err(|e| ChainError::Send(e.to_string()))?;

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
    let decimals_raw = result["tokenDecimals"]
        .as_u64()
        .or_else(|| {
            result["tokenDecimals"]
                .as_array()
                .and_then(|a| a.first()?.as_u64())
        })
        .unwrap_or(0);
    let decimals = u32::try_from(decimals_raw).unwrap_or(u32::MAX);

    Ok((symbol, decimals))
}

pub async fn fetch_balance(
    node_url: &str,
    pubkey: &Pubkey,
    layout: &AccountInfoLayout,
) -> Result<u128, ChainError> {
    let (mut ws, _) = connect_async(node_url)
        .await
        .map_err(|e| ChainError::Connect(e.to_string()))?;

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
        .map_err(|e| ChainError::Send(e.to_string()))?;

    let result = read_text_result(&mut ws).await?;
    if result.is_null() {
        return Err(ChainError::BadShape);
    }
    let hex_str = result.as_str().ok_or(ChainError::BadShape)?;
    let data = hex::decode(hex_str.trim_start_matches("0x"))?;
    Ok(layout.decode_free(&data)?)
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

pub async fn submit_remark(
    node_url: &str,
    remark: &[u8],
    signing: &crate::secret::SigningKey,
    ss58: &str,
    chain_info: &ChainInfo,
) -> Result<String, ChainError> {
    let (mut ws, _) = connect_async(node_url)
        .await
        .map_err(|e| ChainError::Connect(e.to_string()))?;

    let chain_info = refresh_signing_params(&mut ws, chain_info).await?;

    let nonce_req =
        json!({"jsonrpc":"2.0","id":1,"method":"system_accountNextIndex","params":[ss58]});
    ws.send(WsMessage::Text(nonce_req.to_string().into()))
        .await
        .map_err(|e| ChainError::Send(e.to_string()))?;
    let nonce_result = read_text_result(&mut ws).await?;
    let nonce_raw = nonce_result.as_u64().ok_or(ChainError::BadShape)?;
    let nonce = u32::try_from(nonce_raw).map_err(|_| ChainError::SpecVersionOverflow(nonce_raw))?;

    let ext = build_remark_extrinsic(remark, signing, nonce, &chain_info)?;
    let hex = format!("0x{}", hex::encode(&ext));
    let watch_req =
        json!({"jsonrpc":"2.0","id":2,"method":"author_submitAndWatchExtrinsic","params":[hex]});
    ws.send(WsMessage::Text(watch_req.to_string().into()))
        .await
        .map_err(|e| ChainError::Send(e.to_string()))?;

    let wait = async {
        loop {
            let resp = read_text_result_raw(&mut ws).await?;
            if resp.get("result").is_some() && resp.get("method").is_none() {
                continue;
            }
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
                    return Err(ChainError::TxFailed(status.to_string()));
                }
                continue;
            }
            if let Some(err) = resp.get("error") {
                return Err(ChainError::Rpc(err.to_string()));
            }
        }
    };
    match tokio::time::timeout(Duration::from_secs(60), wait).await {
        Ok(result) => result,
        Err(_) => Err(ChainError::Timeout),
    }
}

fn scale_compact_len(data: &[u8], offset: usize) -> Result<usize, ChainError> {
    if offset >= data.len() {
        return Err(ChainError::BadShape);
    }
    match data[offset] & 0b11 {
        0b00 => Ok(1),
        0b01 => Ok(2),
        0b10 => Ok(4),
        0b11 => {
            let extra = usize::from(data[offset] >> 2) + 4;
            Ok(1 + extra)
        }
        _ => unreachable!(),
    }
}

fn hex_to_32(hex_str: &str) -> Result<[u8; 32], ChainError> {
    let bytes = hex::decode(hex_str.trim_start_matches("0x"))?;
    bytes.try_into().map_err(|_| ChainError::BadLength)
}
