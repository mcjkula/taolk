use samp::{
    ContentType, decode_channel_content, decode_channel_create, decode_group_content,
    decode_group_members, decode_remark, decode_thread_content,
};
use serde_json::Value;
use std::sync::mpsc::Sender;

use crate::event::Event;
use crate::types::{BlockRef, Pubkey};

pub struct ReadContext<'a> {
    pub my_pubkey: &'a Pubkey,
    pub seed: &'a [u8; 32],
    pub tx: &'a Sender<Event>,
}

/// Read a block for SAMP messages.
pub fn read_block(block: &Value, ctx: &ReadContext) {
    let extrinsics = match block["extrinsics"].as_array() {
        Some(exts) => exts,
        None => return,
    };

    let block_number = block["header"]["number"]
        .as_str()
        .and_then(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0);

    let _ = ctx.tx.send(Event::BlockUpdate(block_number as u64));

    let block_ts = extract_block_timestamp(extrinsics);

    for (ext_index, ext) in extrinsics.iter().enumerate() {
        let ext_hex = match ext.as_str() {
            Some(s) => s,
            None => continue,
        };
        let ext_bytes = match hex::decode(ext_hex.trim_start_matches("0x")) {
            Ok(b) => b,
            Err(_) => continue,
        };

        let signer = extract_signer(&ext_bytes);
        let remark = extract_remark(&ext_bytes);

        if let (Some(sender), Some(remark_data)) = (signer, remark) {
            process_remark(
                &remark_data,
                &sender,
                ctx,
                block_number,
                ext_index as u16,
                block_ts,
            );
        }
    }
}

/// Process a specific extrinsic (for DAG gap filling).
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
    let remark = extract_remark(&ext_bytes);

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

// Substrate signed extrinsic layout constants
const SIGNED_BIT: u8 = 0x80; // bit 7 of version byte: 1 = signed
const ADDR_TYPE_ACCOUNT: u8 = 0x00; // MultiAddress::Id (raw 32-byte AccountId)
const SIGNED_HEADER_LEN: usize = 99; // version(1) + addr_type(1) + account(32) + sig_type(1) + sig(64)
const MIN_SIGNED_EXTRINSIC: usize = 103; // SIGNED_HEADER_LEN + era(1) + nonce(1) + tip(1) + call(2)
const MIN_SIGNER_PAYLOAD: usize = 34; // version(1) + addr_type(1) + account(32)
const SYSTEM_PALLET: u8 = 0x00;
const REMARK_WITH_EVENT_CALL: u8 = 0x07;
const REMARK_CALL: u8 = 0x09;

/// Extract the remark payload from a system.remark_with_event extrinsic.
fn extract_remark(ext_bytes: &[u8]) -> Option<Vec<u8>> {
    let (_, prefix_len) = decode_compact_prefix(ext_bytes)?;
    let payload = &ext_bytes[prefix_len..];

    // Must be a signed extrinsic with minimum length
    if payload.len() < MIN_SIGNED_EXTRINSIC || payload[0] & SIGNED_BIT == 0 {
        return None;
    }

    // Skip signed header: version + addr_type + account + sig_type + signature
    let mut offset = SIGNED_HEADER_LEN;
    if offset >= payload.len() {
        return None;
    }
    // Era: 0x00 = immortal (1 byte), anything else = mortal (2 bytes)
    if payload[offset] != 0x00 {
        offset += 2; // mortal era
    } else {
        offset += 1; // immortal era
    }
    // Nonce (SCALE compact)
    let (_, nonce_len) = decode_compact_prefix(&payload[offset..])?;
    offset += nonce_len;
    // Tip (SCALE compact)
    let (_, tip_len) = decode_compact_prefix(&payload[offset..])?;
    offset += tip_len;
    // Metadata hash mode byte
    offset += 1;

    // Call: pallet_index(1) + call_index(1)
    if offset + 2 >= payload.len() {
        return None;
    }
    let pallet = payload[offset];
    let call = payload[offset + 1];
    offset += 2;

    // system.remark_with_event or system.remark
    if pallet != SYSTEM_PALLET || (call != REMARK_WITH_EVENT_CALL && call != REMARK_CALL) {
        return None;
    }

    // Remark body (SCALE compact length prefix + data)
    let (remark_len, compact_len) = decode_compact_prefix(&payload[offset..])?;
    offset += compact_len;

    if offset + remark_len > payload.len() {
        return None;
    }
    Some(payload[offset..offset + remark_len].to_vec())
}

fn extract_signer(ext_bytes: &[u8]) -> Option<Pubkey> {
    let (_, prefix_len) = decode_compact_prefix(ext_bytes)?;
    let payload = &ext_bytes[prefix_len..];
    // Signed extrinsic: SIGNED_BIT set, addr_type = AccountId (0x00)
    if payload.len() < MIN_SIGNER_PAYLOAD
        || payload[0] & SIGNED_BIT == 0
        || payload[1] != ADDR_TYPE_ACCOUNT
    {
        return None;
    }
    let mut account = [0u8; 32];
    account.copy_from_slice(&payload[2..34]);
    Some(Pubkey(account))
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
        let (_, prefix_len) = match decode_compact_prefix(&ext_bytes) {
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
        if let Some((ts_ms, _)) = decode_compact_u64(&payload[3..])
            && ts_ms > 1_000_000_000_000
        {
            return ts_ms;
        }
    }
    0
}

fn decode_compact_u64(data: &[u8]) -> Option<(u64, usize)> {
    if data.is_empty() {
        return None;
    }
    let mode = data[0] & 0b11;
    match mode {
        0b00 => Some(((data[0] >> 2) as u64, 1)),
        0b01 => {
            if data.len() < 2 {
                return None;
            }
            let raw = u16::from_le_bytes([data[0], data[1]]);
            Some(((raw >> 2) as u64, 2))
        }
        0b10 => {
            if data.len() < 4 {
                return None;
            }
            let raw = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            Some(((raw >> 2) as u64, 4))
        }
        _ => {
            let bytes_following = ((data[0] >> 2) + 4) as usize;
            if data.len() < 1 + bytes_following {
                return None;
            }
            let mut buf = [0u8; 8];
            let copy_len = bytes_following.min(8);
            buf[..copy_len].copy_from_slice(&data[1..1 + copy_len]);
            Some((u64::from_le_bytes(buf), 1 + bytes_following))
        }
    }
}

fn decode_compact_prefix(data: &[u8]) -> Option<(usize, usize)> {
    if data.is_empty() {
        return None;
    }
    let mode = data[0] & 0b11;
    match mode {
        0b00 => Some(((data[0] >> 2) as usize, 1)),
        0b01 => {
            if data.len() < 2 {
                return None;
            }
            let raw = u16::from_le_bytes([data[0], data[1]]);
            Some(((raw >> 2) as usize, 2))
        }
        0b10 => {
            if data.len() < 4 {
                return None;
            }
            let raw = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            Some(((raw >> 2) as usize, 4))
        }
        _ => None,
    }
}

/// Process a remark payload. Reader is fully stateless -- no group keys needed.
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
        Err(_) => return, // Not a valid SAMP remark
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
            // content = reply_to(6) + continues(6) + body
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
            // content = eph_pubkey(32) + capsules(33*N) + ciphertext
            let scalar = samp::sr25519_signing_scalar(seed);

            let plaintext =
                match samp::decrypt_from_group(&remark.content, &scalar, &remark.nonce, None) {
                    Ok(pt) => pt,
                    Err(_) => return, // Not for us (no matching capsule) or decryption failed
                };

            // Parse plaintext: group_ref(6) + reply_to(6) + continues(6) + body
            let (group_ref, reply_to, continues, body_bytes) =
                match decode_group_content(&plaintext) {
                    Ok(r) => r,
                    Err(_) => return,
                };

            if group_ref.is_zero() {
                // Root message (group creation): body = member_count + pubkeys + first_message
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
                // Emit the root's first message (can be empty)
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
                // Regular message
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
        ContentType::Application(_) => {
            // Application-defined types: not part of SAMP core
        }
    }
}
