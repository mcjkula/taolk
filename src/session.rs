use crate::conversation::{
    Channel, ChannelInfo, Group, InboxMessage, NewMessage, Thread, ThreadMessage,
};
use crate::db::Db;
use crate::error::{Result, SdkError};
use crate::types::{BlockRef, Pubkey};
use chrono::{DateTime, Utc};
use schnorrkel::keys::{ExpansionMode, MiniSecretKey};
use std::collections::HashMap;
use std::sync::mpsc;
use zeroize::Zeroizing;

fn rand_nonce() -> [u8; 12] {
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).expect("OS RNG");
    nonce
}

fn refresh_message_gaps(messages: &mut [ThreadMessage], has_msg: impl Fn(BlockRef) -> bool) {
    for msg in messages {
        msg.has_gap = (msg.reply_to != BlockRef::ZERO && !has_msg(msg.reply_to))
            || (msg.continues != BlockRef::ZERO && !has_msg(msg.continues));
    }
}

fn reindex(index: &mut HashMap<BlockRef, usize>, removed: usize) {
    index.retain(|_, v| {
        if *v > removed {
            *v -= 1;
        }
        true
    });
}

pub struct Session {
    pub keypair: schnorrkel::Keypair,
    pub my_ss58: String,
    seed: Zeroizing<[u8; 32]>,
    pub node_url: String,
    pub chain_info: crate::extrinsic::ChainInfo,
    pub block_number: u64,
    pub inbox: Vec<InboxMessage>,
    pub outbox: Vec<InboxMessage>,
    pub threads: Vec<Thread>,
    pub channels: Vec<Channel>,
    pub groups: Vec<Group>,
    pub db: Db,
    thread_index: HashMap<BlockRef, usize>,
    channel_index: HashMap<BlockRef, usize>,
    group_index: HashMap<BlockRef, usize>,
    pub known_channels: Vec<ChannelInfo>,
    known_channel_index: HashMap<BlockRef, usize>,
    pub peer_pubkeys: HashMap<String, Pubkey>,
    pub token_symbol: String,
    pub token_decimals: u32,
    pub balance: Option<u128>,
    pub has_mirror: bool,
}

impl Session {
    pub fn new(
        keypair: schnorrkel::Keypair,
        seed: Zeroizing<[u8; 32]>,
        node_url: String,
        chain_info: crate::extrinsic::ChainInfo,
        db: Db,
    ) -> Self {
        let pk = Pubkey(keypair.public.to_bytes());
        let my_ss58 = crate::util::ss58_from_pubkey(&pk);
        Self {
            keypair,
            my_ss58,
            seed,
            node_url,
            chain_info,
            block_number: 0,
            inbox: Vec::new(),
            outbox: Vec::new(),
            threads: Vec::new(),
            channels: Vec::new(),
            groups: Vec::new(),
            db,
            thread_index: HashMap::new(),
            channel_index: HashMap::new(),
            group_index: HashMap::new(),
            known_channels: Vec::new(),
            known_channel_index: HashMap::new(),
            peer_pubkeys: HashMap::new(),
            token_symbol: String::new(),
            token_decimals: 0,
            balance: None,
            has_mirror: false,
        }
    }

    pub async fn start(
        seed: &[u8; 32],
        node_url: &str,
        wallet_name: &str,
    ) -> Result<(Self, mpsc::Receiver<crate::event::Event>)> {
        Self::start_with_mirrors(seed, node_url, wallet_name, &[]).await
    }

    pub async fn start_with_mirrors(
        seed: &[u8; 32],
        node_url: &str,
        wallet_name: &str,
        mirror_urls: &[String],
    ) -> Result<(Self, mpsc::Receiver<crate::event::Event>)> {
        let msk = MiniSecretKey::from_bytes(seed)
            .map_err(|e| SdkError::Wallet(format!("Invalid seed: {e}")))?;
        let keypair = msk.expand_to_keypair(ExpansionMode::Ed25519);
        let my_pubkey = Pubkey(keypair.public.to_bytes());

        let chain_info = crate::extrinsic::fetch_chain_info(node_url)
            .await
            .map_err(|e| SdkError::Chain(format!("Failed to fetch chain info: {e}")))?;

        let (symbol, decimals) = crate::extrinsic::fetch_token_info(node_url)
            .await
            .unwrap_or_else(|_| ("TAO".into(), 9));

        let db = Db::open(wallet_name, seed, &chain_info.genesis_hash)
            .map_err(|e| SdkError::Database(e.to_string()))?;

        let mut session = Self::new(
            keypair,
            Zeroizing::new(*seed),
            node_url.to_string(),
            chain_info,
            db,
        );
        session.token_symbol = symbol;
        session.token_decimals = decimals;
        session.load_from_db();

        if let Ok(bal) = crate::extrinsic::fetch_balance(
            node_url,
            &my_pubkey,
            &session.chain_info.account_info_layout,
        )
        .await
        {
            session.balance = Some(bal);
        }

        let (tx, rx) = mpsc::channel();

        // Chain subscription
        {
            let url = node_url.to_string();
            let etx = tx.clone();
            let sc = zeroize::Zeroizing::new(*seed);
            tokio::spawn(async move {
                crate::chain::subscribe_blocks(&url, my_pubkey, sc, etx).await;
            });
        }

        // Mirror sync
        session.has_mirror = !mirror_urls.is_empty();
        if session.has_mirror {
            let subscribed: Vec<BlockRef> =
                session.channels.iter().map(|c| c.channel_ref).collect();
            for mirror_url in mirror_urls {
                let url = mirror_url.clone();
                let sc = zeroize::Zeroizing::new(*seed);
                let pk = my_pubkey;
                let channels = subscribed.clone();
                let etx = tx.clone();
                tokio::spawn(async move {
                    crate::mirror::sync(&url, 42, &sc, &pk, channels, 0, etx).await;
                });
            }
        }

        Ok((session, rx))
    }

    pub fn pubkey(&self) -> Pubkey {
        Pubkey(self.keypair.public.to_bytes())
    }

    pub fn ss58(&self) -> &str {
        &self.my_ss58
    }

    pub fn load_from_db(&mut self) {
        self.peer_pubkeys = self.db.load_all_peers();
        let (inbox, outbox) = self.db.load_inbox();
        self.inbox = inbox;
        self.outbox = outbox;

        for (thread_ref, peer_ss58, messages) in self.db.load_threads() {
            let i = self.threads.len();
            self.thread_index.insert(thread_ref, i);
            let peer_pubkey = self.db.get_peer_pubkey(&peer_ss58).unwrap_or(Pubkey::ZERO);
            if peer_pubkey != Pubkey::ZERO {
                self.peer_pubkeys.insert(peer_ss58.clone(), peer_pubkey);
            }
            let msg_count = messages.len();
            self.threads.push(Thread {
                thread_ref,
                peer_ss58,
                peer_pubkey,
                messages,
                draft: String::new(),
                last_read: msg_count,
            });
            self.refresh_gaps(i);
        }

        for (channel_ref, name, description, creator_ss58, messages) in self.db.load_channels() {
            let i = self.channels.len();
            let msg_count = messages.len();
            self.channel_index.insert(channel_ref, i);
            self.channels.push(Channel {
                name,
                description,
                creator_ss58,
                channel_ref,
                messages,
                draft: String::new(),
                last_read: msg_count,
            });
            self.refresh_channel_gaps(i);
        }

        for (channel_ref, name, description, creator_ss58) in self.db.load_known_channels() {
            if self.known_channel_index.contains_key(&channel_ref) {
                continue;
            }
            let i = self.known_channels.len();
            self.known_channel_index.insert(channel_ref, i);
            self.known_channels.push(ChannelInfo {
                name,
                description,
                creator_ss58,
                channel_ref,
            });
        }

        let me = self.pubkey();
        for (group_ref, creator_pubkey, members) in self.db.load_groups() {
            for member in &members {
                if *member == me {
                    continue;
                }
                let ss58 = crate::util::ss58_short(member);
                self.peer_pubkeys.insert(ss58.clone(), *member);
                self.db.upsert_peer(&ss58, member);
            }
            let i = self.groups.len();
            let messages = self.db.load_group_messages(group_ref);
            let msg_count = messages.len();
            self.group_index.insert(group_ref, i);
            self.groups.push(Group {
                creator_pubkey,
                group_ref,
                members,
                messages,
                draft: String::new(),
                last_read: msg_count,
            });
            self.refresh_group_gaps(i);
        }
    }

    pub fn add_inbox_message(
        &mut self,
        sender: Pubkey,
        recipient: Pubkey,
        timestamp: DateTime<Utc>,
        body: String,
        content_type: u8,
        block_ref: BlockRef,
    ) {
        let is_mine = sender == self.pubkey();
        let peer = if is_mine { recipient } else { sender };
        let peer_ss58 = crate::util::ss58_short(&peer);
        self.peer_pubkeys.insert(peer_ss58.clone(), peer);
        self.db.upsert_peer(&peer_ss58, &peer);

        let (block_number, ext_index) = (block_ref.block, block_ref.index);
        if block_number > 0 {
            let already = if is_mine {
                self.outbox
                    .iter()
                    .any(|m| m.block_number == block_number && m.ext_index == ext_index)
            } else {
                self.inbox
                    .iter()
                    .any(|m| m.block_number == block_number && m.ext_index == ext_index)
            };
            if already {
                return;
            }
        }
        let msg = InboxMessage {
            peer_ss58,
            timestamp,
            body,
            content_type,
            is_mine,
            block_number,
            ext_index,
        };
        self.db.insert_inbox(&msg);
        let target = if is_mine {
            &mut self.outbox
        } else {
            &mut self.inbox
        };
        target.push(msg);
        target.sort_by_key(|m| (m.block_number, m.ext_index));
    }

    pub fn add_thread_message(
        &mut self,
        sender: Pubkey,
        recipient: Pubkey,
        mut thread_ref: BlockRef,
        msg: NewMessage,
    ) {
        let is_mine = sender == self.pubkey();
        let peer = if is_mine { recipient } else { sender };
        let peer_ss58 = if is_mine {
            crate::util::ss58_short(&recipient)
        } else {
            msg.sender_ss58.clone()
        };
        self.peer_pubkeys.insert(peer_ss58.clone(), peer);
        self.db.upsert_peer(&peer_ss58, &peer);

        if self.db.has_message_at(BlockRef {
            block: msg.block_number,
            index: msg.ext_index,
        }) {
            return;
        }

        if thread_ref == BlockRef::ZERO {
            thread_ref = BlockRef {
                block: msg.block_number,
                index: msg.ext_index,
            };
        }

        let idx = if let Some(&i) = self.thread_index.get(&thread_ref) {
            i
        } else if let Some(&i) = self.thread_index.get(&BlockRef::ZERO) {
            if self.threads[i].peer_pubkey == peer {
                self.thread_index.remove(&BlockRef::ZERO);
                self.thread_index.insert(thread_ref, i);
                self.threads[i].thread_ref = thread_ref;
                i
            } else {
                let i = self.threads.len();
                self.thread_index.insert(thread_ref, i);
                self.threads.push(Thread {
                    thread_ref,
                    peer_ss58: peer_ss58.clone(),
                    peer_pubkey: peer,
                    messages: Vec::new(),
                    draft: String::new(),
                    last_read: 0,
                });
                i
            }
        } else {
            let i = self.threads.len();
            self.thread_index.insert(thread_ref, i);
            self.threads.push(Thread {
                thread_ref,
                peer_ss58: peer_ss58.clone(),
                peer_pubkey: peer,
                messages: Vec::new(),
                draft: String::new(),
                last_read: 0,
            });
            i
        };

        let has_gap = (msg.reply_to != BlockRef::ZERO && !self.db.has_message_at(msg.reply_to))
            || (msg.continues != BlockRef::ZERO && !self.db.has_message_at(msg.continues));

        let tm = ThreadMessage {
            sender_ss58: msg.sender_ss58,
            timestamp: msg.timestamp,
            body: msg.body,
            is_mine,
            reply_to: msg.reply_to,
            continues: msg.continues,
            block_number: msg.block_number,
            ext_index: msg.ext_index,
            has_gap,
        };

        self.db
            .insert_thread_message(thread_ref, &peer_ss58, &tm, tm.block_number, tm.ext_index);
        self.threads[idx].messages.push(tm);
        self.threads[idx]
            .messages
            .sort_by_key(|m| (m.block_number, m.ext_index));
    }

    pub fn add_channel_message(&mut self, channel_ref: BlockRef, msg: NewMessage) {
        if self.db.has_channel_message_at(BlockRef {
            block: msg.block_number,
            index: msg.ext_index,
        }) {
            return;
        }
        let idx = match self.channel_index.get(&channel_ref) {
            Some(&i) => i,
            None => return,
        };

        let is_mine = msg.sender_ss58 == crate::util::ss58_short(&self.pubkey());

        let has_gap = (msg.reply_to != BlockRef::ZERO
            && !self.db.has_channel_message_at(msg.reply_to))
            || (msg.continues != BlockRef::ZERO && !self.db.has_channel_message_at(msg.continues));

        let tm = ThreadMessage {
            sender_ss58: msg.sender_ss58,
            timestamp: msg.timestamp,
            body: msg.body,
            is_mine,
            reply_to: msg.reply_to,
            continues: msg.continues,
            block_number: msg.block_number,
            ext_index: msg.ext_index,
            has_gap,
        };

        self.db
            .insert_channel_message(channel_ref, &tm, tm.block_number, tm.ext_index);
        self.channels[idx].messages.push(tm);
        self.channels[idx]
            .messages
            .sort_by_key(|m| (m.block_number, m.ext_index));
    }

    pub fn discover_channel(
        &mut self,
        name: String,
        description: String,
        creator_ss58: String,
        channel_ref: BlockRef,
    ) {
        if let Some(&idx) = self.channel_index.get(&channel_ref) {
            self.db
                .update_channel_meta(channel_ref, &name, &description, &creator_ss58);
            self.channels[idx].name = name;
            self.channels[idx].description = description;
            self.channels[idx].creator_ss58 = creator_ss58;
            return;
        }
        if let Some(&idx) = self.channel_index.get(&BlockRef::ZERO)
            && self.channels[idx].name == name
        {
            self.channel_index.remove(&BlockRef::ZERO);
            self.channel_index.insert(channel_ref, idx);
            self.channels[idx].channel_ref = channel_ref;
            self.db
                .insert_channel(channel_ref, &name, &description, &creator_ss58);
            self.db
                .insert_known_channel(channel_ref, &name, &description, &creator_ss58);
            self.channels[idx].description = description.clone();
            self.channels[idx].creator_ss58 = creator_ss58.clone();
            let ki = self.known_channels.len();
            self.known_channel_index.insert(channel_ref, ki);
            self.known_channels.push(ChannelInfo {
                name,
                description,
                creator_ss58,
                channel_ref,
            });
            return;
        }
        if let Some(&idx) = self.known_channel_index.get(&channel_ref) {
            self.db
                .update_known_channel_meta(channel_ref, &name, &description, &creator_ss58);
            self.known_channels[idx].name = name;
            self.known_channels[idx].description = description;
            self.known_channels[idx].creator_ss58 = creator_ss58;
            return;
        }
        let i = self.known_channels.len();
        self.known_channel_index.insert(channel_ref, i);
        self.db
            .insert_known_channel(channel_ref, &name, &description, &creator_ss58);
        self.known_channels.push(ChannelInfo {
            name,
            description,
            creator_ss58,
            channel_ref,
        });
    }

    pub fn subscribe_channel(&mut self, channel_ref: BlockRef) -> usize {
        if let Some(&idx) = self.channel_index.get(&channel_ref) {
            return idx;
        }
        let (name, description, creator_ss58) =
            if let Some(&ki) = self.known_channel_index.get(&channel_ref) {
                let info = &self.known_channels[ki];
                (
                    info.name.clone(),
                    info.description.clone(),
                    info.creator_ss58.clone(),
                )
            } else {
                ("Loading...".into(), String::new(), String::new())
            };
        let i = self.channels.len();
        self.channel_index.insert(channel_ref, i);
        self.db
            .insert_channel(channel_ref, &name, &description, &creator_ss58);
        self.channels.push(Channel {
            name,
            description,
            creator_ss58,
            channel_ref,
            messages: Vec::new(),
            draft: String::new(),
            last_read: 0,
        });
        i
    }

    pub fn is_subscribed(&self, channel_ref: &BlockRef) -> bool {
        self.channel_index.contains_key(channel_ref)
    }

    pub fn channel_idx(&self, channel_ref: &BlockRef) -> Option<usize> {
        self.channel_index.get(channel_ref).copied()
    }

    pub fn unsubscribe_channel(&mut self, idx: usize) -> Option<String> {
        let channel = self.channels.get(idx)?;
        let name = channel.name.clone();
        let channel_ref = channel.channel_ref;
        self.channel_index.remove(&channel_ref);
        self.db.delete_channel(channel_ref);
        self.channels.remove(idx);
        reindex(&mut self.channel_index, idx);
        Some(name)
    }

    pub fn create_pending_group(&mut self, creator_pubkey: Pubkey, members: Vec<Pubkey>) -> usize {
        let i = self.groups.len();
        self.group_index.insert(BlockRef::ZERO, i);
        self.groups.push(Group {
            creator_pubkey,
            group_ref: BlockRef::ZERO,
            members,
            messages: Vec::new(),
            draft: String::new(),
            last_read: 0,
        });
        i
    }

    pub fn create_pending_channel(&mut self, name: String, creator_ss58: String) -> usize {
        let i = self.channels.len();
        self.channel_index.insert(BlockRef::ZERO, i);
        self.channels.push(Channel {
            name,
            description: String::new(),
            creator_ss58,
            channel_ref: BlockRef::ZERO,
            messages: Vec::new(),
            draft: String::new(),
            last_read: 0,
        });
        i
    }

    pub fn refresh_gaps(&mut self, thread_idx: usize) {
        if let Some(thread) = self.threads.get_mut(thread_idx) {
            refresh_message_gaps(&mut thread.messages, |br| self.db.has_message_at(br));
        }
    }

    pub fn refresh_channel_gaps(&mut self, chan_idx: usize) {
        if let Some(ch) = self.channels.get_mut(chan_idx) {
            refresh_message_gaps(&mut ch.messages, |br| self.db.has_channel_message_at(br));
        }
    }

    pub fn refresh_group_gaps(&mut self, group_idx: usize) {
        if let Some(g) = self.groups.get_mut(group_idx) {
            refresh_message_gaps(&mut g.messages, |br| self.db.has_group_message_at(br));
        }
    }

    pub fn discover_group(
        &mut self,
        creator_pubkey: Pubkey,
        group_ref: BlockRef,
        members: Vec<Pubkey>,
    ) {
        let me = self.pubkey();
        for member in &members {
            if *member == me {
                continue;
            }
            let ss58 = crate::util::ss58_short(member);
            self.peer_pubkeys.insert(ss58.clone(), *member);
            self.db.upsert_peer(&ss58, member);
        }
        if self.group_index.contains_key(&group_ref) {
            return;
        }
        if let Some(&idx) = self.group_index.get(&BlockRef::ZERO)
            && self.groups[idx].members == members
        {
            self.group_index.remove(&BlockRef::ZERO);
            self.group_index.insert(group_ref, idx);
            self.groups[idx].group_ref = group_ref;
            self.groups[idx].creator_pubkey = creator_pubkey;
            return;
        }
        let i = self.groups.len();
        self.group_index.insert(group_ref, i);
        self.groups.push(Group {
            creator_pubkey,
            group_ref,
            members,
            messages: Vec::new(),
            draft: String::new(),
            last_read: 0,
        });
    }

    pub fn add_group_message(&mut self, group_ref: BlockRef, msg: NewMessage) {
        if self.db.has_group_message_at(BlockRef {
            block: msg.block_number,
            index: msg.ext_index,
        }) {
            return;
        }
        let idx = match self.group_index.get(&group_ref) {
            Some(&i) => i,
            None => return,
        };

        let is_mine = msg.sender_ss58 == crate::util::ss58_short(&self.pubkey());

        let has_gap = (msg.reply_to != BlockRef::ZERO
            && !self.db.has_group_message_at(msg.reply_to))
            || (msg.continues != BlockRef::ZERO && !self.db.has_group_message_at(msg.continues));

        let tm = ThreadMessage {
            sender_ss58: msg.sender_ss58,
            timestamp: msg.timestamp,
            body: msg.body,
            is_mine,
            reply_to: msg.reply_to,
            continues: msg.continues,
            block_number: msg.block_number,
            ext_index: msg.ext_index,
            has_gap,
        };

        self.db
            .insert_group_message(group_ref, &tm, tm.block_number, tm.ext_index);
        self.groups[idx].messages.push(tm);
        self.groups[idx]
            .messages
            .sort_by_key(|m| (m.block_number, m.ext_index));
    }

    pub fn create_thread(&mut self, pubkey: Pubkey) -> Result<usize> {
        if pubkey == self.pubkey() {
            return Err(SdkError::Other("Cannot message yourself".into()));
        }
        let peer_ss58 = crate::util::ss58_short(&pubkey);
        self.peer_pubkeys.insert(peer_ss58.clone(), pubkey);
        self.db.upsert_peer(&peer_ss58, &pubkey);

        let idx = self.threads.len();
        self.thread_index.insert(BlockRef::ZERO, idx);
        self.threads.push(Thread {
            thread_ref: BlockRef::ZERO,
            peer_ss58,
            peer_pubkey: pubkey,
            messages: Vec::new(),
            draft: String::new(),
            last_read: 0,
        });

        Ok(idx)
    }

    pub fn known_contacts(&self) -> Vec<(String, Pubkey)> {
        self.peer_pubkeys
            .iter()
            .map(|(ss58, pk)| (ss58.clone(), *pk))
            .collect()
    }

    // -----------------------------------------------------------------------
    // Remark builders — encode SAMP messages ready for on-chain submission
    // -----------------------------------------------------------------------

    pub fn build_public_message(&self, recipient: &Pubkey, body: &str) -> Result<Vec<u8>> {
        Ok(samp::encode_public(&recipient.0, body.as_bytes()))
    }

    pub fn build_encrypted_message(&self, recipient: &Pubkey, body: &str) -> Result<Vec<u8>> {
        let nonce = rand_nonce();
        let enc_pk = curve25519_dalek::ristretto::CompressedRistretto(recipient.0);
        let encrypted = samp::encrypt(body.as_bytes(), &enc_pk, &nonce, &self.seed)
            .map_err(|e| SdkError::Encryption(e.to_string()))?;
        let vt = samp::compute_view_tag(&self.seed, &enc_pk, &nonce)
            .map_err(|e| SdkError::Encryption(e.to_string()))?;
        Ok(samp::encode_encrypted(
            samp::CONTENT_TYPE_ENCRYPTED,
            vt,
            &nonce,
            &encrypted,
        ))
    }

    pub fn build_thread_root(&self, recipient: &Pubkey, body: &str) -> Result<Vec<u8>> {
        let nonce = rand_nonce();
        let plaintext = samp::encode_thread_content(
            BlockRef::ZERO,
            BlockRef::ZERO,
            BlockRef::ZERO,
            body.as_bytes(),
        );
        let enc_pk = curve25519_dalek::ristretto::CompressedRistretto(recipient.0);
        let encrypted = samp::encrypt(&plaintext, &enc_pk, &nonce, &self.seed)
            .map_err(|e| SdkError::Encryption(e.to_string()))?;
        let vt = samp::compute_view_tag(&self.seed, &enc_pk, &nonce)
            .map_err(|e| SdkError::Encryption(e.to_string()))?;
        Ok(samp::encode_encrypted(
            samp::CONTENT_TYPE_THREAD,
            vt,
            &nonce,
            &encrypted,
        ))
    }

    pub fn build_thread_reply(&self, thread_idx: usize, body: &str) -> Result<Vec<u8>> {
        let thread = self
            .threads
            .get(thread_idx)
            .ok_or_else(|| SdkError::NotFound("Thread not found".into()))?;
        let nonce = rand_nonce();
        let plaintext = samp::encode_thread_content(
            thread.thread_ref,
            thread.last_ref(),
            thread.my_last_ref(),
            body.as_bytes(),
        );
        let enc_pk = curve25519_dalek::ristretto::CompressedRistretto(thread.peer_pubkey.0);
        let encrypted = samp::encrypt(&plaintext, &enc_pk, &nonce, &self.seed)
            .map_err(|e| SdkError::Encryption(e.to_string()))?;
        let vt = samp::compute_view_tag(&self.seed, &enc_pk, &nonce)
            .map_err(|e| SdkError::Encryption(e.to_string()))?;
        Ok(samp::encode_encrypted(
            samp::CONTENT_TYPE_THREAD,
            vt,
            &nonce,
            &encrypted,
        ))
    }

    pub fn build_channel_create(&self, name: &str, description: &str) -> Result<Vec<u8>> {
        samp::encode_channel_create(name, description).map_err(|e| SdkError::Other(e.to_string()))
    }

    pub fn build_channel_message(&self, channel_idx: usize, body: &str) -> Result<Vec<u8>> {
        let channel = self
            .channels
            .get(channel_idx)
            .ok_or_else(|| SdkError::NotFound("Channel not found".into()))?;
        Ok(samp::encode_channel_msg(
            channel.channel_ref,
            channel.last_ref(),
            channel.my_last_ref(),
            body.as_bytes(),
        ))
    }

    pub fn build_group_create(&self, members: &[Pubkey], body: &str) -> Result<Vec<u8>> {
        let nonce = rand_nonce();
        let raw_members: Vec<[u8; 32]> = members.iter().map(|pk| pk.0).collect();
        let mut body_bytes = samp::encode_group_members(&raw_members);
        body_bytes.extend_from_slice(body.as_bytes());
        let plaintext = samp::encode_thread_content(
            BlockRef::ZERO,
            BlockRef::ZERO,
            BlockRef::ZERO,
            &body_bytes,
        );
        let (eph_pubkey, capsules, ciphertext) =
            samp::encrypt_for_group(&plaintext, &raw_members, &nonce, &self.seed)
                .map_err(|e| SdkError::Encryption(e.to_string()))?;
        Ok(samp::encode_group(
            &nonce,
            &eph_pubkey,
            &capsules,
            &ciphertext,
        ))
    }

    pub fn build_group_message(&self, group_idx: usize, body: &str) -> Result<Vec<u8>> {
        let group = self
            .groups
            .get(group_idx)
            .ok_or_else(|| SdkError::NotFound("Group not found".into()))?;
        let nonce = rand_nonce();
        let plaintext = samp::encode_thread_content(
            group.group_ref,
            group.last_ref(),
            group.my_last_ref(),
            body.as_bytes(),
        );
        let raw_members: Vec<[u8; 32]> = group.members.iter().map(|pk| pk.0).collect();
        let (eph_pubkey, capsules, ciphertext) =
            samp::encrypt_for_group(&plaintext, &raw_members, &nonce, &self.seed)
                .map_err(|e| SdkError::Encryption(e.to_string()))?;
        Ok(samp::encode_group(
            &nonce,
            &eph_pubkey,
            &capsules,
            &ciphertext,
        ))
    }

    pub async fn submit(&self, remark: &[u8]) -> Result<String> {
        crate::extrinsic::submit_remark(
            &self.node_url,
            remark,
            &self.keypair,
            &self.my_ss58,
            &self.chain_info,
        )
        .await
        .map_err(SdkError::Chain)
    }

    pub async fn fetch_balance(&self) -> Result<u128> {
        let pk = self.pubkey();
        crate::extrinsic::fetch_balance(&self.node_url, &pk, &self.chain_info.account_info_layout)
            .await
            .map_err(SdkError::Chain)
    }

    pub async fn estimate_fee(&self, remark: &[u8]) -> Result<u128> {
        crate::extrinsic::estimate_fee(
            &self.node_url,
            remark,
            &self.keypair,
            &self.my_ss58,
            &self.chain_info,
        )
        .await
        .map_err(SdkError::Chain)
    }

    // -----------------------------------------------------------------------

    pub fn cleanup_pending(&mut self) -> Option<CleanupResult> {
        let mut removed_thread = None;
        let mut removed_channel = None;
        let mut removed_group = None;

        if let Some(&idx) = self.thread_index.get(&BlockRef::ZERO) {
            self.threads.remove(idx);
            self.thread_index.remove(&BlockRef::ZERO);
            reindex(&mut self.thread_index, idx);
            removed_thread = Some(idx);
        }
        if let Some(&idx) = self.channel_index.get(&BlockRef::ZERO) {
            self.channels.remove(idx);
            self.channel_index.remove(&BlockRef::ZERO);
            reindex(&mut self.channel_index, idx);
            removed_channel = Some(idx);
        }
        if let Some(&idx) = self.group_index.get(&BlockRef::ZERO) {
            self.groups.remove(idx);
            self.group_index.remove(&BlockRef::ZERO);
            reindex(&mut self.group_index, idx);
            removed_group = Some(idx);
        }

        if removed_thread.is_some() || removed_channel.is_some() || removed_group.is_some() {
            Some(CleanupResult {
                removed_thread,
                removed_channel,
                removed_group,
            })
        } else {
            None
        }
    }
}

pub struct CleanupResult {
    pub removed_thread: Option<usize>,
    pub removed_channel: Option<usize>,
    pub removed_group: Option<usize>,
}
