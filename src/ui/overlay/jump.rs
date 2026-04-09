use crate::app::{App, View};
use crate::ui::chrome;
use crate::ui::composer::TextBuffer;
use crate::ui::modal::centered_rect;
use crate::ui::palette;
use crossterm::event::{KeyCode, KeyEvent};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use ratatui::Frame;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, Paragraph};

#[derive(Debug, Clone)]
pub struct JumpTarget {
    pub label: String,
    pub hint: &'static str,
    pub view: View,
}

#[derive(Default, Debug)]
pub struct JumpState {
    pub query: TextBuffer,
    pub cursor: usize,
    targets: Vec<JumpTarget>,
    ranking: Vec<RankedTarget>,
}

#[derive(Debug, Clone, Copy)]
struct RankedTarget {
    idx: usize,
    score: u16,
}

pub enum Action {
    None,
    Close,
    Jump(View),
}

impl JumpState {
    pub fn new(app: &App) -> Self {
        let mut targets: Vec<JumpTarget> = Vec::new();
        targets.push(JumpTarget {
            label: "Inbox".into(),
            hint: "inbox",
            view: View::Inbox,
        });
        targets.push(JumpTarget {
            label: "Sent".into(),
            hint: "outbox",
            view: View::Outbox,
        });
        targets.push(JumpTarget {
            label: "Channels".into(),
            hint: "directory",
            view: View::ChannelDir,
        });
        for (i, t) in app.session.threads.iter().enumerate() {
            targets.push(JumpTarget {
                label: t.peer_ss58.clone(),
                hint: "thread",
                view: View::Thread(i),
            });
        }
        for (i, c) in app.session.channels.iter().enumerate() {
            if !app.session.is_subscribed(&c.channel_ref) {
                continue;
            }
            targets.push(JumpTarget {
                label: format!("#{}", c.name),
                hint: "channel",
                view: View::Channel(i),
            });
        }
        for (i, g) in app.session.groups.iter().enumerate() {
            let members = g.members.len();
            targets.push(JumpTarget {
                label: format!("group ({members})"),
                hint: "group",
                view: View::Group(i),
            });
        }
        let mut s = Self {
            query: TextBuffer::new(),
            cursor: 0,
            targets,
            ranking: Vec::new(),
        };
        s.recompute();
        s
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => Action::Close,
            KeyCode::Enter => {
                if let Some(r) = self.ranking.get(self.cursor) {
                    return Action::Jump(self.targets[r.idx].view);
                }
                Action::Close
            }
            KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                Action::None
            }
            KeyCode::Down => {
                if self.cursor + 1 < self.ranking.len() {
                    self.cursor += 1;
                }
                Action::None
            }
            _ => {
                if self.query.handle_edit_key(key) {
                    self.cursor = 0;
                    self.recompute();
                }
                Action::None
            }
        }
    }

    fn recompute(&mut self) {
        let mut matcher = Matcher::new(Config::DEFAULT);
        let needle_str = self.query.as_str();
        let mut needle_buf: Vec<char> = Vec::new();
        let mut hay_buf: Vec<char> = Vec::new();

        let mut ranked: Vec<RankedTarget> = Vec::new();
        for (idx, t) in self.targets.iter().enumerate() {
            if needle_str.is_empty() {
                ranked.push(RankedTarget { idx, score: 0 });
                continue;
            }
            let needle = Utf32Str::new(needle_str, &mut needle_buf);
            let hay = Utf32Str::new(&t.label, &mut hay_buf);
            if let Some(score) = matcher.fuzzy_match(hay, needle) {
                ranked.push(RankedTarget { idx, score });
            }
        }
        ranked.sort_by(|a, b| b.score.cmp(&a.score));
        self.ranking = ranked;
        if self.cursor >= self.ranking.len() {
            self.cursor = self.ranking.len().saturating_sub(1);
        }
    }
}

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let width = area.width.saturating_sub(8).min(72);
    let height = area.height.saturating_sub(6).min(18);
    let rect = centered_rect(area, width, height);

    frame.render_widget(Clear, rect);

    let block = chrome::panel(false).title(" jump ");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let rows = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Length(1),
            ratatui::layout::Constraint::Min(1),
        ])
        .split(inner);

    let state = match &app.jump {
        Some(s) => s,
        None => return,
    };
    let prompt = Line::from(vec![
        Span::styled(" > ", Style::default().fg(palette::ACCENT)),
        Span::styled(
            state.query.as_str().to_string(),
            Style::default().fg(ratatui::style::Color::Reset),
        ),
    ]);
    frame.render_widget(Paragraph::new(prompt), rows[0]);

    let selected_style = Style::default()
        .fg(palette::ACCENT)
        .add_modifier(Modifier::BOLD);
    let name_style = Style::default().fg(ratatui::style::Color::Reset);
    let hint_style = Style::default().fg(palette::MUTED);

    let items: Vec<ListItem> = state
        .ranking
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let t = &state.targets[r.idx];
            let selected = i == state.cursor;
            let prefix = if selected { " \u{25B8} " } else { "   " };
            let line = Line::from(vec![
                Span::styled(
                    prefix.to_string(),
                    if selected { selected_style } else { name_style },
                ),
                Span::styled(
                    format!("{:<20} ", t.label),
                    if selected { selected_style } else { name_style },
                ),
                Span::styled(t.hint.to_string(), hint_style),
            ]);
            ListItem::new(line)
        })
        .collect();
    frame.render_widget(List::new(items), rows[1]);

    let cursor_x = rows[0].x + 3 + u16::try_from(state.query.cursor()).unwrap_or(u16::MAX);
    let cursor_y = rows[0].y;
    if cursor_x < rows[0].x + rows[0].width {
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}
