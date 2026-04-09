use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use crate::app::App;
use crate::ui::palette;

struct Card {
    title: &'static str,
    entries: &'static [(&'static str, &'static str)],
}

const CARDS: &[Card] = &[
    Card {
        title: "Sidebar",
        entries: &[
            ("\u{2191} / \u{2193}", "Previous / next conversation"),
            ("Tab / S-Tab", "Previous / next conversation"),
            ("Space", "Toggle sidebar"),
        ],
    },
    Card {
        title: "Page content",
        entries: &[
            ("j / k", "Down / up one line"),
            ("C-d / C-u", "Half-page down / up"),
            ("PgDn / PgUp", "Page down / up"),
            ("G / End", "Bottom"),
            ("Home", "Top"),
        ],
    },
    Card {
        title: "Actions",
        entries: &[
            ("i", "Compose or reply in current"),
            ("n", "New thread"),
            ("m", "Standalone message"),
            ("c", "Browse channels"),
            ("g", "Create group"),
            ("/", "Search current view"),
            ("y", "Copy sender SS58"),
            ("r", "Refresh / fill DAG gaps"),
            ("U", "Unlock all locked outbound"),
            ("?", "Show this help"),
            ("q", "Quit (warns if drafts)"),
            ("C-c", "Quit immediately"),
            ("C-l", "Lock session"),
            ("C-w", "Switch wallet"),
        ],
    },
    Card {
        title: "Channel directory",
        entries: &[
            ("j / k", "Move channel cursor"),
            ("digits / :", "Type a channel ref"),
            ("Enter", "Subscribe to ref"),
            ("+", "Create a new channel"),
            ("Esc", "Clear input or back"),
        ],
    },
    Card {
        title: "Insert",
        entries: &[
            ("Enter", "Send (preview fee)"),
            ("C-n", "Insert newline"),
            ("Esc", "Save draft and exit"),
            ("C-\u{2190} / C-\u{2192}", "Jump by word"),
            ("Backspace", "Delete left"),
        ],
    },
    Card {
        title: "Confirm",
        entries: &[("Enter", "Submit transaction"), ("Esc", "Back to edit")],
    },
    Card {
        title: "Compose / Message",
        entries: &[
            ("type", "Filter or paste SS58"),
            ("\u{2191} / \u{2193}", "Pick contact"),
            ("C-n", "Swap public/encrypted"),
            ("Enter", "Confirm and compose"),
            ("Esc", "Cancel"),
        ],
    },
    Card {
        title: "Sender picker",
        entries: &[
            ("\u{2191} / \u{2193}", "Pick sender"),
            ("Enter", "Copy SS58 to clipboard"),
            ("Esc", "Cancel"),
        ],
    },
    Card {
        title: "Group members",
        entries: &[
            ("type", "Filter or paste SS58"),
            ("\u{2191} / \u{2193}", "Pick contact"),
            ("Enter", "Add or remove"),
            ("Tab", "Done, create group"),
            ("Esc", "Cancel"),
        ],
    },
];

const CARD_H_PAD: usize = 2;
const CARD_ENTRY_GAP: usize = 2;
const COLUMN_GAP: usize = 2;
const SIDE_MARGIN: usize = 2;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let accent = Style::default()
        .fg(palette::ACCENT)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(palette::MUTED);

    let body = Rect {
        x: area.x,
        y: area.y.saturating_add(1),
        width: area.width,
        height: area.height.saturating_sub(2),
    };

    let header = Paragraph::new(Line::from(vec![Span::styled(
        " taolk \u{2014} help ",
        accent,
    )]))
    .alignment(Alignment::Center);
    let header_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };
    frame.render_widget(header, header_area);

    let width = usize::from(body.width);
    let card_width = compute_card_width();
    let columns_qty =
        ((width.saturating_sub(SIDE_MARGIN * 2) + COLUMN_GAP) / (card_width + COLUMN_GAP)).max(1);

    let mut columns: Vec<Vec<Line<'static>>> = vec![Vec::new(); columns_qty];
    let mut heights: Vec<usize> = vec![0; columns_qty];

    for card in CARDS {
        let card_lines = render_card(card, card_width);
        let (idx, _) = heights.iter().enumerate().min_by_key(|(_, h)| **h).unwrap();
        if !columns[idx].is_empty() {
            columns[idx].push(blank_line(card_width));
        }
        heights[idx] += card_lines.len() + 1;
        columns[idx].extend(card_lines);
    }

    let grid_width = columns_qty * card_width + columns_qty.saturating_sub(1) * COLUMN_GAP;
    let left_pad = width.saturating_sub(grid_width) / 2;

    let max_rows = columns.iter().map(|c| c.len()).max().unwrap_or(0);
    let mut page: Vec<Line<'static>> = Vec::with_capacity(max_rows);
    for row in 0..max_rows {
        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.push(Span::raw(" ".repeat(left_pad)));
        for (j, col) in columns.iter().enumerate() {
            if j > 0 {
                spans.push(Span::raw(" ".repeat(COLUMN_GAP)));
            }
            if row < col.len() {
                spans.extend(col[row].spans.iter().cloned());
            } else {
                spans.push(Span::raw(" ".repeat(card_width)));
            }
        }
        page.push(Line::from(spans));
    }

    let visible = body.height;
    let grid_rows = u16::try_from(max_rows).unwrap_or(u16::MAX);
    let top_pad = visible.saturating_sub(grid_rows) / 2;
    if top_pad > 0 {
        let blank = Line::from(Span::raw(String::new()));
        for _ in 0..top_pad {
            page.insert(0, blank.clone());
        }
    }

    let total_rows = u16::try_from(page.len()).unwrap_or(u16::MAX);
    let max_scroll = total_rows.saturating_sub(visible);
    let scroll = app.help_scroll.get().min(max_scroll);
    app.help_scroll.set(scroll);
    frame.render_widget(Paragraph::new(page).scroll((scroll, 0)), body);

    if total_rows > visible {
        let footer = Paragraph::new(Line::from(Span::styled(
            format!(" {}/{} ", scroll + 1, max_scroll + 1),
            dim,
        )))
        .alignment(Alignment::Right);
        let footer_area = Rect {
            x: area.x,
            y: area.y + area.height.saturating_sub(1),
            width: area.width,
            height: 1,
        };
        frame.render_widget(footer, footer_area);
    }
}

fn compute_card_width() -> usize {
    let mut max_entry = 0usize;
    let mut max_title = 0usize;
    for card in CARDS {
        max_title = max_title.max(UnicodeWidthStr::width(card.title));
        for (key, desc) in card.entries {
            let w = UnicodeWidthStr::width(*key) + UnicodeWidthStr::width(*desc) + CARD_ENTRY_GAP;
            max_entry = max_entry.max(w);
        }
    }
    (max_entry + CARD_H_PAD * 2).max(max_title + CARD_H_PAD * 2 + 2)
}

fn render_card(card: &Card, width: usize) -> Vec<Line<'static>> {
    let title_style = Style::default()
        .fg(palette::ACCENT_ALT)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(ratatui::style::Color::Reset);
    let key_style = Style::default().fg(palette::ACCENT);

    let mut lines = Vec::with_capacity(card.entries.len() + 3);
    let title_w = UnicodeWidthStr::width(card.title);
    let left = (width.saturating_sub(title_w)) / 2;
    let right = width.saturating_sub(title_w).saturating_sub(left);
    lines.push(Line::from(vec![
        Span::raw(" ".repeat(left)),
        Span::styled(card.title.to_string(), title_style),
        Span::raw(" ".repeat(right)),
    ]));
    lines.push(blank_line(width));

    for (key, desc) in card.entries {
        let key_w = UnicodeWidthStr::width(*key);
        let desc_w = UnicodeWidthStr::width(*desc);
        let used = CARD_H_PAD * 2 + key_w + desc_w;
        let gap = width.saturating_sub(used).max(1);
        lines.push(Line::from(vec![
            Span::raw(" ".repeat(CARD_H_PAD)),
            Span::styled((*desc).to_string(), desc_style),
            Span::raw(" ".repeat(gap)),
            Span::styled((*key).to_string(), key_style),
            Span::raw(" ".repeat(CARD_H_PAD)),
        ]));
    }
    lines.push(blank_line(width));
    lines
}

fn blank_line(width: usize) -> Line<'static> {
    Line::from(Span::raw(" ".repeat(width)))
}
