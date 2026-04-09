use crate::app::{App, Focus, Overlay};
use crate::ui::theme::{apply_mode, theme_for};
use ratatui::style::Style;
use ratatui::text::{Line, Span};

pub fn hints(app: &App) -> Line<'static> {
    let theme = theme_for(app.theme);
    let mode = app.color_mode;
    let key = Style::default().fg(apply_mode(mode, theme.accent));
    let desc = Style::default().fg(apply_mode(mode, theme.text_dim));

    let pairs = pairs_for(app);
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(pairs.len() * 2 + 1);
    spans.push(Span::raw(" "));
    for (k, d) in pairs {
        spans.push(Span::styled(format!("{k} "), key));
        spans.push(Span::styled(format!("{d}  "), desc));
    }
    Line::from(spans)
}

fn pairs_for(app: &App) -> &'static [(&'static str, &'static str)] {
    match app.overlay {
        Some(Overlay::Help) => &[("any key", "close")],
        Some(Overlay::Confirm) => &[("Enter", "confirm"), ("Esc", "back")],
        Some(Overlay::Compose) => &[
            ("\u{2191}\u{2193}", "nav"),
            ("Enter", "select"),
            ("Esc", "cancel"),
        ],
        Some(Overlay::Message) => &[
            ("\u{2191}\u{2193}", "nav"),
            ("Enter", "select"),
            ("p", "public"),
            ("e", "encrypted"),
            ("Esc", "cancel"),
        ],
        Some(Overlay::CreateChannel) => &[("Enter", "next"), ("Esc", "cancel")],
        Some(Overlay::CreateChannelDesc) => &[("Enter", "create"), ("Esc", "back")],
        Some(Overlay::CreateGroupMembers) => &[
            ("\u{2191}\u{2193}", "nav"),
            ("Enter", "toggle"),
            ("Tab", "done"),
            ("Esc", "cancel"),
        ],
        Some(Overlay::Search) => &[("Enter", "apply"), ("Esc", "clear")],
        Some(Overlay::SenderPicker) => &[
            ("\u{2191}\u{2193}", "nav"),
            ("Enter", "copy"),
            ("Esc", "cancel"),
        ],
        None => match app.focus {
            Focus::Composer => &[("Enter", "send"), ("C-n", "newline"), ("Esc", "leave")],
            Focus::Timeline => &[
                ("i", "compose"),
                ("n", "thread"),
                ("m", "message"),
                ("c", "channels"),
                ("g", "group"),
                ("/", "search"),
                ("?", "help"),
                ("q", "quit"),
            ],
        },
    }
}
