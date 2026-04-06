use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use chrono::{TimeZone, Utc};
use hkdf::Hkdf;
use rusqlite::{Connection, params};
use sha2::Sha256;
use std::path::PathBuf;

use crate::conversation::{InboxMessage, ThreadMessage};
use crate::types::{BlockRef, Pubkey};

type ThreadRow = (BlockRef, String, ThreadMessage, Vec<u8>);
type GroupRow = (BlockRef, Pubkey, Vec<Pubkey>);
type ChannelRow = (BlockRef, String, String, String, Vec<ThreadMessage>);
type ChannelMeta = (BlockRef, String, String, String);

const DB_KEY_INFO: &[u8] = b"taolk-db-v1";

fn ts(secs: i64) -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(secs, 0).single().unwrap_or_default()
}

pub struct Db {
    conn: Connection,
    cipher: ChaCha20Poly1305,
}

impl Db {
    #[allow(dead_code)]
    pub fn open_in_memory(seed: &[u8; 32]) -> Result<Self, Box<dyn std::error::Error>> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn, seed)
    }

    pub fn open(wallet_name: &str, seed: &[u8; 32]) -> Result<Self, Box<dyn std::error::Error>> {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("taolk")
            .join(wallet_name);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("messages.db");
        let conn = Connection::open(path)?;
        Self::init(conn, seed)
    }

    fn init(conn: Connection, seed: &[u8; 32]) -> Result<Self, Box<dyn std::error::Error>> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS inbox (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                peer_ss58 TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                body BLOB NOT NULL,
                content_type INTEGER NOT NULL,
                is_mine INTEGER NOT NULL,
                block_number INTEGER NOT NULL DEFAULT 0,
                ext_index INTEGER NOT NULL DEFAULT 0,
                UNIQUE(block_number, ext_index)
            );
            CREATE TABLE IF NOT EXISTS thread_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                thread_block INTEGER NOT NULL,
                thread_index INTEGER NOT NULL,
                peer_ss58 TEXT NOT NULL,
                sender_ss58 TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                body BLOB NOT NULL,
                is_mine INTEGER NOT NULL,
                reply_to_block INTEGER NOT NULL,
                reply_to_index INTEGER NOT NULL,
                continues_block INTEGER NOT NULL,
                continues_index INTEGER NOT NULL,
                block_number INTEGER NOT NULL,
                ext_index INTEGER NOT NULL,
                UNIQUE(block_number, ext_index)
            );
            CREATE TABLE IF NOT EXISTS peers (
                ss58_short TEXT PRIMARY KEY,
                pubkey BLOB NOT NULL
            );
            CREATE TABLE IF NOT EXISTS known_channels (
                channel_block INTEGER NOT NULL,
                channel_index INTEGER NOT NULL,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                creator_ss58 TEXT NOT NULL,
                PRIMARY KEY (channel_block, channel_index)
            );
            CREATE TABLE IF NOT EXISTS channels (
                channel_block INTEGER NOT NULL,
                channel_index INTEGER NOT NULL,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                creator_ss58 TEXT NOT NULL,
                PRIMARY KEY (channel_block, channel_index)
            );
            CREATE TABLE IF NOT EXISTS groups (
                group_block INTEGER NOT NULL,
                group_index INTEGER NOT NULL,
                creator_pubkey BLOB NOT NULL,
                members BLOB NOT NULL,
                PRIMARY KEY (group_block, group_index)
            );
            CREATE TABLE IF NOT EXISTS group_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                group_block INTEGER NOT NULL,
                group_index INTEGER NOT NULL,
                sender_ss58 TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                body BLOB NOT NULL,
                is_mine INTEGER NOT NULL,
                reply_to_block INTEGER NOT NULL,
                reply_to_index INTEGER NOT NULL,
                continues_block INTEGER NOT NULL,
                continues_index INTEGER NOT NULL,
                block_number INTEGER NOT NULL,
                ext_index INTEGER NOT NULL,
                UNIQUE(block_number, ext_index)
            );
            CREATE TABLE IF NOT EXISTS channel_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_block INTEGER NOT NULL,
                channel_index INTEGER NOT NULL,
                sender_ss58 TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                body BLOB NOT NULL,
                is_mine INTEGER NOT NULL,
                reply_to_block INTEGER NOT NULL,
                reply_to_index INTEGER NOT NULL,
                continues_block INTEGER NOT NULL,
                continues_index INTEGER NOT NULL,
                block_number INTEGER NOT NULL,
                ext_index INTEGER NOT NULL,
                UNIQUE(block_number, ext_index)
            );",
        )?;

        let mut db_key = derive_db_key(seed);
        let cipher = ChaCha20Poly1305::new((&db_key).into());
        zeroize::Zeroize::zeroize(&mut db_key);

        Ok(Self { conn, cipher })
    }

    fn encrypt_body(&self, plaintext: &str, block: u32, index: u16) -> Vec<u8> {
        let nonce = body_nonce(block, index);
        self.cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
            .unwrap_or_default()
    }

    fn decrypt_body(&self, ciphertext: &[u8], block: u32, index: u16) -> String {
        let nonce = body_nonce(block, index);
        self.cipher
            .decrypt(Nonce::from_slice(&nonce), ciphertext)
            .ok()
            .and_then(|pt| String::from_utf8(pt).ok())
            .unwrap_or_default()
    }

    fn encrypt_inbox_body(&self, plaintext: &str, id_hint: i64) -> Vec<u8> {
        let nonce = inbox_nonce(id_hint);
        self.cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
            .unwrap_or_default()
    }

    fn decrypt_inbox_body(&self, ciphertext: &[u8], id: i64) -> String {
        let nonce = inbox_nonce(id);
        self.cipher
            .decrypt(Nonce::from_slice(&nonce), ciphertext)
            .ok()
            .and_then(|pt| String::from_utf8(pt).ok())
            .unwrap_or_default()
    }

    pub fn insert_inbox(&self, msg: &InboxMessage) {
        let next_id: i64 = self
            .conn
            .query_row("SELECT COALESCE(MAX(id), 0) + 1 FROM inbox", [], |r| {
                r.get(0)
            })
            .unwrap_or(1);
        let encrypted = self.encrypt_inbox_body(&msg.body, next_id);
        let _ = self.conn.execute(
            "INSERT OR IGNORE INTO inbox (peer_ss58, timestamp, body, content_type, is_mine, block_number, ext_index)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![msg.peer_ss58, msg.timestamp.timestamp(), encrypted, msg.content_type, msg.is_mine as i32, msg.block_number, msg.ext_index],
        );
    }

    pub fn insert_thread_message(
        &self,
        thread_ref: BlockRef,
        peer_ss58: &str,
        msg: &ThreadMessage,
        block_number: u32,
        ext_index: u16,
    ) {
        let encrypted = self.encrypt_body(&msg.body, block_number, ext_index);
        let _ = self.conn.execute(
            "INSERT OR IGNORE INTO thread_messages
             (thread_block, thread_index, peer_ss58, sender_ss58, timestamp, body, is_mine,
              reply_to_block, reply_to_index, continues_block, continues_index, block_number, ext_index)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![thread_ref.block, thread_ref.index, peer_ss58, msg.sender_ss58, msg.timestamp.timestamp(),
                    encrypted, msg.is_mine as i32, msg.reply_to.block, msg.reply_to.index,
                    msg.continues.block, msg.continues.index, block_number, ext_index],
        );
    }

    pub fn upsert_peer(&self, ss58_short: &str, pubkey: &Pubkey) {
        let _ = self.conn.execute(
            "INSERT OR REPLACE INTO peers (ss58_short, pubkey) VALUES (?1, ?2)",
            params![ss58_short, &pubkey.0[..]],
        );
    }

    pub fn get_peer_pubkey(&self, ss58_short: &str) -> Option<Pubkey> {
        self.conn
            .query_row(
                "SELECT pubkey FROM peers WHERE ss58_short = ?1",
                params![ss58_short],
                |row| {
                    let blob: Vec<u8> = row.get(0)?;
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&blob);
                    Ok(Pubkey(key))
                },
            )
            .ok()
    }

    pub fn has_message_at(&self, block_ref: BlockRef) -> bool {
        self.conn
            .query_row(
                "SELECT 1 FROM thread_messages WHERE block_number = ?1 AND ext_index = ?2",
                params![block_ref.block, block_ref.index],
                |_| Ok(()),
            )
            .is_ok()
    }

    pub fn load_inbox(&self) -> (Vec<InboxMessage>, Vec<InboxMessage>) {
        let inner = || -> Option<Vec<InboxMessage>> {
            let mut stmt = self.conn.prepare(
                "SELECT id, peer_ss58, timestamp, body, content_type, is_mine, block_number, ext_index FROM inbox ORDER BY timestamp"
            ).ok()?;
            let all = stmt
                .query_map([], |row| {
                    let id: i64 = row.get(0)?;
                    let t: i64 = row.get(2)?;
                    let ct: Vec<u8> = row.get(3)?;
                    Ok((
                        id,
                        InboxMessage {
                            peer_ss58: row.get(1)?,
                            timestamp: ts(t),
                            body: String::new(),
                            content_type: row.get(4)?,
                            is_mine: row.get::<_, i32>(5)? != 0,
                            block_number: row.get::<_, u32>(6).unwrap_or(0),
                            ext_index: row.get::<_, u16>(7).unwrap_or(0),
                        },
                        ct,
                    ))
                })
                .ok()?
                .filter_map(|r| r.ok())
                .map(|(id, mut msg, ct)| {
                    msg.body = self.decrypt_inbox_body(&ct, id);
                    msg
                })
                .collect();
            Some(all)
        };
        let all = inner().unwrap_or_default();
        let (outbox, inbox): (Vec<_>, Vec<_>) = all.into_iter().partition(|m| m.is_mine);
        (inbox, outbox)
    }

    pub fn load_threads(&self) -> Vec<(BlockRef, String, Vec<ThreadMessage>)> {
        let inner = || -> Option<Vec<ThreadRow>> {
            let mut stmt = self.conn.prepare(
                "SELECT thread_block, thread_index, peer_ss58, sender_ss58, timestamp, body, is_mine,
                        reply_to_block, reply_to_index, continues_block, continues_index, block_number, ext_index
                 FROM thread_messages ORDER BY block_number, ext_index"
            ).ok()?;
            let rows = stmt
                .query_map([], |row| {
                    let t: i64 = row.get(4)?;
                    let ct: Vec<u8> = row.get(5)?;
                    let bn: u32 = row.get(11)?;
                    let ei: u16 = row.get(12)?;
                    Ok((
                        BlockRef {
                            block: row.get::<_, u32>(0)?,
                            index: row.get::<_, u16>(1)?,
                        },
                        row.get::<_, String>(2)?,
                        ThreadMessage {
                            sender_ss58: row.get(3)?,
                            timestamp: ts(t),
                            body: String::new(),
                            is_mine: row.get::<_, i32>(6)? != 0,
                            reply_to: BlockRef {
                                block: row.get(7)?,
                                index: row.get(8)?,
                            },
                            continues: BlockRef {
                                block: row.get(9)?,
                                index: row.get(10)?,
                            },
                            block_number: bn,
                            ext_index: ei,
                            has_gap: false,
                        },
                        ct,
                    ))
                })
                .ok()?
                .filter_map(|r| r.ok())
                .collect();
            Some(rows)
        };
        let rows = inner().unwrap_or_default();
        let mut groups: Vec<(BlockRef, String, Vec<ThreadMessage>)> = Vec::new();
        let mut index: std::collections::HashMap<BlockRef, usize> =
            std::collections::HashMap::new();
        for (thread_ref, peer_ss58, mut msg, ct) in rows {
            msg.body = self.decrypt_body(&ct, msg.block_number, msg.ext_index);
            if let Some(&i) = index.get(&thread_ref) {
                groups[i].2.push(msg);
            } else {
                let i = groups.len();
                index.insert(thread_ref, i);
                groups.push((thread_ref, peer_ss58, vec![msg]));
            }
        }
        groups
    }

    pub fn insert_channel(
        &self,
        channel_ref: BlockRef,
        name: &str,
        description: &str,
        creator_ss58: &str,
    ) {
        let _ = self.conn.execute(
            "INSERT OR IGNORE INTO channels (channel_block, channel_index, name, description, creator_ss58) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![channel_ref.block, channel_ref.index, name, description, creator_ss58],
        );
    }

    pub fn update_channel_meta(
        &self,
        channel_ref: BlockRef,
        name: &str,
        description: &str,
        creator_ss58: &str,
    ) {
        let _ = self.conn.execute(
            "UPDATE channels SET name = ?3, description = ?4, creator_ss58 = ?5 WHERE channel_block = ?1 AND channel_index = ?2",
            params![channel_ref.block, channel_ref.index, name, description, creator_ss58],
        );
    }

    pub fn insert_known_channel(
        &self,
        channel_ref: BlockRef,
        name: &str,
        description: &str,
        creator_ss58: &str,
    ) {
        let _ = self.conn.execute(
            "INSERT OR IGNORE INTO known_channels (channel_block, channel_index, name, description, creator_ss58) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![channel_ref.block, channel_ref.index, name, description, creator_ss58],
        );
    }

    pub fn update_known_channel_meta(
        &self,
        channel_ref: BlockRef,
        name: &str,
        description: &str,
        creator_ss58: &str,
    ) {
        let _ = self.conn.execute(
            "UPDATE known_channels SET name = ?3, description = ?4, creator_ss58 = ?5 WHERE channel_block = ?1 AND channel_index = ?2",
            params![channel_ref.block, channel_ref.index, name, description, creator_ss58],
        );
    }

    pub fn load_known_channels(&self) -> Vec<(BlockRef, String, String, String)> {
        let inner = || -> Option<Vec<_>> {
            let mut stmt = self.conn.prepare("SELECT channel_block, channel_index, name, description, creator_ss58 FROM known_channels").ok()?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        BlockRef {
                            block: row.get::<_, u32>(0)?,
                            index: row.get::<_, u16>(1)?,
                        },
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })
                .ok()?
                .filter_map(|r| r.ok())
                .collect();
            Some(rows)
        };
        inner().unwrap_or_default()
    }

    pub fn insert_group(&self, group_ref: BlockRef, creator_pubkey: &Pubkey, members: &[Pubkey]) {
        let nonce = group_nonce(group_ref.block, group_ref.index);
        let members_raw: Vec<u8> = members.iter().flat_map(|pk| pk.0.iter().copied()).collect();
        let encrypted_members = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), members_raw.as_slice())
            .unwrap_or_default();
        let _ = self.conn.execute(
            "INSERT OR IGNORE INTO groups (group_block, group_index, creator_pubkey, members) VALUES (?1, ?2, ?3, ?4)",
            params![group_ref.block, group_ref.index, &creator_pubkey.0[..], encrypted_members],
        );
    }

    pub fn load_groups(&self) -> Vec<GroupRow> {
        let inner = || -> Option<Vec<_>> {
            let mut stmt = self
                .conn
                .prepare("SELECT group_block, group_index, creator_pubkey, members FROM groups")
                .ok()?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, u32>(0)?,
                        row.get::<_, u16>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                    ))
                })
                .ok()?
                .filter_map(|r| r.ok())
                .filter_map(|(block, index, cpk, enc)| {
                    if cpk.len() != 32 {
                        return None;
                    }
                    let mut creator_bytes = [0u8; 32];
                    creator_bytes.copy_from_slice(&cpk);
                    let nonce = group_nonce(block, index);
                    let dec = self
                        .cipher
                        .decrypt(Nonce::from_slice(&nonce), enc.as_slice())
                        .ok()?;
                    if dec.len() % 32 != 0 {
                        return None;
                    }
                    let members = dec
                        .chunks_exact(32)
                        .map(|c| {
                            let mut pk = [0u8; 32];
                            pk.copy_from_slice(c);
                            Pubkey(pk)
                        })
                        .collect();
                    Some((BlockRef { block, index }, Pubkey(creator_bytes), members))
                })
                .collect();
            Some(rows)
        };
        inner().unwrap_or_default()
    }

    pub fn insert_group_message(
        &self,
        group_ref: BlockRef,
        msg: &ThreadMessage,
        block_number: u32,
        ext_index: u16,
    ) {
        let encrypted = self.encrypt_body(&msg.body, block_number, ext_index);
        let _ = self.conn.execute(
            "INSERT OR IGNORE INTO group_messages
             (group_block, group_index, sender_ss58, timestamp, body, is_mine,
              reply_to_block, reply_to_index, continues_block, continues_index, block_number, ext_index)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![group_ref.block, group_ref.index, msg.sender_ss58, msg.timestamp.timestamp(),
                    encrypted, msg.is_mine as i32, msg.reply_to.block, msg.reply_to.index,
                    msg.continues.block, msg.continues.index, block_number, ext_index],
        );
    }

    pub fn has_group_message_at(&self, block_ref: BlockRef) -> bool {
        self.conn
            .query_row(
                "SELECT 1 FROM group_messages WHERE block_number = ?1 AND ext_index = ?2",
                params![block_ref.block, block_ref.index],
                |_| Ok(()),
            )
            .is_ok()
    }

    pub fn load_group_messages(&self, group_ref: BlockRef) -> Vec<ThreadMessage> {
        let inner = || -> Option<Vec<ThreadMessage>> {
            let mut stmt = self
                .conn
                .prepare(
                    "SELECT sender_ss58, timestamp, body, is_mine,
                        reply_to_block, reply_to_index, continues_block, continues_index,
                        block_number, ext_index
                 FROM group_messages WHERE group_block = ?1 AND group_index = ?2
                 ORDER BY block_number, ext_index",
                )
                .ok()?;
            let rows = stmt
                .query_map(params![group_ref.block, group_ref.index], |row| {
                    let t: i64 = row.get(1)?;
                    let ct: Vec<u8> = row.get(2)?;
                    let bn: u32 = row.get(8)?;
                    let ei: u16 = row.get(9)?;
                    Ok((
                        ThreadMessage {
                            sender_ss58: row.get(0)?,
                            timestamp: ts(t),
                            body: String::new(),
                            is_mine: row.get::<_, i32>(3)? != 0,
                            reply_to: BlockRef {
                                block: row.get(4)?,
                                index: row.get(5)?,
                            },
                            continues: BlockRef {
                                block: row.get(6)?,
                                index: row.get(7)?,
                            },
                            block_number: bn,
                            ext_index: ei,
                            has_gap: false,
                        },
                        ct,
                    ))
                })
                .ok()?
                .filter_map(|r| r.ok())
                .map(|(mut msg, ct)| {
                    msg.body = self.decrypt_body(&ct, msg.block_number, msg.ext_index);
                    msg
                })
                .collect();
            Some(rows)
        };
        inner().unwrap_or_default()
    }

    pub fn insert_channel_message(
        &self,
        channel_ref: BlockRef,
        msg: &ThreadMessage,
        block_number: u32,
        ext_index: u16,
    ) {
        let encrypted = self.encrypt_body(&msg.body, block_number, ext_index);
        let _ = self.conn.execute(
            "INSERT OR IGNORE INTO channel_messages
             (channel_block, channel_index, sender_ss58, timestamp, body, is_mine,
              reply_to_block, reply_to_index, continues_block, continues_index, block_number, ext_index)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![channel_ref.block, channel_ref.index, msg.sender_ss58, msg.timestamp.timestamp(),
                    encrypted, msg.is_mine as i32, msg.reply_to.block, msg.reply_to.index,
                    msg.continues.block, msg.continues.index, block_number, ext_index],
        );
    }

    pub fn has_channel_message_at(&self, block_ref: BlockRef) -> bool {
        self.conn
            .query_row(
                "SELECT 1 FROM channel_messages WHERE block_number = ?1 AND ext_index = ?2",
                params![block_ref.block, block_ref.index],
                |_| Ok(()),
            )
            .is_ok()
    }

    pub fn delete_channel(&self, channel_ref: BlockRef) {
        let _ = self.conn.execute(
            "DELETE FROM channel_messages WHERE channel_block = ?1 AND channel_index = ?2",
            params![channel_ref.block, channel_ref.index],
        );
        let _ = self.conn.execute(
            "DELETE FROM channels WHERE channel_block = ?1 AND channel_index = ?2",
            params![channel_ref.block, channel_ref.index],
        );
    }

    pub fn load_channels(&self) -> Vec<ChannelRow> {
        let inner = || -> Option<Vec<ChannelMeta>> {
            let mut stmt = self.conn.prepare("SELECT channel_block, channel_index, name, description, creator_ss58 FROM channels").ok()?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        BlockRef {
                            block: row.get::<_, u32>(0)?,
                            index: row.get::<_, u16>(1)?,
                        },
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })
                .ok()?
                .filter_map(|r| r.ok())
                .collect();
            Some(rows)
        };
        let channels = inner().unwrap_or_default();

        let msg_inner = || -> Option<std::collections::HashMap<BlockRef, Vec<ThreadMessage>>> {
            let mut stmt = self.conn.prepare(
                "SELECT channel_block, channel_index, sender_ss58, timestamp, body, is_mine,
                        reply_to_block, reply_to_index, continues_block, continues_index, block_number, ext_index
                 FROM channel_messages ORDER BY block_number, ext_index"
            ).ok()?;
            let mut map: std::collections::HashMap<BlockRef, Vec<ThreadMessage>> =
                std::collections::HashMap::new();
            for row in stmt
                .query_map([], |row| {
                    let ch = BlockRef {
                        block: row.get(0)?,
                        index: row.get(1)?,
                    };
                    let t: i64 = row.get(3)?;
                    let ct: Vec<u8> = row.get(4)?;
                    let bn: u32 = row.get(10)?;
                    let ei: u16 = row.get(11)?;
                    Ok((
                        ch,
                        ThreadMessage {
                            sender_ss58: row.get(2)?,
                            timestamp: ts(t),
                            body: String::new(),
                            is_mine: row.get::<_, i32>(5)? != 0,
                            reply_to: BlockRef {
                                block: row.get(6)?,
                                index: row.get(7)?,
                            },
                            continues: BlockRef {
                                block: row.get(8)?,
                                index: row.get(9)?,
                            },
                            block_number: bn,
                            ext_index: ei,
                            has_gap: false,
                        },
                        ct,
                    ))
                })
                .ok()?
                .filter_map(|r| r.ok())
            {
                let (ch, mut msg, ct) = row;
                msg.body = self.decrypt_body(&ct, msg.block_number, msg.ext_index);
                map.entry(ch).or_default().push(msg);
            }
            Some(map)
        };
        let mut msg_map = msg_inner().unwrap_or_default();

        channels
            .into_iter()
            .map(|(ch_ref, name, desc, creator)| {
                let msgs = msg_map.remove(&ch_ref).unwrap_or_default();
                (ch_ref, name, desc, creator, msgs)
            })
            .collect()
    }
}

fn derive_db_key(seed: &[u8; 32]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, seed);
    let mut key = [0u8; 32];
    let _ = hk.expand(DB_KEY_INFO, &mut key);
    key
}

fn body_nonce(block: u32, index: u16) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[..4].copy_from_slice(&block.to_le_bytes());
    nonce[4..6].copy_from_slice(&index.to_le_bytes());
    nonce
}

fn group_nonce(block: u32, index: u16) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[..4].copy_from_slice(&block.to_le_bytes());
    nonce[4..6].copy_from_slice(&index.to_le_bytes());
    nonce[6] = 0xAA;
    nonce
}

fn inbox_nonce(id: i64) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[..8].copy_from_slice(&id.to_le_bytes());
    nonce[8] = 0xFF;
    nonce
}
