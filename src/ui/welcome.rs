use crate::app::App;
use crate::ui::icons;
use crate::ui::palette;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let _ = app;
    let key = Style::default().fg(palette::ACCENT);
    let text = Style::default();

    let shortcuts: &[(&str, &str, &str)] = &[
        ("n", icons::THREADS, "new thread"),
        ("m", icons::OUTBOX, "standalone message"),
        ("c", icons::CHANNELS, "channels"),
        ("g", icons::GROUPS, "create group"),
        ("?", icons::HELP, "help"),
        ("q", icons::EXIT, "quit"),
    ];

    let content_h = u16::try_from(shortcuts.len()).unwrap_or(u16::MAX);
    let top_pad = area.height.saturating_sub(content_h) / 2;

    let mut lines: Vec<Line<'static>> = Vec::new();
    for _ in 0..top_pad {
        lines.push(Line::raw(""));
    }
    for (k, glyph, label) in shortcuts {
        let row = format!("  {k}   {glyph} {label}");
        let pad = usize::from(area.width).saturating_sub(row.chars().count()) / 2;
        lines.push(Line::from(vec![
            Span::raw(" ".repeat(pad)),
            Span::styled(format!("  {k}   "), key),
            Span::styled(format!("{glyph} "), key),
            Span::styled((*label).to_string(), text),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

pub fn should_render(app: &App) -> bool {
    use crate::app::View;
    app.view == View::Inbox
        && app.msg_recipient.is_none()
        && app.session.inbox.is_empty()
        && app.session.outbox.is_empty()
        && app.session.threads.is_empty()
        && app.session.channels.is_empty()
        && app.session.groups.is_empty()
}
