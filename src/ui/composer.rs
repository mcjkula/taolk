use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use std::ops::Deref;

#[derive(Default, Debug, Clone)]
pub struct TextBuffer {
    buf: String,
    cursor: usize,
}

impl TextBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.buf.clear();
        self.cursor = 0;
    }

    pub fn as_str(&self) -> &str {
        &self.buf
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn set(&mut self, s: impl Into<String>) {
        self.buf = s.into();
        self.cursor = self.buf.len();
    }

    pub fn insert_char(&mut self, c: char) {
        self.buf.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    pub fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    pub fn delete_before(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let prev = prev_char_boundary(&self.buf, self.cursor);
        self.buf.drain(prev..self.cursor);
        self.cursor = prev;
        true
    }

    pub fn delete_after(&mut self) -> bool {
        if self.cursor >= self.buf.len() {
            return false;
        }
        let next = next_char_boundary(&self.buf, self.cursor);
        self.buf.drain(self.cursor..next);
        true
    }

    pub fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor = prev_char_boundary(&self.buf, self.cursor);
    }

    pub fn move_right(&mut self) {
        if self.cursor >= self.buf.len() {
            return;
        }
        self.cursor = next_char_boundary(&self.buf, self.cursor);
    }

    pub fn move_word_left(&mut self) {
        while self.cursor > 0 {
            let prev = prev_char_boundary(&self.buf, self.cursor);
            if kind_at(&self.buf, prev) != CharKind::Space {
                break;
            }
            self.cursor = prev;
        }
        if self.cursor == 0 {
            return;
        }
        let start_kind = kind_at(&self.buf, prev_char_boundary(&self.buf, self.cursor));
        while self.cursor > 0 {
            let prev = prev_char_boundary(&self.buf, self.cursor);
            if kind_at(&self.buf, prev) != start_kind {
                break;
            }
            self.cursor = prev;
        }
    }

    pub fn move_word_right(&mut self) {
        let len = self.buf.len();
        if self.cursor >= len {
            return;
        }
        let start_kind = kind_at(&self.buf, self.cursor);
        if start_kind != CharKind::Space {
            while self.cursor < len && kind_at(&self.buf, self.cursor) == start_kind {
                self.cursor = next_char_boundary(&self.buf, self.cursor);
            }
        }
        while self.cursor < len && kind_at(&self.buf, self.cursor) == CharKind::Space {
            self.cursor = next_char_boundary(&self.buf, self.cursor);
        }
    }

    pub fn move_home(&mut self) {
        self.cursor = self.buf[..self.cursor].rfind('\n').map_or(0, |i| i + 1);
    }

    pub fn move_end(&mut self) {
        let rest = &self.buf[self.cursor..];
        self.cursor += rest.find('\n').unwrap_or(rest.len());
    }

    pub fn move_line_up(&mut self) {
        let before = &self.buf[..self.cursor];
        let Some(nl) = before.rfind('\n') else {
            return;
        };
        let col = self.cursor - nl - 1;
        let prev_start = before[..nl].rfind('\n').map_or(0, |p| p + 1);
        let prev_len = nl - prev_start;
        self.cursor = prev_start + col.min(prev_len);
    }

    pub fn move_line_down(&mut self) {
        let before = &self.buf[..self.cursor];
        let line_start = before.rfind('\n').map_or(0, |p| p + 1);
        let col = self.cursor - line_start;
        let Some(nl) = self.buf[self.cursor..].find('\n') else {
            return;
        };
        let next_start = self.cursor + nl + 1;
        let next_end = self.buf[next_start..]
            .find('\n')
            .map_or(self.buf.len(), |p| next_start + p);
        let next_len = next_end - next_start;
        self.cursor = next_start + col.min(next_len);
    }

    pub fn cursor_line_col(&self) -> (usize, usize) {
        let before = &self.buf[..self.cursor.min(self.buf.len())];
        let line = before.matches('\n').count();
        let col = before
            .rfind('\n')
            .map_or(self.cursor, |nl| self.cursor - nl - 1);
        (line, col)
    }

    pub fn handle_edit_key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char(c) if !ctrl => {
                self.insert_char(c);
                true
            }
            KeyCode::Backspace => {
                self.delete_before();
                true
            }
            KeyCode::Delete => {
                self.delete_after();
                true
            }
            KeyCode::Left if ctrl => {
                self.move_word_left();
                true
            }
            KeyCode::Right if ctrl => {
                self.move_word_right();
                true
            }
            KeyCode::Left => {
                self.move_left();
                true
            }
            KeyCode::Right => {
                self.move_right();
                true
            }
            KeyCode::Home => {
                self.move_home();
                true
            }
            KeyCode::End => {
                self.move_end();
                true
            }
            KeyCode::Up => {
                self.move_line_up();
                true
            }
            KeyCode::Down => {
                self.move_line_down();
                true
            }
            _ => false,
        }
    }
}

impl Deref for TextBuffer {
    type Target = str;
    fn deref(&self) -> &str {
        &self.buf
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CharKind {
    Space,
    Punct,
    Other,
}

impl CharKind {
    fn of(c: char) -> Self {
        if c.is_whitespace() {
            Self::Space
        } else if c.is_ascii_punctuation() {
            Self::Punct
        } else {
            Self::Other
        }
    }
}

fn kind_at(s: &str, pos: usize) -> CharKind {
    s[pos..]
        .chars()
        .next()
        .map(CharKind::of)
        .unwrap_or(CharKind::Space)
}

fn prev_char_boundary(s: &str, mut pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    pos -= 1;
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

fn next_char_boundary(s: &str, mut pos: usize) -> usize {
    let len = s.len();
    if pos >= len {
        return len;
    }
    pos += 1;
    while pos < len && !s.is_char_boundary(pos) {
        pos += 1;
    }
    pos
}

pub fn render_composer(frame: &mut Frame, app: &App, sep: Line<'_>, area: Rect) {
    use super::input::{compose_hints, styles, visible_input};
    let st = styles(app);
    let prompt = "> ";
    let prompt_width: usize = 3;
    let w = usize::from(area.width);
    let hints = compose_hints(w, app.input.contains('\n'), st);

    if app.input.is_empty() {
        let placeholder = match (&app.msg_recipient, app.msg_type) {
            (Some((_, ss58)), Some(0x01)) => format!("public to {ss58}..."),
            (Some((_, ss58)), Some(0x02)) => format!("encrypted to {ss58}..."),
            (Some((_, ss58)), None) => format!("new thread to {ss58}..."),
            _ if matches!(app.view, crate::app::View::Channel(_)) => {
                "Post to channel...".to_string()
            }
            _ if matches!(app.view, crate::app::View::Group(idx) if app.session.groups.get(idx).is_some_and(|g| g.group_ref == taolk::types::BlockRef::ZERO)) =>
            {
                let n = app.pending_group_members.len();
                format!("First message to group ({n} members)...")
            }
            _ if matches!(app.view, crate::app::View::Group(_)) => "Post to group...".to_string(),
            _ => "Type a message...".to_string(),
        };
        let input_line = Line::from(vec![
            Span::raw(" "),
            Span::styled(prompt, Style::default().fg(st.dim)),
            Span::styled(placeholder, Style::default().fg(st.dim)),
        ]);
        frame.render_widget(Paragraph::new(vec![sep, hints, input_line]), area);
        let cursor_x = area.x + u16::try_from(prompt_width).unwrap_or(u16::MAX);
        let cursor_y = area.y + 2;
        if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
        return;
    }

    let avail = w.saturating_sub(prompt_width + 1);
    let lines_vec: Vec<&str> = app.input.split('\n').collect();
    let total_lines = lines_vec.len();
    let (cursor_line, cursor_col) = app.input.cursor_line_col();

    let max_visible = (usize::from(area.height)).saturating_sub(2).max(1);
    let scroll_start = if cursor_line >= max_visible {
        cursor_line - max_visible + 1
    } else {
        0
    };
    let scroll_end = (scroll_start + max_visible).min(total_lines);

    let mut paragraph_lines: Vec<Line> = vec![sep, hints];
    for i in scroll_start..scroll_end {
        let line_text = lines_vec.get(i).copied().unwrap_or("");
        let is_cursor_line = i == cursor_line;
        let (text_spans, _) = visible_input(
            line_text,
            if is_cursor_line { cursor_col } else { 0 },
            avail,
            None,
            st,
        );
        let line_prompt = if i == scroll_start && scroll_start == 0 {
            prompt
        } else {
            "  "
        };
        let mut spans = vec![
            Span::raw(" "),
            Span::styled(line_prompt, Style::default().fg(st.dim)),
        ];
        spans.extend(text_spans);
        paragraph_lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(paragraph_lines), area);

    let visible_cursor_row = cursor_line - scroll_start;
    let (_, cursor_off) = visible_input(
        lines_vec.get(cursor_line).copied().unwrap_or(""),
        cursor_col,
        avail,
        None,
        st,
    );
    let cursor_x = area.x + u16::try_from(prompt_width).unwrap_or(u16::MAX) + cursor_off;
    let cursor_y = area.y + 2 + u16::try_from(visible_cursor_row).unwrap_or(u16::MAX);
    if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_delete() {
        let mut b = TextBuffer::new();
        b.insert_char('h');
        b.insert_char('i');
        assert_eq!(b.as_str(), "hi");
        assert_eq!(b.cursor(), 2);
        b.delete_before();
        assert_eq!(b.as_str(), "h");
    }

    #[test]
    fn move_left_right_skips_utf8() {
        let mut b = TextBuffer::new();
        for c in "τalk".chars() {
            b.insert_char(c);
        }
        b.move_left();
        b.move_left();
        b.move_left();
        b.move_left();
        assert_eq!(b.cursor(), 0);
        b.move_right();
        assert_eq!(b.cursor(), 2);
    }

    #[test]
    fn word_jump_left_and_right() {
        let mut b = TextBuffer::new();
        for c in "hello world rust".chars() {
            b.insert_char(c);
        }
        b.move_home();
        b.move_word_right();
        assert_eq!(b.cursor(), 6);
        b.move_word_right();
        assert_eq!(b.cursor(), 12);
        b.move_word_left();
        assert_eq!(b.cursor(), 6);
    }

    #[test]
    fn word_jump_splits_on_punct() {
        let mut b = TextBuffer::new();
        for c in "foo.bar".chars() {
            b.insert_char(c);
        }
        b.move_home();
        b.move_word_right();
        assert_eq!(b.cursor(), 3);
    }

    #[test]
    fn move_line_up_down_keeps_column() {
        let mut b = TextBuffer::new();
        for c in "abc\nxy\npqr".chars() {
            b.insert_char(c);
        }
        assert_eq!(b.cursor(), 10);
        b.move_line_up();
        assert_eq!(b.cursor_line_col(), (1, 2));
        b.move_line_up();
        assert_eq!(b.cursor_line_col(), (0, 2));
        b.move_line_down();
        assert_eq!(b.cursor_line_col(), (1, 2));
    }

    #[test]
    fn home_end_on_line() {
        let mut b = TextBuffer::new();
        for c in "foo\nbar".chars() {
            b.insert_char(c);
        }
        b.move_line_up();
        b.move_home();
        assert_eq!(b.cursor(), 0);
        b.move_end();
        assert_eq!(b.cursor(), 3);
    }
}
