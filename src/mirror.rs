use std::sync::mpsc::Sender;

use curve25519_dalek::scalar::Scalar;
use samp::{ContentType, decode_remark};

use crate::error::ChainError;
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
) -> Result<(), ChainError> {
    let _ = tx.send(Event::Status("Catching up...".into()));
    let client = reqwest::Client::new();
    let base = mirror_url.trim_end_matches('/');

    let health: HealthResp = client
        .get(format!("{base}/v1/health"))
        .send()
        .await
        .map_err(|e| ChainError::Http(format!("health: {e}")))?
        .json()
        .await
        .map_err(|e| ChainError::Parse(format!("health json: {e}")))?;

    if health.ss58_prefix != expected_ss58_prefix {
        return Err(ChainError::MirrorChainMismatch {
            chain: health.chain,
            got: health.ss58_prefix,
            expected: expected_ss58_prefix,
        });
    }

    let channels: Vec<ChannelResp> = client
        .get(format!("{base}/v1/channels"))
        .send()
        .await
        .map_err(|e| ChainError::Http(format!("channels: {e}")))?
        .json()
        .await
        .map_err(|e| ChainError::Parse(format!("channels json: {e}")))?;

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

    for ch in &subscribed_channels {
        let (ch_block, ch_index) = (ch.block, ch.index);
        let remarks: Vec<RemarkResp> = client
            .get(format!(
                "{base}/v1/channels/{ch_block}/{ch_index}/messages?after={last_block}"
            ))
            .send()
            .await
            .map_err(|e| ChainError::Http(format!("channel messages: {e}")))?
            .json()
            .await
            .map_err(|e| ChainError::Parse(format!("channel messages json: {e}")))?;

        for r in remarks {
            process_remark_from_mirror(&r, tx);
        }
    }

    let scalar = samp::sr25519_signing_scalar(seed);

    for r in fetch_remarks(&client, base, 0x10, last_block, "public").await? {
        process_public_remark(&r, my_pubkey, tx);
    }
    for r in fetch_remarks(&client, base, 0x11, last_block, "encrypted").await? {
        process_encrypted_remark(&r, &scalar, my_pubkey, seed, tx);
    }
    for r in fetch_remarks(&client, base, 0x12, last_block, "thread").await? {
        process_encrypted_remark(&r, &scalar, my_pubkey, seed, tx);
    }
    for r in fetch_remarks(&client, base, 0x15, last_block, "group").await? {
        process_group_remark(&r, &scalar, tx);
    }

    let _ = tx.send(Event::Status("All caught up".into()));
    Ok(())
}

async fn fetch_remarks(
    client: &reqwest::Client,
    base: &str,
    type_byte: u8,
    after: u64,
    label: &str,
) -> Result<Vec<RemarkResp>, ChainError> {
    client
        .get(format!(
            "{base}/v1/remarks?type=0x{type_byte:02x}&after={after}"
        ))
        .send()
        .await
        .map_err(|e| ChainError::Http(format!("{label}: {e}")))?
        .json()
        .await
        .map_err(|e| ChainError::Parse(format!("{label} json: {e}")))
}

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

fn process_public_remark(r: &RemarkResp, my_pubkey: &Pubkey, tx: &Sender<Event>) {
    let bytes = match hex::decode(&r.remark) {
        Ok(b) => b,
        Err(_) => return,
    };
    let remark = match decode_remark(&bytes) {
        Ok(r) => r,
        Err(_) => return,
    };
    if remark.content_type != ContentType::Public {
        return;
    }
    let sender = crate::util::pubkey_from_ss58(&r.sender).unwrap_or(Pubkey::ZERO);
    if remark.recipient != my_pubkey.0 && sender != *my_pubkey {
        return;
    }
    let _ = tx.send(Event::NewMessage {
        sender,
        content_type: remark.content_type.to_byte(),
        recipient: Pubkey(remark.recipient),
        decrypted_body: String::from_utf8(remark.content).ok(),
        thread_ref: BlockRef::ZERO,
        reply_to: BlockRef::ZERO,
        continues: BlockRef::ZERO,
        block_number: r.block,
        ext_index: r.index,
        timestamp: r.timestamp,
    });
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

    let tag = match samp::check_view_tag(&remark, scalar) {
        Ok(t) => t,
        Err(_) => return,
    };

    let (plaintext, is_mine) = if tag == remark.view_tag {
        match samp::decrypt(&remark, scalar) {
            Ok(pt) => (pt, false),
            Err(_) => return,
        }
    } else {
        match samp::decrypt_as_sender(&remark, seed) {
            Ok(pt) => match samp::unseal_recipient(&remark, seed) {
                Ok(_) => (pt, true),
                Err(_) => return,
            },
            Err(_) => return,
        }
    };

    let ct = remark.content_type.to_byte();
    let mut recipient = remark.recipient;
    if is_mine && let Ok(r) = samp::unseal_recipient(&remark, seed) {
        recipient = r;
    }

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

    let plaintext = match samp::decrypt_from_group(&remark.content, scalar, &remark.nonce, None) {
        Ok(pt) => pt,
        Err(_) => return,
    };

    let (group_ref, reply_to, continues, body_bytes) = match samp::decode_group_content(&plaintext)
    {
        Ok(r) => r,
        Err(_) => return,
    };

    let sender_pubkey = crate::util::pubkey_from_ss58(&r.sender).unwrap_or(Pubkey::ZERO);

    if group_ref.is_zero() {
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
