use crate::app::App;
use crate::ui::modal::centered_line;
use crate::ui::theme::{apply_mode, theme_for};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const BANNER: &[&str] = &[
    " ██████╗  █████╗  ██████╗ ██╗     ██╗  ██╗",
    " ╚══██╔╝ ██╔══██╗██╔═══██╗██║     ██║ ██╔╝",
    "    ██║  ███████║██║   ██║██║     █████╔╝ ",
    "    ██║  ██╔══██║██║   ██║██║     ██╔═██╗ ",
    "    ██║  ██║  ██║╚██████╔╝███████╗██║  ██╗",
    "    ╚═╝  ╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚═╝  ╚═╝",
];

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = theme_for(app.theme);
    let mode = app.color_mode;
    let accent = Style::default()
        .fg(apply_mode(mode, theme.accent))
        .add_modifier(Modifier::BOLD);
    let text = Style::default().fg(apply_mode(mode, theme.text));
    let dim = Style::default().fg(apply_mode(mode, theme.text_dim));
    let key = Style::default().fg(apply_mode(mode, theme.accent));

    let ss58 = app.session.my_ss58.clone();
    let content_h: u16 = u16::try_from(BANNER.len() + 12).unwrap_or(u16::MAX);
    let top_pad = area.height.saturating_sub(content_h) / 2;

    let mut lines: Vec<Line<'static>> = Vec::new();
    for _ in 0..top_pad {
        lines.push(Line::raw(""));
    }
    for b in BANNER {
        lines.push(centered_line(b, area.width, accent));
    }
    lines.push(Line::raw(""));
    lines.push(centered_line(
        "Substrate Account Messaging Protocol",
        area.width,
        dim,
    ));
    lines.push(Line::raw(""));
    lines.push(centered_line(&format!("you: {ss58}"), area.width, text));
    lines.push(Line::raw(""));
    lines.push(Line::raw(""));

    let shortcuts: &[(&str, &str)] = &[
        ("n", "new thread"),
        ("m", "standalone message"),
        ("c", "channels"),
        ("g", "create group"),
        ("?", "help"),
        ("q", "quit"),
    ];
    for (k, label) in shortcuts {
        let row = format!("  {k}   {label}");
        let pad = usize::from(area.width).saturating_sub(row.chars().count()) / 2;
        let spans = vec![
            Span::raw(" ".repeat(pad)),
            Span::styled(format!("  {k}   "), key),
            Span::styled((*label).to_string(), text),
        ];
        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

pub fn should_render(app: &App) -> bool {
    use crate::app::View;
    app.view == View::Inbox
        && app.session.inbox.is_empty()
        && app.session.outbox.is_empty()
        && app.session.threads.is_empty()
        && app.session.channels.is_empty()
        && app.session.groups.is_empty()
}
