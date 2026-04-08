use std::cell::RefCell;
use std::time::Instant;
use taolk::audio::Audio;
use taolk::event::ConnState;
use taolk::session::Session;
use taolk::types::{BlockRef, Pubkey};

#[derive(PartialEq, Clone, Copy)]
pub enum Mode {
    Normal,
    Insert,
    Confirm,
    Compose,
    Message,
    CreateChannel,
    CreateChannelDesc,
    CreateGroupMembers,
    Search,
    SenderPicker,
    Help,
}

#[derive(PartialEq, Clone, Copy)]
pub enum View {
    Inbox,
    Outbox,
    Thread(usize),
    Channel(usize),
    Group(usize),
    ChannelDir,
}

pub struct App {
    pub session: Session,
    pub running: bool,
    pub mode: Mode,
    pub view: View,
    pub show_sidebar: bool,
    pub input: String,
    pub cursor_pos: usize,
    pub channel_dir_cursor: usize,
    pub channel_dir_input: String,
    pub status_message: Option<(String, Instant, u64, bool)>,
    pub pending_remark: Option<Vec<u8>>,
    pub pending_text: Option<String>,
    pub pending_fee: Option<String>,
    pub pending_view: Option<View>,
    pub frame: u32,
    pub sending: bool,
    pub msg_recipient: Option<(Pubkey, String)>,
    pub msg_type: Option<u8>,
    pub scroll_offset: usize,
    pub quit_confirm: bool,
    pub search_query: String,
    pub contact_idx: usize,
    pub pending_channel_name: Option<String>,
    pub pending_channel_desc: Option<String>,
    pub pending_group_members: Vec<(Pubkey, String)>,
    pub last_fee: Option<u128>,
    pub block_changed_at: u32,
    pub balance_changed_at: u32,
    pub balance_decreased: bool,
    pub sidebar_width: u16,
    pub timestamp_format: String,
    pub date_format: String,
    pub audio: Audio,
    pub sound_armed: bool,
    pub picker_senders: Vec<(String, Option<Pubkey>)>,
    pub sender_click_regions: RefCell<Vec<(u16, u16, u16, String)>>,
    pub connection: ConnState,
}

impl App {
    pub fn new(session: Session, audio: Audio) -> Self {
        Self {
            session,
            running: true,
            mode: Mode::Normal,
            view: View::Inbox,
            show_sidebar: true,
            input: String::new(),
            cursor_pos: 0,
            channel_dir_cursor: 0,
            channel_dir_input: String::new(),
            status_message: None,
            pending_remark: None,
            pending_text: None,
            pending_fee: None,
            pending_view: None,
            frame: 0,
            sending: false,
            msg_recipient: None,
            msg_type: None,
            scroll_offset: 0,
            quit_confirm: false,
            search_query: String::new(),
            contact_idx: 0,
            pending_channel_name: None,
            pending_channel_desc: None,
            pending_group_members: Vec::new(),
            last_fee: None,
            block_changed_at: 0,
            balance_changed_at: 0,
            balance_decreased: false,
            sidebar_width: 28,
            timestamp_format: "%H:%M".into(),
            date_format: "%Y-%m-%d %H:%M".into(),
            audio,
            sound_armed: false,
            picker_senders: Vec::new(),
            sender_click_regions: RefCell::new(Vec::new()),
            connection: ConnState::Connected,
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), Instant::now(), 5, false));
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), Instant::now(), 30, true));
    }

    pub fn set_chain_error(&mut self, raw: &str) {
        let translated = self.session.chain_info.errors.humanize_rpc_error(raw);
        self.set_error(translated);
    }

    pub fn reset_input(&mut self) {
        self.input.clear();
        self.cursor_pos = 0;
    }

    pub fn enter_mode(&mut self, mode: Mode) {
        self.reset_input();
        self.mode = mode;
    }

    pub fn check_not_sending(&mut self) -> bool {
        if self.sending {
            self.set_error("Still sending previous message");
            return false;
        }
        true
    }

    pub fn current_status(&self) -> Option<(&str, bool)> {
        self.status_message
            .as_ref()
            .and_then(|(msg, when, secs, is_err)| {
                if when.elapsed() < std::time::Duration::from_secs(*secs) {
                    Some((msg.as_str(), *is_err))
                } else {
                    None
                }
            })
    }

    pub fn spinner_1(&self) -> char {
        const FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        FRAMES[usize::try_from(self.frame).unwrap_or(0) % FRAMES.len()]
    }

    pub fn spinner_5(&self) -> &'static str {
        const POS: &[&str] = &["⠿⠒⠒⠒⠒", "⠒⠿⠒⠒⠒", "⠒⠒⠿⠒⠒", "⠒⠒⠒⠿⠒", "⠒⠒⠒⠒⠿"];
        const EASE: &[usize] = &[0, 0, 1, 2, 3, 4, 4, 3, 2, 1];
        POS[EASE[usize::try_from(self.frame).unwrap_or(0) % EASE.len()]]
    }

    pub fn spinner_16(&self) -> &'static str {
        const POS: &[&str] = &[
            "⠿⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒",
            "⠒⠿⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒",
            "⠒⠒⠿⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒",
            "⠒⠒⠒⠿⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒",
            "⠒⠒⠒⠒⠿⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒",
            "⠒⠒⠒⠒⠒⠿⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒",
            "⠒⠒⠒⠒⠒⠒⠿⠒⠒⠒⠒⠒⠒⠒⠒⠒",
            "⠒⠒⠒⠒⠒⠒⠒⠿⠒⠒⠒⠒⠒⠒⠒⠒",
            "⠒⠒⠒⠒⠒⠒⠒⠒⠿⠒⠒⠒⠒⠒⠒⠒",
            "⠒⠒⠒⠒⠒⠒⠒⠒⠒⠿⠒⠒⠒⠒⠒⠒",
            "⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠿⠒⠒⠒⠒⠒",
            "⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠿⠒⠒⠒⠒",
            "⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠿⠒⠒⠒",
            "⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠿⠒⠒",
            "⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠿⠒",
            "⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠒⠿",
        ];
        const EASE: &[usize] = &[
            0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 15, 15, 14, 13, 12, 11, 10,
            9, 8, 7, 6, 5, 4, 3, 2, 1,
        ];
        POS[EASE[usize::try_from(self.frame).unwrap_or(0) % EASE.len()]]
    }

    pub fn is_busy(&self) -> bool {
        matches!(self.current_status(), Some((s, _)) if s.ends_with("..."))
    }

    pub fn save_draft(&mut self) {
        let key: Option<(u8, BlockRef)> = match self.view {
            View::Thread(i) => self.session.threads.get(i).map(|t| (0u8, t.thread_ref)),
            View::Channel(i) => self.session.channels.get(i).map(|c| (1u8, c.channel_ref)),
            View::Group(i) => self.session.groups.get(i).map(|g| (2u8, g.group_ref)),
            _ => None,
        };
        match self.view {
            View::Thread(i) => {
                if let Some(thread) = self.session.threads.get_mut(i) {
                    thread.draft = self.input.clone();
                }
            }
            View::Channel(i) => {
                if let Some(ch) = self.session.channels.get_mut(i) {
                    ch.draft = self.input.clone();
                }
            }
            View::Group(i) => {
                if let Some(g) = self.session.groups.get_mut(i) {
                    g.draft = self.input.clone();
                }
            }
            _ => {}
        }
        if let Some((kind, bref)) = key {
            self.session
                .db
                .save_draft(kind, bref.block, bref.index, &self.input);
        }
    }

    pub fn load_draft(&mut self) {
        let draft = match self.view {
            View::Thread(i) => self.session.threads.get(i).map(|c| c.draft.clone()),
            View::Channel(i) => self.session.channels.get(i).map(|c| c.draft.clone()),
            View::Group(i) => self.session.groups.get(i).map(|g| g.draft.clone()),
            _ => None,
        };
        self.input = draft.unwrap_or_default();
        self.cursor_pos = self.input.len();
    }

    pub fn current_draft(&self) -> Option<&str> {
        let draft = match self.view {
            View::Thread(i) => self.session.threads.get(i).map(|c| c.draft.as_str()),
            View::Channel(i) => self.session.channels.get(i).map(|c| c.draft.as_str()),
            View::Group(i) => self.session.groups.get(i).map(|g| g.draft.as_str()),
            _ => None,
        };
        draft.filter(|d| !d.is_empty())
    }

    pub fn mark_read(&mut self) {
        match self.view {
            View::Thread(i) => {
                if let Some(thread) = self.session.threads.get_mut(i) {
                    thread.last_read = thread.messages.len();
                }
            }
            View::Channel(i) => {
                if let Some(ch) = self.session.channels.get_mut(i) {
                    ch.last_read = ch.messages.len();
                }
            }
            View::Group(i) => {
                if let Some(g) = self.session.groups.get_mut(i) {
                    g.last_read = g.messages.len();
                }
            }
            _ => {}
        }
    }

    pub fn clear_draft(&mut self) {
        let key: Option<(u8, BlockRef)> = match self.view {
            View::Thread(i) => self.session.threads.get(i).map(|t| (0u8, t.thread_ref)),
            View::Channel(i) => self.session.channels.get(i).map(|c| (1u8, c.channel_ref)),
            View::Group(i) => self.session.groups.get(i).map(|g| (2u8, g.group_ref)),
            _ => None,
        };
        match self.view {
            View::Thread(i) => {
                if let Some(thread) = self.session.threads.get_mut(i) {
                    thread.draft.clear();
                }
            }
            View::Channel(i) => {
                if let Some(ch) = self.session.channels.get_mut(i) {
                    ch.draft.clear();
                }
            }
            View::Group(i) => {
                if let Some(g) = self.session.groups.get_mut(i) {
                    g.draft.clear();
                }
            }
            _ => {}
        }
        if let Some((kind, bref)) = key {
            self.session.db.delete_draft(kind, bref.block, bref.index);
        }
    }

    pub fn filtered_contacts(&self) -> Vec<(String, Pubkey)> {
        let filter = self.input.to_lowercase();
        if filter.is_empty() {
            return self.session.known_contacts();
        }
        self.session
            .peer_pubkeys
            .iter()
            .filter(|(ss58, _)| ss58.to_lowercase().contains(&filter))
            .map(|(ss58, pk)| (ss58.clone(), *pk))
            .collect()
    }

    pub fn clear_standalone(&mut self) {
        self.msg_recipient = None;
        self.msg_type = None;
    }

    pub fn build_picker_senders(&self) -> Vec<(String, Option<Pubkey>)> {
        use std::collections::HashMap;
        let mut last_seen: HashMap<String, BlockRef> = HashMap::new();
        let mut record = |ss58: &str, br: BlockRef| {
            let cur = last_seen.get(ss58).copied().unwrap_or(BlockRef::ZERO);
            if br > cur {
                last_seen.insert(ss58.to_string(), br);
            }
        };
        match self.view {
            View::Inbox => {
                for m in &self.session.inbox {
                    if !m.is_mine {
                        record(
                            &m.peer_ss58,
                            BlockRef {
                                block: m.block_number,
                                index: m.ext_index,
                            },
                        );
                    }
                }
            }
            View::Outbox => {
                for m in &self.session.outbox {
                    record(
                        &m.peer_ss58,
                        BlockRef {
                            block: m.block_number,
                            index: m.ext_index,
                        },
                    );
                }
            }
            View::Thread(i) => {
                if let Some(t) = self.session.threads.get(i) {
                    for m in &t.messages {
                        if !m.is_mine {
                            record(
                                &m.sender_ss58,
                                BlockRef {
                                    block: m.block_number,
                                    index: m.ext_index,
                                },
                            );
                        }
                    }
                }
            }
            View::Channel(i) => {
                if let Some(c) = self.session.channels.get(i) {
                    for m in &c.messages {
                        if !m.is_mine {
                            record(
                                &m.sender_ss58,
                                BlockRef {
                                    block: m.block_number,
                                    index: m.ext_index,
                                },
                            );
                        }
                    }
                }
            }
            View::Group(i) => {
                if let Some(g) = self.session.groups.get(i) {
                    for m in &g.messages {
                        if !m.is_mine {
                            record(
                                &m.sender_ss58,
                                BlockRef {
                                    block: m.block_number,
                                    index: m.ext_index,
                                },
                            );
                        }
                    }
                }
            }
            View::ChannelDir => {}
        }
        let mut entries: Vec<(String, BlockRef)> = last_seen.into_iter().collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries
            .into_iter()
            .map(|(ss58, _)| {
                let pk = self.session.peer_pubkeys.get(&ss58).copied();
                (ss58, pk)
            })
            .collect()
    }

    pub fn is_pending_channel(&self) -> bool {
        self.pending_channel_name.is_some()
    }

    pub fn is_pending_group(&self) -> bool {
        !self.pending_group_members.is_empty()
    }

    pub fn clear_pending(&mut self) {
        self.pending_remark = None;
        self.pending_text = None;
        self.pending_fee = None;
        self.pending_view = None;
        self.pending_channel_name = None;
        self.pending_channel_desc = None;
        self.pending_group_members.clear();
    }

    pub fn sidebar_rows(&self) -> Vec<Option<View>> {
        let mut rows = Vec::new();
        rows.push(Some(View::Inbox));
        rows.push(Some(View::Outbox));

        if !self.session.threads.is_empty() {
            rows.push(None);
            rows.push(None);

            let mut peer_groups: Vec<(String, Vec<usize>)> = Vec::new();
            let mut peer_idx: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for (i, t) in self.session.threads.iter().enumerate() {
                if let Some(&gi) = peer_idx.get(&t.peer_ss58) {
                    peer_groups[gi].1.push(i);
                } else {
                    let gi = peer_groups.len();
                    peer_idx.insert(t.peer_ss58.clone(), gi);
                    peer_groups.push((t.peer_ss58.clone(), vec![i]));
                }
            }
            peer_groups.sort_by(|a, b| {
                let la =
                    a.1.iter()
                        .filter_map(|&i| self.session.threads[i].messages.last())
                        .map(|m| BlockRef {
                            block: m.block_number,
                            index: m.ext_index,
                        })
                        .max()
                        .unwrap_or(BlockRef::ZERO);
                let lb =
                    b.1.iter()
                        .filter_map(|&i| self.session.threads[i].messages.last())
                        .map(|m| BlockRef {
                            block: m.block_number,
                            index: m.ext_index,
                        })
                        .max()
                        .unwrap_or(BlockRef::ZERO);
                lb.cmp(&la)
            });
            for (_, idxs) in &mut peer_groups {
                idxs.sort_by(|&a, &b| {
                    let la = self.session.threads[a]
                        .messages
                        .last()
                        .map(|m| BlockRef {
                            block: m.block_number,
                            index: m.ext_index,
                        })
                        .unwrap_or(BlockRef::ZERO);
                    let lb = self.session.threads[b]
                        .messages
                        .last()
                        .map(|m| BlockRef {
                            block: m.block_number,
                            index: m.ext_index,
                        })
                        .unwrap_or(BlockRef::ZERO);
                    lb.cmp(&la)
                });
            }

            for (_peer, idxs) in &peer_groups {
                if idxs.len() == 1 {
                    let t = &self.session.threads[idxs[0]];
                    if t.thread_ref == BlockRef::ZERO {
                        rows.push(None);
                    } else {
                        rows.push(Some(View::Thread(idxs[0])));
                    }
                } else {
                    rows.push(None);
                    for &i in idxs {
                        let t = &self.session.threads[i];
                        if t.thread_ref == BlockRef::ZERO {
                            rows.push(None);
                        } else {
                            rows.push(Some(View::Thread(i)));
                        }
                    }
                }
            }
        }

        {
            rows.push(None);
            rows.push(Some(View::ChannelDir));
            let mut chan_order: Vec<usize> = (0..self.session.channels.len()).collect();
            chan_order.sort_by(|&a, &b| {
                let la = self.session.channels[a]
                    .messages
                    .last()
                    .map(|m| BlockRef {
                        block: m.block_number,
                        index: m.ext_index,
                    })
                    .unwrap_or(BlockRef::ZERO);
                let lb = self.session.channels[b]
                    .messages
                    .last()
                    .map(|m| BlockRef {
                        block: m.block_number,
                        index: m.ext_index,
                    })
                    .unwrap_or(BlockRef::ZERO);
                lb.cmp(&la)
            });
            for i in chan_order {
                if self.session.channels[i].channel_ref == BlockRef::ZERO {
                    rows.push(None);
                } else {
                    rows.push(Some(View::Channel(i)));
                }
            }
        }

        if !self.session.groups.is_empty() {
            rows.push(None);
            rows.push(None);
            let mut group_order: Vec<usize> = (0..self.session.groups.len()).collect();
            group_order.sort_by(|&a, &b| {
                let la = self.session.groups[a]
                    .messages
                    .last()
                    .map(|m| BlockRef {
                        block: m.block_number,
                        index: m.ext_index,
                    })
                    .unwrap_or(BlockRef::ZERO);
                let lb = self.session.groups[b]
                    .messages
                    .last()
                    .map(|m| BlockRef {
                        block: m.block_number,
                        index: m.ext_index,
                    })
                    .unwrap_or(BlockRef::ZERO);
                lb.cmp(&la)
            });
            for i in group_order {
                rows.push(Some(View::Group(i)));
            }
        }

        rows
    }

    pub fn sidebar_len(&self) -> usize {
        self.sidebar_rows().iter().filter(|r| r.is_some()).count()
    }

    pub fn sidebar_index(&self) -> usize {
        self.sidebar_rows()
            .iter()
            .filter(|r| r.is_some())
            .position(|r| *r == Some(self.view))
            .unwrap_or(0)
    }

    pub fn select_sidebar(&mut self, index: usize) {
        if self.mode == Mode::Insert {
            self.save_draft();
            self.mode = Mode::Normal;
        }
        self.scroll_offset = 0;
        let selectable: Vec<View> = self.sidebar_rows().into_iter().flatten().collect();
        if let Some(&view) = selectable.get(index) {
            self.view = view;
        }
        self.mark_read();
    }

    pub fn select_sidebar_row(&mut self, row: usize) {
        let rows = self.sidebar_rows();
        if let Some(Some(view)) = rows.get(row) {
            if self.mode == Mode::Insert {
                self.save_draft();
                self.mode = Mode::Normal;
            }
            self.scroll_offset = 0;
            self.view = *view;
            self.mark_read();
        }
    }

    pub fn next_sidebar(&mut self) {
        let len = self.sidebar_len();
        if len == 0 {
            return;
        }
        let next = (self.sidebar_index() + 1) % len;
        self.select_sidebar(next);
    }

    pub fn prev_sidebar(&mut self) {
        let len = self.sidebar_len();
        if len == 0 {
            return;
        }
        let cur = self.sidebar_index();
        let prev = if cur == 0 { len - 1 } else { cur - 1 };
        self.select_sidebar(prev);
    }
}
