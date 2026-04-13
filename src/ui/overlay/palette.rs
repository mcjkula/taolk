use crate::app::App;
use crate::cmd::registry::{COMMANDS, Command};
use crate::ui::chrome;
use crate::ui::composer::TextBuffer;
use crate::ui::modal::centered_rect;
use crate::ui::palette;
use crossterm::event::{KeyCode, KeyEvent};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use ratatui::Frame;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, ListState, Paragraph};

#[derive(Default, Debug)]
pub struct PaletteState {
    pub query: TextBuffer,
    pub cursor: usize,
    ranking: Vec<RankedCommand>,
}

#[derive(Debug, Clone, Copy)]
struct RankedCommand {
    idx: usize,
    score: u16,
}

pub enum Action {
    None,
    Close,
    Run(&'static Command, String),
}

impl PaletteState {
    pub fn new() -> Self {
        let mut s = Self::default();
        s.recompute();
        s
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => Action::Close,
            KeyCode::Enter => {
                if let Some(ranked) = self.ranking.get(self.cursor) {
                    let cmd = &COMMANDS[ranked.idx];
                    let query = self.query.as_str().to_string();
                    let args = parse_args(&query);
                    return Action::Run(cmd, args);
                }
                Action::Close
            }
            KeyCode::Up | KeyCode::BackTab => {
                self.cursor = self.cursor.saturating_sub(1);
                Action::None
            }
            KeyCode::Down | KeyCode::Tab => {
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
        let raw = self.query.as_str();
        let needle_str = raw.split_whitespace().next().unwrap_or("");
        let mut needle_buf: Vec<char> = Vec::new();
        let mut hay_buf: Vec<char> = Vec::new();

        let mut ranked: Vec<RankedCommand> = Vec::new();
        for (idx, cmd) in COMMANDS.iter().enumerate() {
            if needle_str.is_empty() {
                ranked.push(RankedCommand { idx, score: 0 });
                continue;
            }
            let needle = Utf32Str::new(needle_str, &mut needle_buf);
            let hay = Utf32Str::new(cmd.name, &mut hay_buf);
            if let Some(score) = matcher.fuzzy_match(hay, needle) {
                ranked.push(RankedCommand { idx, score });
            }
        }
        ranked.sort_by(|a, b| b.score.cmp(&a.score));
        self.ranking = ranked;
        if self.cursor >= self.ranking.len() {
            self.cursor = self.ranking.len().saturating_sub(1);
        }
    }
}

fn parse_args(query: &str) -> String {
    query
        .split_whitespace()
        .skip(1)
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let width = area.width.saturating_sub(8).min(72);
    let height = area.height.saturating_sub(6).min(18);
    let rect = centered_rect(area, width, height);

    frame.render_widget(Clear, rect);

    let block = chrome::panel(false).title(" commands ");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let rows = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Length(1),
            ratatui::layout::Constraint::Min(1),
        ])
        .split(inner);

    let prompt = Line::from(vec![
        Span::styled(" > ", Style::default().fg(palette::ACCENT)),
        Span::styled(
            app.palette
                .as_ref()
                .map_or("", |p| p.query.as_str())
                .to_string(),
            Style::default().fg(ratatui::style::Color::Reset),
        ),
    ]);
    frame.render_widget(Paragraph::new(prompt), rows[0]);

    let selected_style = Style::default()
        .fg(palette::ACCENT)
        .add_modifier(Modifier::BOLD);
    let name_style = Style::default().fg(ratatui::style::Color::Reset);
    let summary_style = Style::default().fg(palette::MUTED);

    let state = match &app.palette {
        Some(s) => s,
        None => return,
    };
    let items: Vec<ListItem> = state
        .ranking
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let cmd = &COMMANDS[r.idx];
            let selected = i == state.cursor;
            let prefix = if selected { " \u{25B8} " } else { "   " };
            let line = Line::from(vec![
                Span::styled(
                    prefix.to_string(),
                    if selected { selected_style } else { name_style },
                ),
                Span::styled(
                    format!("{} ", cmd.glyph),
                    if selected { selected_style } else { name_style },
                ),
                Span::styled(
                    format!("{:<13} ", cmd.name),
                    if selected { selected_style } else { name_style },
                ),
                Span::styled(cmd.summary.to_string(), summary_style),
            ]);
            ListItem::new(line)
        })
        .collect();

    let mut list_state = ListState::default().with_selected(Some(state.cursor));
    frame.render_stateful_widget(List::new(items), rows[1], &mut list_state);

    let cursor_x = rows[0].x + 3 + u16::try_from(state.query.cursor()).unwrap_or(u16::MAX);
    let cursor_y = rows[0].y;
    if cursor_x < rows[0].x + rows[0].width {
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_seeds_all_commands() {
        let s = PaletteState::new();
        assert_eq!(s.ranking.len(), COMMANDS.len());
    }

    #[test]
    fn typing_filters_by_prefix() {
        let mut s = PaletteState::new();
        s.query.insert_char('h');
        s.query.insert_char('e');
        s.query.insert_char('l');
        s.recompute();
        let top = s.ranking.first().expect("nonempty");
        assert_eq!(COMMANDS[top.idx].name, "help");
    }
}
