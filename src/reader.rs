use samp::extrinsic::{extract_call, extract_signer as samp_extract_signer};
use samp::scale::{decode_bytes, decode_compact};
use samp::{
    ContentType, EncryptedPayload, Remark, decode_channel_content, decode_group_content,
    decode_group_members, decode_remark, decode_thread_content,
};
use serde_json::Value;
use std::sync::mpsc::Sender;

use crate::event::Event;
use crate::extrinsic::{SYSTEM_REMARK, SYSTEM_REMARK_WITH_EVENT};
use crate::secret::DecryptionKeys;
use crate::types::{BlockRef, Pubkey};

pub struct ReadContext<'a> {
    pub my_pubkey: &'a Pubkey,
    pub keys: &'a DecryptionKeys,
    pub tx: &'a Sender<Event>,
}

pub struct RemarkSource {
    pub sender: Pubkey,
    pub remark: Remark,
    pub remark_bytes: samp::RemarkBytes,
    pub at: BlockRef,
    pub timestamp_secs: u64,
}

pub fn read_block(block: &Value, ctx: &ReadContext) {
    let extrinsics = match block["extrinsics"].as_array() {
        Some(exts) => exts,
        None => return,
    };

    let block_number = block["header"]["number"]
        .as_str()
        .and_then(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0);

    let _ = ctx.tx.send(Event::BlockUpdate(u64::from(block_number)));

    let block_ts_ms = extract_block_timestamp(extrinsics);

    for (ext_index, ext) in extrinsics.iter().enumerate() {
        let Some(ext_hex) = ext.as_str() else {
            continue;
        };
        let ext_index_u16 = u16::try_from(ext_index).unwrap_or(u16::MAX);
        if let Some(source) =
            source_from_extrinsic(ext_hex, block_number, ext_index_u16, block_ts_ms)
        {
            process_remark(&source, ctx.my_pubkey, ctx.keys, ctx.tx);
        }
    }
}

pub fn read_extrinsic(
    ext_hex: &str,
    ctx: &ReadContext,
    block_number: u32,
    ext_index: u16,
    block_ts_ms: u64,
) {
    if let Some(source) = source_from_extrinsic(ext_hex, block_number, ext_index, block_ts_ms) {
        process_remark(&source, ctx.my_pubkey, ctx.keys, ctx.tx);
    }
}

pub fn source_from_extrinsic(
    ext_hex: &str,
    block_number: u32,
    ext_index: u16,
    block_ts_ms: u64,
) -> Option<RemarkSource> {
    let ext_bytes =
        samp::ExtrinsicBytes::from_bytes(hex::decode(ext_hex.trim_start_matches("0x")).ok()?);
    let sender = samp_extract_signer(&ext_bytes)?;
    let remark_bytes = extract_remark_from_call(&ext_bytes)?;
    let remark = decode_remark(&remark_bytes).ok()?;
    Some(RemarkSource {
        sender,
        remark,
        remark_bytes,
        at: BlockRef::from_parts(block_number, ext_index),
        timestamp_secs: block_ts_ms / 1000,
    })
}

fn extract_remark_from_call(ext_bytes: &samp::ExtrinsicBytes) -> Option<samp::RemarkBytes> {
    let call = extract_call(ext_bytes)?;
    let pair = (call.pallet, call.call);
    if pair != SYSTEM_REMARK && pair != SYSTEM_REMARK_WITH_EVENT {
        return None;
    }
    let (payload, _) = decode_bytes(call.args)?;
    Some(samp::RemarkBytes::from_bytes(payload.to_vec()))
}

pub fn extract_block_timestamp(extrinsics: &[Value]) -> u64 {
    for ext in extrinsics {
        let ext_hex = match ext.as_str() {
            Some(s) => s,
            None => continue,
        };
        let ext_bytes = match hex::decode(ext_hex.trim_start_matches("0x")) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let (_, prefix_len) = match decode_compact(&ext_bytes) {
            Some(v) => v,
            None => continue,
        };
        let payload = &ext_bytes[prefix_len..];
        if payload.is_empty() || payload[0] & 0x80 != 0 {
            continue;
        }
        if payload.len() < 4 {
            continue;
        }
        if let Some((ts_ms, _)) = decode_compact(&payload[3..])
            && ts_ms > 1_000_000_000_000
        {
            return ts_ms;
        }
    }
    0
}

pub fn process_remark(
    source: &RemarkSource,
    my_pubkey: &Pubkey,
    keys: &DecryptionKeys,
    tx: &Sender<Event>,
) {
    let sender = source.sender;
    let block_number = source.at.block().get();
    let ext_index = source.at.index().get();
    let timestamp = samp::Timestamp::from_unix_secs(source.timestamp_secs);

    match &source.remark {
        Remark::Public { recipient, body } => {
            if recipient != my_pubkey && sender != *my_pubkey {
                return;
            }
            let _ = tx.send(Event::NewMessage {
                sender,
                content_type: ContentType::Public.to_byte(),
                recipient: *recipient,
                decrypted_body: Some(body.clone()),
                thread_ref: BlockRef::ZERO,
                reply_to: BlockRef::ZERO,
                continues: BlockRef::ZERO,
                block_number,
                ext_index,
                timestamp,
            });
        }
        Remark::Encrypted(payload) => process_one_to_one(
            payload,
            ContentType::Encrypted,
            sender,
            my_pubkey,
            keys,
            source,
            tx,
            false,
        ),
        Remark::Thread(payload) => process_one_to_one(
            payload,
            ContentType::Thread,
            sender,
            my_pubkey,
            keys,
            source,
            tx,
            true,
        ),
        Remark::ChannelCreate { name, description } => {
            let creator_ss58 = crate::util::ss58_short(&sender);
            let _ = tx.send(Event::ChannelDiscovered {
                name: name.as_str().to_string(),
                description: description.as_str().to_string(),
                creator_ss58,
                channel_ref: BlockRef::from_parts(block_number, ext_index),
            });
        }
        Remark::Channel {
            channel_ref,
            content,
        } => {
            if let Ok((reply_to, continues, body_bytes)) = decode_channel_content(content)
                && let Ok(body) = String::from_utf8(body_bytes.to_vec())
            {
                let sender_ss58 = crate::util::ss58_short(&sender);
                let _ = tx.send(Event::NewChannelMessage {
                    sender,
                    sender_ss58,
                    channel_ref: *channel_ref,
                    body,
                    reply_to,
                    continues,
                    block_number,
                    ext_index,
                    timestamp,
                });
            }
        }
        Remark::Group(payload) => {
            let scalar = keys.scalar();

            let plaintext =
                match samp::decrypt_from_group(&payload.content, &scalar, &payload.nonce, None) {
                    Ok(pt) => pt,
                    Err(_) => return,
                };

            let (group_ref, reply_to, continues, body_bytes) =
                match decode_group_content(plaintext.as_bytes()) {
                    Ok(r) => r,
                    Err(_) => return,
                };

            if group_ref.is_zero() {
                let (members, first_msg) = match decode_group_members(body_bytes) {
                    Ok(r) => r,
                    Err(_) => return,
                };
                let _ = tx.send(Event::GroupDiscovered {
                    creator_pubkey: sender,
                    group_ref: BlockRef::from_parts(block_number, ext_index),
                    members,
                });
                let body = String::from_utf8(first_msg.to_vec()).unwrap_or_default();
                let sender_ss58 = crate::util::ss58_short(&sender);
                let _ = tx.send(Event::NewGroupMessage {
                    sender,
                    sender_ss58,
                    group_ref: BlockRef::from_parts(block_number, ext_index),
                    body,
                    reply_to: BlockRef::ZERO,
                    continues: BlockRef::ZERO,
                    block_number,
                    ext_index,
                    timestamp,
                });
            } else {
                let body = match String::from_utf8(body_bytes.to_vec()) {
                    Ok(b) => b,
                    Err(_) => return,
                };
                let sender_ss58 = crate::util::ss58_short(&sender);
                let _ = tx.send(Event::NewGroupMessage {
                    sender,
                    sender_ss58,
                    group_ref,
                    body,
                    reply_to,
                    continues,
                    block_number,
                    ext_index,
                    timestamp,
                });
            }
        }
        Remark::Application { .. } => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn process_one_to_one(
    payload: &EncryptedPayload,
    ct: ContentType,
    sender: Pubkey,
    my_pubkey: &Pubkey,
    keys: &DecryptionKeys,
    source: &RemarkSource,
    tx: &Sender<Event>,
    is_thread: bool,
) {
    let block_number = source.at.block().get();
    let ext_index = source.at.index().get();
    let timestamp = samp::Timestamp::from_unix_secs(source.timestamp_secs);
    let is_mine = sender == *my_pubkey;
    let scalar = keys.scalar();

    if !is_mine {
        let tag = match samp::check_view_tag(payload, &scalar) {
            Ok(t) => t,
            Err(_) => return,
        };
        if tag != payload.view_tag {
            return;
        }
    }

    let plaintext = if is_mine {
        let Some(seed_bytes) = keys.seed() else {
            let _ = tx.send(Event::LockedOutbound {
                sender,
                block_number,
                ext_index,
                timestamp,
                remark_bytes: source.remark_bytes.clone(),
            });
            return;
        };
        let sender_seed = samp::Seed::from_bytes(*seed_bytes);
        samp::decrypt_as_sender(payload, &sender_seed)
    } else {
        samp::decrypt(payload, &scalar)
    };

    let plaintext = match plaintext {
        Ok(pt) => pt,
        Err(_) => return,
    };

    let mut recipient = *my_pubkey;
    if is_mine
        && let Some(seed_bytes) = keys.seed()
        && let Ok(r) = samp::unseal_recipient(payload, &samp::Seed::from_bytes(*seed_bytes))
    {
        recipient = r;
    }

    let (body, thread_ref, reply_to, continues) = if is_thread {
        match decode_thread_content(plaintext.as_bytes()) {
            Ok((thread, reply_to, continues, body_bytes)) => {
                let body = String::from_utf8(body_bytes.to_vec()).ok();
                (body, thread, reply_to, continues)
            }
            Err(_) => return,
        }
    } else {
        (
            String::from_utf8(plaintext.into_bytes()).ok(),
            BlockRef::ZERO,
            BlockRef::ZERO,
            BlockRef::ZERO,
        )
    };

    let _ = tx.send(Event::NewMessage {
        sender,
        content_type: ct.to_byte(),
        recipient,
        decrypted_body: body,
        thread_ref,
        reply_to,
        continues,
        block_number,
        ext_index,
        timestamp,
    });
}
