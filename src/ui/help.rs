use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

pub const KEYBINDS: &[(&str, &[(&str, &str)])] = &[
    (
        "Global",
        &[
            ("?", "Show this help"),
            ("Ctrl+L", "Lock session"),
            ("Ctrl+W", "Switch wallet"),
            ("Ctrl+C", "Quit immediately"),
        ],
    ),
    (
        "Normal",
        &[
            ("i", "Compose / reply"),
            ("n", "New thread"),
            ("m", "Standalone message (public or encrypted)"),
            ("c", "Browse channels"),
            ("g", "Create group"),
            ("/", "Search current view"),
            ("y", "Copy sender SS58"),
            ("r", "Refresh / fill DAG gaps"),
            ("Space", "Toggle sidebar"),
            ("j / k", "Down / up"),
            ("Ctrl+D / Ctrl+U", "Half-page down / up"),
            ("PgDn / PgUp", "Page down / up"),
            ("G / Home", "Bottom / top"),
            ("Tab / Shift-Tab", "Next / previous conversation"),
            ("q", "Quit (warns if drafts exist)"),
        ],
    ),
    (
        "Insert",
        &[
            ("Enter", "Send (preview fee in Confirm)"),
            ("Ctrl+N", "Insert newline"),
            ("Esc", "Save draft and exit"),
            ("Backspace", "Delete left"),
            ("Ctrl+\u{2190} / Ctrl+\u{2192}", "Word jump"),
        ],
    ),
    (
        "Confirm",
        &[("Enter", "Submit transaction"), ("Esc", "Back to edit")],
    ),
    (
        "Compose / Message",
        &[
            ("type", "Filter contacts or paste SS58"),
            ("\u{2191} / \u{2193}", "Pick contact"),
            ("Enter", "Confirm and start composing"),
            ("Esc", "Cancel"),
        ],
    ),
    (
        "Sender picker (y)",
        &[
            ("\u{2191} / \u{2193}", "Pick sender"),
            ("Enter", "Copy SS58 to clipboard"),
            ("Esc", "Cancel"),
        ],
    ),
];

pub fn render(frame: &mut Frame, area: Rect) {
    let content = build_lines();
    let want_h = (content.len() as u16 + 4).min(area.height);
    let want_w = 64.min(area.width);
    let rect = super::modal::centered_rect(area, want_w, want_h);

    frame.render_widget(Clear, rect);
    let block = Block::default()
        .title(Span::styled(
            " Help — press any key to close ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    frame.render_widget(Paragraph::new(content), inner);
}

fn build_lines() -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    for (group, rows) in KEYBINDS {
        lines.push(Line::styled(
            format!(" {group}"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        for (key, desc) in *rows {
            lines.push(Line::from(vec![
                Span::styled(format!("   {key:<18}"), Style::default().fg(Color::Cyan)),
                Span::styled(*desc, Style::default().fg(Color::White)),
            ]));
        }
        lines.push(Line::raw(""));
    }
    lines
}
