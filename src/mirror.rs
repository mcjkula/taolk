use std::sync::mpsc::Sender;

use curve25519_dalek::scalar::Scalar;
use samp::{ContentType, decode_remark};

use crate::event::Event;
use crate::types::{BlockRef, Pubkey};

#[derive(serde::Deserialize)]
struct HealthResp {
    chain: String,
    ss58_prefix: u16,
}

#[derive(serde::Deserialize)]
struct ChannelResp {
    block: u32,
    index: u16,
    creator: String,
    name: String,
    description: String,
}

#[derive(serde::Deserialize)]
struct RemarkResp {
    block: u32,
    index: u16,
    sender: String,
    timestamp: u64,
    remark: String,
}

/// Sync historical data from a SAMP mirror. Produces events on the same channel as the chain reader.
pub async fn sync(
    mirror_url: &str,
    expected_ss58_prefix: u16,
    seed: &[u8; 32],
    my_pubkey: &Pubkey,
    subscribed_channels: Vec<BlockRef>,
    last_block: u64,
    tx: Sender<Event>,
) {
    if let Err(e) = sync_inner(
        mirror_url,
        expected_ss58_prefix,
        seed,
        my_pubkey,
        subscribed_channels,
        last_block,
        &tx,
    )
    .await
    {
        let _ = tx.send(Event::Error(format!("Could not reach mirror: {e}")));
    }
    let _ = tx.send(Event::CatchupComplete);
}

async fn sync_inner(
    mirror_url: &str,
    expected_ss58_prefix: u16,
    seed: &[u8; 32],
    my_pubkey: &Pubkey,
    subscribed_channels: Vec<BlockRef>,
    last_block: u64,
    tx: &Sender<Event>,
) -> Result<(), String> {
    let _ = tx.send(Event::Status("Catching up...".into()));
    let client = reqwest::Client::new();
    let base = mirror_url.trim_end_matches('/');

    // 1. Health check: verify SS58 prefix matches (same chain)
    let health: HealthResp = client
        .get(format!("{base}/v1/health"))
        .send()
        .await
        .map_err(|e| format!("health: {e}"))?
        .json()
        .await
        .map_err(|e| format!("health json: {e}"))?;

    if health.ss58_prefix != expected_ss58_prefix {
        return Err(format!(
            "chain mismatch: mirror serves '{}' (SS58 prefix {}), expected prefix {}",
            health.chain, health.ss58_prefix, expected_ss58_prefix
        ));
    }

    // 2. Fetch all channels
    let channels: Vec<ChannelResp> = client
        .get(format!("{base}/v1/channels"))
        .send()
        .await
        .map_err(|e| format!("channels: {e}"))?
        .json()
        .await
        .map_err(|e| format!("channels json: {e}"))?;

    for ch in &channels {
        let _ = tx.send(Event::ChannelDiscovered {
            name: ch.name.clone(),
            description: ch.description.clone(),
            creator_ss58: ch.creator.clone(),
            channel_ref: BlockRef {
                block: ch.block,
                index: ch.index,
            },
        });
    }

    // 3. Fetch messages for subscribed channels
    for ch in &subscribed_channels {
        let (ch_block, ch_index) = (ch.block, ch.index);
        let remarks: Vec<RemarkResp> = client
            .get(format!(
                "{base}/v1/channels/{ch_block}/{ch_index}/messages?after={last_block}"
            ))
            .send()
            .await
            .map_err(|e| format!("channel messages: {e}"))?
            .json()
            .await
            .map_err(|e| format!("channel messages json: {e}"))?;

        for r in remarks {
            process_remark_from_mirror(&r, tx);
        }
    }

    // 4. Fetch encrypted remarks (client checks view tags locally)
    let encrypted: Vec<RemarkResp> = client
        .get(format!("{base}/v1/remarks?type=0x11&after={last_block}"))
        .send()
        .await
        .map_err(|e| format!("encrypted: {e}"))?
        .json()
        .await
        .map_err(|e| format!("encrypted json: {e}"))?;

    let scalar = samp::sr25519_signing_scalar(seed);
    for r in encrypted {
        process_encrypted_remark(&r, &scalar, my_pubkey, seed, tx);
    }

    // 5. Fetch thread remarks
    let threads: Vec<RemarkResp> = client
        .get(format!("{base}/v1/remarks?type=0x12&after={last_block}"))
        .send()
        .await
        .map_err(|e| format!("threads: {e}"))?
        .json()
        .await
        .map_err(|e| format!("threads json: {e}"))?;

    for r in threads {
        process_encrypted_remark(&r, &scalar, my_pubkey, seed, tx);
    }

    // 6. Fetch group remarks (client decrypts capsules locally)
    let groups: Vec<RemarkResp> = client
        .get(format!("{base}/v1/remarks?type=0x15&after={last_block}"))
        .send()
        .await
        .map_err(|e| format!("groups: {e}"))?
        .json()
        .await
        .map_err(|e| format!("groups json: {e}"))?;

    for r in groups {
        process_group_remark(&r, &scalar, tx);
    }

    let _ = tx.send(Event::Status("All caught up".into()));
    Ok(())
}

/// Fetch a single channel's messages from the mirror. Called when subscribing or pressing `r`.
pub async fn fetch_channel(mirror_url: &str, channel_ref: BlockRef, tx: Sender<Event>) {
    let base = mirror_url.trim_end_matches('/');
    let client = reqwest::Client::new();
    let (ch_block, ch_index) = (channel_ref.block, channel_ref.index);

    let remarks: Vec<RemarkResp> = match client
        .get(format!(
            "{base}/v1/channels/{ch_block}/{ch_index}/messages?after=0"
        ))
        .send()
        .await
    {
        Ok(resp) => match resp.json().await {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(Event::Error(format!("Could not load messages: {e}")));
                return;
            }
        },
        Err(e) => {
            let _ = tx.send(Event::Error(format!("Could not load messages: {e}")));
            return;
        }
    };

    for r in remarks {
        process_remark_from_mirror(&r, &tx);
    }
}

fn process_remark_from_mirror(r: &RemarkResp, tx: &Sender<Event>) {
    let remark_bytes = match hex::decode(&r.remark) {
        Ok(b) => b,
        Err(_) => return,
    };
    let remark = match decode_remark(&remark_bytes) {
        Ok(r) => r,
        Err(_) => return,
    };

    if remark.content_type == ContentType::Channel
        && let Ok((reply_to, continues, body_bytes)) = samp::decode_channel_content(&remark.content)
        && let Ok(body) = String::from_utf8(body_bytes.to_vec())
    {
        let sender = crate::util::pubkey_from_ss58(&r.sender).unwrap_or(Pubkey::ZERO);
        let sender_ss58 = if sender == Pubkey::ZERO {
            r.sender.clone()
        } else {
            crate::util::ss58_short(&sender)
        };
        let _ = tx.send(Event::NewChannelMessage {
            sender,
            sender_ss58,
            channel_ref: samp::channel_ref_from_recipient(&remark.recipient),
            body,
            reply_to,
            continues,
            block_number: r.block,
            ext_index: r.index,
            timestamp: r.timestamp,
        });
    }
}

fn process_encrypted_remark(
    r: &RemarkResp,
    scalar: &Scalar,
    my_pubkey: &Pubkey,
    seed: &[u8; 32],
    tx: &Sender<Event>,
) {
    let remark_bytes = match hex::decode(&r.remark) {
        Ok(b) => b,
        Err(_) => return,
    };
    let remark = match decode_remark(&remark_bytes) {
        Ok(r) => r,
        Err(_) => return,
    };

    // Check view tag (recipient path)
    let tag = match samp::check_view_tag(scalar, &remark.content) {
        Ok(t) => t,
        Err(_) => return,
    };

    // Try recipient decryption first
    let (plaintext, is_mine) = if tag == remark.view_tag {
        match samp::decrypt(&remark.content, scalar, &remark.nonce) {
            Ok(pt) => (pt, false),
            Err(_) => return,
        }
    } else {
        // Try sender self-decryption
        match samp::decrypt_as_sender(&remark.content, seed, &remark.nonce) {
            Ok(pt) => {
                // Verify the unsealed recipient is valid
                match samp::unseal_recipient(&remark.content, seed, &remark.nonce) {
                    Ok(_) => (pt, true),
                    Err(_) => return,
                }
            }
            Err(_) => return, // Not for us and not from us
        }
    };

    let ct = remark.content_type.to_byte();
    let mut recipient = remark.recipient;
    if is_mine && let Ok(r) = samp::unseal_recipient(&remark.content, seed, &remark.nonce) {
        recipient = r;
    }

    // Parse thread content if 0x12
    let (body, thread_ref, reply_to, continues) = if ct & 0x0F == 0x02 {
        match samp::decode_thread_content(&plaintext) {
            Ok((thread, reply_to, continues, body_bytes)) => (
                String::from_utf8(body_bytes.to_vec()).ok(),
                thread,
                reply_to,
                continues,
            ),
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
        sender: if is_mine { *my_pubkey } else { Pubkey::ZERO },
        content_type: ct,
        recipient: Pubkey(recipient),
        decrypted_body: body,
        thread_ref,
        reply_to,
        continues,
        block_number: r.block,
        ext_index: r.index,
        timestamp: r.timestamp,
    });
}

fn process_group_remark(r: &RemarkResp, scalar: &Scalar, tx: &Sender<Event>) {
    let remark_bytes = match hex::decode(&r.remark) {
        Ok(b) => b,
        Err(_) => return,
    };
    let remark = match decode_remark(&remark_bytes) {
        Ok(r) => r,
        Err(_) => return,
    };

    // Decrypt using capsule scanning (stateless)
    let plaintext = match samp::decrypt_from_group(&remark.content, scalar, &remark.nonce, None) {
        Ok(pt) => pt,
        Err(_) => return, // Not for us
    };

    let (group_ref, reply_to, continues, body_bytes) = match samp::decode_group_content(&plaintext)
    {
        Ok(r) => r,
        Err(_) => return,
    };

    // Sender pubkey: parse from the mirror's SS58
    let sender_pubkey = crate::util::pubkey_from_ss58(&r.sender).unwrap_or(Pubkey::ZERO);

    if group_ref.is_zero() {
        // Root message: parse member list
        let (members, first_msg) = match samp::decode_group_members(body_bytes) {
            Ok(r) => r,
            Err(_) => return,
        };
        let members = members.into_iter().map(Pubkey).collect();
        let _ = tx.send(Event::GroupDiscovered {
            creator_pubkey: sender_pubkey,
            group_ref: BlockRef {
                block: r.block,
                index: r.index,
            },
            members,
        });
        let body = String::from_utf8(first_msg.to_vec()).unwrap_or_default();
        let sender_ss58 = crate::util::ss58_short(&sender_pubkey);
        let _ = tx.send(Event::NewGroupMessage {
            sender: sender_pubkey,
            sender_ss58,
            group_ref: BlockRef {
                block: r.block,
                index: r.index,
            },
            body,
            reply_to: BlockRef::ZERO,
            continues: BlockRef::ZERO,
            block_number: r.block,
            ext_index: r.index,
            timestamp: r.timestamp,
        });
    } else {
        let body = match String::from_utf8(body_bytes.to_vec()) {
            Ok(b) => b,
            Err(_) => return,
        };
        let sender_ss58 = crate::util::ss58_short(&sender_pubkey);
        let _ = tx.send(Event::NewGroupMessage {
            sender: sender_pubkey,
            sender_ss58,
            group_ref,
            body,
            reply_to,
            continues,
            block_number: r.block,
            ext_index: r.index,
            timestamp: r.timestamp,
        });
    }
}
