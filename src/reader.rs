use samp::extrinsic::{extract_call, extract_signer as samp_extract_signer};
use samp::scale::{decode_bytes, decode_compact};
use samp::{
    ContentType, decode_channel_content, decode_channel_create, decode_group_content,
    decode_group_members, decode_remark, decode_thread_content,
};
use serde_json::Value;
use std::sync::mpsc::Sender;

use crate::event::Event;
use crate::extrinsic::ChainInfo;
use crate::types::{BlockRef, Pubkey};

pub struct ReadContext<'a> {
    pub my_pubkey: &'a Pubkey,
    pub seed: &'a [u8; 32],
    pub tx: &'a Sender<Event>,
    pub chain_info: &'a ChainInfo,
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

    let block_ts = extract_block_timestamp(extrinsics);

    for (ext_index, ext) in extrinsics.iter().enumerate() {
        let Some(ext_hex) = ext.as_str() else {
            continue;
        };
        let Ok(ext_bytes) = hex::decode(ext_hex.trim_start_matches("0x")) else {
            continue;
        };

        let signer = extract_signer(&ext_bytes);
        let remark = extract_remark(&ext_bytes, ctx.chain_info);

        let ext_index_u16 = u16::try_from(ext_index).unwrap_or(u16::MAX);
        if let (Some(sender), Some(remark_data)) = (signer, remark) {
            process_remark(
                &remark_data,
                &sender,
                ctx,
                block_number,
                ext_index_u16,
                block_ts,
            );
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
    let ext_bytes = match hex::decode(ext_hex.trim_start_matches("0x")) {
        Ok(b) => b,
        Err(_) => return,
    };

    let signer = extract_signer(&ext_bytes);
    let remark = extract_remark(&ext_bytes, ctx.chain_info);

    if let (Some(sender), Some(remark_data)) = (signer, remark) {
        process_remark(
            &remark_data,
            &sender,
            ctx,
            block_number,
            ext_index,
            block_ts_ms,
        );
    }
}

fn extract_signer(ext_bytes: &[u8]) -> Option<Pubkey> {
    samp_extract_signer(ext_bytes).map(Pubkey)
}

fn extract_remark(ext_bytes: &[u8], chain_info: &ChainInfo) -> Option<Vec<u8>> {
    let call = extract_call(ext_bytes)?;
    let pair = (call.pallet, call.call);
    if pair != chain_info.system_remark && pair != chain_info.system_remark_with_event {
        return None;
    }
    let (payload, _) = decode_bytes(call.args)?;
    Some(payload.to_vec())
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

fn process_remark(
    remark_data: &[u8],
    sender: &Pubkey,
    ctx: &ReadContext,
    block_number: u32,
    ext_index: u16,
    block_ts_ms: u64,
) {
    let my_pubkey = ctx.my_pubkey;
    let seed = ctx.seed;
    let tx = ctx.tx;
    let remark = match decode_remark(remark_data) {
        Ok(r) => r,
        Err(_) => return,
    };

    match remark.content_type {
        ContentType::Public => {
            if remark.recipient != my_pubkey.0 && *sender != *my_pubkey {
                return;
            }
            let ct = remark.content_type.to_byte();
            let recipient = Pubkey(remark.recipient);
            let body = String::from_utf8(remark.content).ok();
            let _ = tx.send(Event::NewMessage {
                sender: *sender,
                content_type: ct,
                recipient,
                decrypted_body: body,
                thread_ref: BlockRef::ZERO,
                reply_to: BlockRef::ZERO,
                continues: BlockRef::ZERO,
                block_number,
                ext_index,
                timestamp: block_ts_ms / 1000,
            });
        }
        ContentType::Encrypted | ContentType::Thread => {
            let is_mine = *sender == *my_pubkey;

            if !is_mine {
                let scalar = samp::sr25519_signing_scalar(seed);
                let tag = match samp::check_view_tag(&remark, &scalar) {
                    Ok(t) => t,
                    Err(_) => return,
                };
                if tag != remark.view_tag {
                    return;
                }
            }

            let plaintext = if is_mine {
                samp::decrypt_as_sender(&remark, seed)
            } else {
                let scalar = samp::sr25519_signing_scalar(seed);
                samp::decrypt(&remark, &scalar)
            };

            let plaintext = match plaintext {
                Ok(pt) => pt,
                Err(_) => return,
            };

            let mut recipient = remark.recipient;
            if is_mine && let Ok(r) = samp::unseal_recipient(&remark, seed) {
                recipient = r;
            }

            let ct = remark.content_type.to_byte();
            let (body, thread_ref, reply_to, continues) = if ct & 0x0F == 0x02 {
                match decode_thread_content(&plaintext) {
                    Ok((thread, reply_to, continues, body_bytes)) => {
                        let body = String::from_utf8(body_bytes.to_vec()).ok();
                        (body, thread, reply_to, continues)
                    }
                    Err(_) => return,
                }
            } else {
                (
                    String::from_utf8(plaintext).ok(),
                    BlockRef::ZERO,
                    BlockRef::ZERO,
                    BlockRef::ZERO,
                )
            };

            let _ = tx.send(Event::NewMessage {
                sender: *sender,
                content_type: ct,
                recipient: Pubkey(recipient),
                decrypted_body: body,
                thread_ref,
                reply_to,
                continues,
                block_number,
                ext_index,
                timestamp: block_ts_ms / 1000,
            });
        }
        ContentType::ChannelCreate => {
            let (name, description) = match decode_channel_create(&remark.content) {
                Ok(r) => (r.0.to_string(), r.1.to_string()),
                Err(_) => return,
            };
            let creator_ss58 = crate::util::ss58_short(sender);
            let _ = tx.send(Event::ChannelDiscovered {
                name,
                description,
                creator_ss58,
                channel_ref: BlockRef {
                    block: block_number,
                    index: ext_index,
                },
            });
        }
        ContentType::Channel => {
            let channel_ref = samp::channel_ref_from_recipient(&remark.recipient);
            if let Ok((reply_to, continues, body_bytes)) = decode_channel_content(&remark.content)
                && let Ok(body) = String::from_utf8(body_bytes.to_vec())
            {
                let sender_ss58 = crate::util::ss58_short(sender);
                let _ = tx.send(Event::NewChannelMessage {
                    sender: *sender,
                    sender_ss58,
                    channel_ref,
                    body,
                    reply_to,
                    continues,
                    block_number,
                    ext_index,
                    timestamp: block_ts_ms / 1000,
                });
            }
        }
        ContentType::Group => {
            let scalar = samp::sr25519_signing_scalar(seed);

            let plaintext =
                match samp::decrypt_from_group(&remark.content, &scalar, &remark.nonce, None) {
                    Ok(pt) => pt,
                    Err(_) => return,
                };

            let (group_ref, reply_to, continues, body_bytes) =
                match decode_group_content(&plaintext) {
                    Ok(r) => r,
                    Err(_) => return,
                };

            if group_ref.is_zero() {
                let (members, first_msg) = match decode_group_members(body_bytes) {
                    Ok(r) => r,
                    Err(_) => return,
                };
                let members = members.into_iter().map(Pubkey).collect();
                let _ = tx.send(Event::GroupDiscovered {
                    creator_pubkey: *sender,
                    group_ref: BlockRef {
                        block: block_number,
                        index: ext_index,
                    },
                    members,
                });
                let body = String::from_utf8(first_msg.to_vec()).unwrap_or_default();
                let sender_ss58 = crate::util::ss58_short(sender);
                let _ = tx.send(Event::NewGroupMessage {
                    sender: *sender,
                    sender_ss58,
                    group_ref: BlockRef {
                        block: block_number,
                        index: ext_index,
                    },
                    body,
                    reply_to: BlockRef::ZERO,
                    continues: BlockRef::ZERO,
                    block_number,
                    ext_index,
                    timestamp: block_ts_ms / 1000,
                });
            } else {
                let body = match String::from_utf8(body_bytes.to_vec()) {
                    Ok(b) => b,
                    Err(_) => return,
                };
                let sender_ss58 = crate::util::ss58_short(sender);
                let _ = tx.send(Event::NewGroupMessage {
                    sender: *sender,
                    sender_ss58,
                    group_ref,
                    body,
                    reply_to,
                    continues,
                    block_number,
                    ext_index,
                    timestamp: block_ts_ms / 1000,
                });
            }
        }
        ContentType::Application(_) => {}
    }
}
