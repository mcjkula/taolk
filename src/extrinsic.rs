use std::sync::Arc;
use std::time::Duration;

use blake2::Digest;
use futures_util::{SinkExt, StreamExt};
use samp::extrinsic::{ChainParams, build_signed_extrinsic};
use samp::metadata::{ErrorTable, Metadata, StorageLayout};
use samp::scale::decode_compact;
use samp::{CallArgs, CallIdx, ExtrinsicNonce, PalletIdx, SpecVersion, TxVersion};
use serde_json::{Value, json};
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

use crate::error::ChainError;
use crate::types::Pubkey;

pub(crate) const SYSTEM_REMARK: (u8, u8) = (0, 9);
pub(crate) const SYSTEM_REMARK_WITH_EVENT: (u8, u8) = (0, 7);

#[derive(Clone)]
pub struct ChainInfo {
    pub name: crate::types::ChainName,
    pub ss58_prefix: samp::Ss58Prefix,
    pub chain_params: ChainParams,
    pub account_storage: StorageLayout,
    pub errors: Arc<ErrorTable>,
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
    let metadata = Metadata::from_runtime_metadata(&metadata_bytes)?;

    let account_storage = metadata.storage_layout("System", "Account", &["data", "free"])?;

    let req = json!({"jsonrpc":"2.0","id":4,"method":"system_chain","params":[]});
    ws.send(WsMessage::Text(req.to_string().into()))
        .await
        .map_err(|e| ChainError::Send(e.to_string()))?;
    let chain_result = read_text_result(&mut ws).await?;
    let name_str = chain_result
        .as_str()
        .ok_or(ChainError::MissingField("system_chain"))?
        .to_string();
    let name = crate::types::ChainName::parse(name_str).map_err(|_| ChainError::BadShape)?;

    let req = json!({"jsonrpc":"2.0","id":5,"method":"system_properties","params":[]});
    ws.send(WsMessage::Text(req.to_string().into()))
        .await
        .map_err(|e| ChainError::Send(e.to_string()))?;
    let props = read_text_result(&mut ws).await?;
    let ss58_raw = props["ss58Format"]
        .as_u64()
        .ok_or(ChainError::MissingField("ss58Format"))?;
    let ss58_raw_u16 = u16::try_from(ss58_raw).map_err(|_| ChainError::BadShape)?;
    let ss58_prefix = samp::Ss58Prefix::new(ss58_raw_u16).map_err(|_| ChainError::BadShape)?;

    Ok(ChainInfo {
        name,
        ss58_prefix,
        chain_params: ChainParams::new(
            samp::GenesisHash::from_bytes(genesis_bytes),
            SpecVersion::new(spec_version),
            TxVersion::new(tx_version),
        ),
        account_storage,
        errors: Arc::new(metadata.errors().clone()),
    })
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
        name: base.name.clone(),
        ss58_prefix: base.ss58_prefix,
        chain_params: ChainParams::new(
            samp::GenesisHash::from_bytes(genesis_hash),
            SpecVersion::new(spec_version),
            TxVersion::new(tx_version),
        ),
        account_storage: base.account_storage.clone(),
        errors: base.errors.clone(),
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

fn build_remark_call_args(remark: &samp::RemarkBytes) -> Result<CallArgs, ChainError> {
    let remark_len = u64::try_from(remark.len())
        .map_err(|_| ChainError::MessageTooLong { len: remark.len() })?;
    let mut args = Vec::with_capacity(remark.len() + 5);
    samp::scale::encode_compact(remark_len, &mut args);
    args.extend_from_slice(remark.as_bytes());
    Ok(CallArgs::from_bytes(args))
}

fn build_remark_with_event(
    remark: &samp::RemarkBytes,
    signing: &crate::secret::SigningKey,
    nonce: u32,
    chain_info: &ChainInfo,
) -> Result<samp::ExtrinsicBytes, ChainError> {
    let args = build_remark_call_args(remark)?;
    let public_key = signing.public_key();
    build_signed_extrinsic(
        PalletIdx::new(SYSTEM_REMARK_WITH_EVENT.0),
        CallIdx::new(SYSTEM_REMARK_WITH_EVENT.1),
        &args,
        &public_key,
        |msg| samp::Signature::from_bytes(signing.sign(msg)),
        ExtrinsicNonce::new(nonce),
        &chain_info.chain_params,
    )
    .map_err(ChainError::from)
}

pub async fn estimate_fee(
    node_url: &str,
    remark: &samp::RemarkBytes,
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

    let ext = build_remark_with_event(remark, signing, nonce, &chain_info)?;

    let mut params = Vec::new();
    params.extend_from_slice(ext.as_bytes());
    let ext_len =
        u32::try_from(ext.len()).map_err(|_| ChainError::MessageTooLong { len: ext.len() })?;
    params.extend_from_slice(&ext_len.to_le_bytes());

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
    offset += compact_len(&bytes, offset)?;
    offset += compact_len(&bytes, offset)?;
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
    layout: &StorageLayout,
) -> Result<u128, ChainError> {
    let (mut ws, _) = connect_async(node_url)
        .await
        .map_err(|e| ChainError::Connect(e.to_string()))?;

    let mut key = Vec::with_capacity(64);
    key.extend_from_slice(&twox128(b"System"));
    key.extend_from_slice(&twox128(b"Account"));
    let mut hasher = blake2::Blake2b::<blake2::digest::typenum::U16>::new();
    hasher.update(*pubkey.as_bytes());
    key.extend_from_slice(&hasher.finalize());
    key.extend_from_slice(pubkey.as_bytes());

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
    Ok(layout.decode_uint(&data)?)
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
    remark: &samp::RemarkBytes,
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

    let ext = build_remark_with_event(remark, signing, nonce, &chain_info)?;
    let hex = format!("0x{}", hex::encode(ext.as_bytes()));
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

fn compact_len(data: &[u8], offset: usize) -> Result<usize, ChainError> {
    let (_, consumed) = decode_compact(&data[offset..]).ok_or(ChainError::BadShape)?;
    Ok(consumed)
}

fn hex_to_32(hex_str: &str) -> Result<[u8; 32], ChainError> {
    let bytes = hex::decode(hex_str.trim_start_matches("0x"))?;
    bytes.try_into().map_err(|_| ChainError::BadLength)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twox128_system() {
        let hash = twox128(b"System");
        assert_eq!(hex::encode(hash), "26aa394eea5630e07c48ae0c9558cef7");
    }

    #[test]
    fn twox128_account() {
        let hash = twox128(b"Account");
        assert_eq!(hex::encode(hash), "b99d880ec681799c0cf30e8886371da9");
    }

    #[test]
    fn twox128_empty() {
        let hash = twox128(b"");
        assert_eq!(hash.len(), 16);
        let hash2 = twox128(b"");
        assert_eq!(hash, hash2);
    }

    #[test]
    fn twox128_different_inputs_differ() {
        assert_ne!(twox128(b"System"), twox128(b"Account"));
    }

    #[test]
    fn hex_to_32_with_0x_prefix() {
        let hex_str = format!("0x{}", "aa".repeat(32));
        let result = hex_to_32(&hex_str).unwrap();
        assert_eq!(result, [0xAA; 32]);
    }

    #[test]
    fn hex_to_32_without_prefix() {
        let hex_str = "bb".repeat(32);
        let result = hex_to_32(&hex_str).unwrap();
        assert_eq!(result, [0xBB; 32]);
    }

    #[test]
    fn hex_to_32_wrong_length() {
        let result = hex_to_32("aabbcc");
        assert!(matches!(result, Err(ChainError::BadLength)));
    }

    #[test]
    fn hex_to_32_invalid_hex() {
        let result = hex_to_32("zzzz");
        assert!(matches!(result, Err(ChainError::Hex(_))));
    }

    #[test]
    fn hex_to_32_empty() {
        let result = hex_to_32("");
        assert!(matches!(result, Err(ChainError::BadLength)));
    }

    #[test]
    fn compact_len_single_byte() {
        let mut buf = Vec::new();
        samp::scale::encode_compact(10, &mut buf);
        let len = compact_len(&buf, 0).unwrap();
        assert_eq!(len, 1);
    }

    #[test]
    fn compact_len_two_bytes() {
        let mut buf = Vec::new();
        samp::scale::encode_compact(1000, &mut buf);
        let len = compact_len(&buf, 0).unwrap();
        assert_eq!(len, 2);
    }

    #[test]
    fn compact_len_four_bytes() {
        let mut buf = Vec::new();
        samp::scale::encode_compact(1_000_000, &mut buf);
        let len = compact_len(&buf, 0).unwrap();
        assert_eq!(len, 4);
    }

    #[test]
    fn compact_len_with_offset() {
        let mut buf = vec![0xFF, 0xFF]; // padding
        let offset = buf.len();
        samp::scale::encode_compact(42, &mut buf);
        let len = compact_len(&buf, offset).unwrap();
        assert_eq!(len, 1);
    }

    #[test]
    fn compact_len_empty_data() {
        let result = compact_len(&[], 0);
        assert!(result.is_err());
    }

    #[test]
    fn build_remark_call_args_small() {
        let remark = samp::RemarkBytes::from_bytes(b"hello".to_vec());
        let args = build_remark_call_args(&remark).unwrap();
        let data = args.as_bytes();
        let (decoded_len, consumed) = samp::scale::decode_compact(data).unwrap();
        assert_eq!(decoded_len as usize, 5);
        assert_eq!(&data[consumed..], b"hello");
    }

    #[test]
    fn build_remark_call_args_empty() {
        let remark = samp::RemarkBytes::from_bytes(vec![]);
        let args = build_remark_call_args(&remark).unwrap();
        let data = args.as_bytes();
        let (decoded_len, _) = samp::scale::decode_compact(data).unwrap();
        assert_eq!(decoded_len, 0);
    }

    #[test]
    fn build_remark_call_args_large() {
        let payload = vec![0xAB; 1000];
        let remark = samp::RemarkBytes::from_bytes(payload.clone());
        let args = build_remark_call_args(&remark).unwrap();
        let data = args.as_bytes();
        let (decoded_len, consumed) = samp::scale::decode_compact(data).unwrap();
        assert_eq!(decoded_len as usize, 1000);
        assert_eq!(&data[consumed..], &payload);
    }

    #[test]
    fn build_remark_with_event_produces_extrinsic() {
        let seed = crate::secret::Seed::from_bytes([0xAA; 32]);
        let signing = seed.derive_signing_key();
        let remark = samp::RemarkBytes::from_bytes(b"test message".to_vec());
        let chain_info = ChainInfo {
            name: crate::types::ChainName::parse("Test").unwrap(),
            ss58_prefix: samp::Ss58Prefix::SUBSTRATE_GENERIC,
            chain_params: samp::extrinsic::ChainParams::new(
                samp::GenesisHash::from_bytes([0; 32]),
                samp::SpecVersion::new(1),
                samp::TxVersion::new(1),
            ),
            account_storage: samp::metadata::StorageLayout {
                offset: 16,
                width: 8,
            },
            errors: Default::default(),
        };
        let ext = build_remark_with_event(&remark, &signing, 0, &chain_info).unwrap();
        assert!(!ext.as_bytes().is_empty());

        let signer = samp::extrinsic::extract_signer(&ext).unwrap();
        assert_eq!(signer, signing.public_key());

        let call = samp::extrinsic::extract_call(&ext).unwrap();
        assert_eq!(call.pallet().get(), SYSTEM_REMARK_WITH_EVENT.0);
        assert_eq!(call.call().get(), SYSTEM_REMARK_WITH_EVENT.1);
    }

    #[test]
    fn build_remark_with_event_different_nonces() {
        let seed = crate::secret::Seed::from_bytes([0xAA; 32]);
        let signing = seed.derive_signing_key();
        let remark = samp::RemarkBytes::from_bytes(b"msg".to_vec());
        let chain_info = ChainInfo {
            name: crate::types::ChainName::parse("Test").unwrap(),
            ss58_prefix: samp::Ss58Prefix::SUBSTRATE_GENERIC,
            chain_params: samp::extrinsic::ChainParams::new(
                samp::GenesisHash::from_bytes([0; 32]),
                samp::SpecVersion::new(1),
                samp::TxVersion::new(1),
            ),
            account_storage: samp::metadata::StorageLayout {
                offset: 16,
                width: 8,
            },
            errors: Default::default(),
        };
        let ext0 = build_remark_with_event(&remark, &signing, 0, &chain_info).unwrap();
        let ext1 = build_remark_with_event(&remark, &signing, 1, &chain_info).unwrap();
        assert_ne!(ext0.as_bytes(), ext1.as_bytes());
    }

    #[test]
    fn system_remark_constants() {
        assert_eq!(SYSTEM_REMARK, (0, 9));
        assert_eq!(SYSTEM_REMARK_WITH_EVENT, (0, 7));
    }
}
