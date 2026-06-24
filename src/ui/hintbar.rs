use crate::app::{App, Focus, Overlay, View};
use crate::ui::icons;
use crate::ui::palette;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

pub fn hints(app: &App) -> Line<'static> {
    let key = Style::default().fg(palette::theme().accent);
    let desc = palette::dim();

    let pairs = pairs_for(app);
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(pairs.len() * 2 + 1);
    spans.push(Span::raw(" "));
    for (k, d) in pairs {
        // `{updn}` is a placeholder for the up/down nav arrows in the active theme.
        let k = if *k == "{updn}" {
            format!("{}{}", icons::icons().arrow_up, icons::icons().arrow_down)
        } else {
            (*k).to_string()
        };
        spans.push(Span::styled(format!("{k} "), key));
        spans.push(Span::styled(format!("{d}  "), desc));
    }
    Line::from(spans)
}

fn pairs_for(app: &App) -> &'static [(&'static str, &'static str)] {
    match app.overlay {
        Some(Overlay::Help) => &[("j/k", "scroll"), ("any", "close")],
        Some(Overlay::Confirm) => &[("Enter", "confirm"), ("Esc", "back")],
        Some(Overlay::Compose) => &[("{updn}", "nav"), ("Enter", "select"), ("Esc", "cancel")],
        Some(Overlay::Message) => {
            if app.msg_recipient.is_some() {
                &[("p", "public"), ("e", "encrypted"), ("Esc", "cancel")]
            } else {
                &[("{updn}", "nav"), ("Enter", "select"), ("Esc", "cancel")]
            }
        }
        Some(Overlay::CreateChannel) => &[("Enter", "next"), ("Esc", "cancel")],
        Some(Overlay::CreateChannelDesc) => &[("Enter", "create"), ("Esc", "back")],
        Some(Overlay::CreateGroupMembers) => &[
            ("{updn}", "nav"),
            ("Enter", "toggle"),
            ("Tab", "done"),
            ("Esc", "cancel"),
        ],
        Some(Overlay::Search) => &[("Enter", "apply"), ("Esc", "clear")],
        Some(Overlay::SenderPicker) => &[("{updn}", "nav"), ("Enter", "copy"), ("Esc", "cancel")],
        Some(Overlay::CommandPalette) => &[("{updn}", "nav"), ("Enter", "run"), ("Esc", "cancel")],
        Some(Overlay::FuzzyJump) => &[("{updn}", "nav"), ("Enter", "jump"), ("Esc", "cancel")],
        None => match app.focus {
            Focus::Composer => &[
                ("Enter", "send"),
                ("S-Ent", "newline"),
                ("/", "cmd"),
                ("Esc", "leave"),
            ],
            Focus::Timeline => match app.view {
                View::Thread(_) | View::Channel(_) | View::Group(_) => &[
                    ("i", "compose"),
                    ("/", "cmd"),
                    ("C-f", "find"),
                    ("C-j", "jump"),
                    ("?", "help"),
                    ("q", "quit"),
                ],
                View::ChannelDir => &[
                    ("j/k", "browse"),
                    ("Enter", "subscribe"),
                    ("+", "create"),
                    ("Esc", "back"),
                    ("?", "help"),
                ],
                _ => &[("/", "cmd"), ("C-j", "jump"), ("?", "help"), ("q", "quit")],
            },
        },
    }
}
