use crate::app::{App, Mode};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use taolk::event::ConnState;

fn reconnect_pill(state: ConnState) -> Option<Span<'static>> {
    match state {
        ConnState::Connected => None,
        ConnState::Reconnecting { in_secs } => Some(Span::styled(
            format!(" reconnecting in {in_secs}s "),
            Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        )),
    }
}

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let mode = match app.mode {
        Mode::Normal => Span::styled(
            " NORMAL ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Mode::Insert => Span::styled(
            " INSERT ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Mode::Compose => Span::styled(
            " NEW THREAD ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Mode::Confirm => Span::styled(
            " CONFIRM ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Mode::Message => Span::styled(
            " MESSAGE ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Mode::CreateChannel => Span::styled(
            " CREATE CHANNEL ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Mode::CreateChannelDesc => Span::styled(
            " CHANNEL DESC ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Mode::CreateGroupMembers => Span::styled(
            " SELECT MEMBERS ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Mode::Search => Span::styled(
            " SEARCH ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Mode::SenderPicker => Span::styled(
            " COPY SS58 ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Mode::Help => Span::styled(
            " HELP ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    };

    let status_span = if let Some((status, is_error)) = app.current_status() {
        if app.is_busy() {
            let spinner = app.spinner_1();
            Span::styled(
                format!(" {spinner} {status} "),
                Style::default().fg(Color::Cyan),
            )
        } else if is_error {
            Span::styled(
                format!(" {} {status} ", super::icons::ERROR),
                Style::default().fg(Color::Red),
            )
        } else {
            Span::styled(
                format!(" {} {status} ", super::icons::SUCCESS),
                Style::default().fg(Color::White),
            )
        }
    } else if !app.search_query.is_empty() {
        Span::styled(
            format!(" /{} ", app.search_query),
            Style::default().fg(Color::Cyan),
        )
    } else {
        Span::raw("")
    };

    let highlight_frames: u32 = 8;

    let balance_str = match app.session.balance {
        Some(bal) => format!(
            " {} ",
            taolk::util::format_balance_short(
                bal,
                app.session.token_decimals,
                &app.session.token_symbol
            )
        ),
        None => String::new(),
    };
    let balance_fresh = app.frame.wrapping_sub(app.balance_changed_at) < highlight_frames;
    let balance_color = if balance_fresh {
        if app.balance_decreased {
            Color::Red
        } else {
            Color::Green
        }
    } else {
        Color::White
    };
    let balance_span = Span::styled(&balance_str, Style::default().fg(balance_color));

    let block_str = format!(" #{} ", format_number(u128::from(app.session.block_number)));
    let block_fresh = app.frame.wrapping_sub(app.block_changed_at) < highlight_frames;
    let block_color = if block_fresh {
        Color::White
    } else {
        Color::DarkGray
    };
    let block_span = Span::styled(&block_str, Style::default().fg(block_color));

    let reconnect = reconnect_pill(app.connection);
    let reconnect_width = reconnect
        .as_ref()
        .map_or(0, |s| u16::try_from(s.width()).unwrap_or(u16::MAX));

    let right_width = u16::try_from(balance_str.len()).unwrap_or(u16::MAX)
        + u16::try_from(block_str.len()).unwrap_or(u16::MAX)
        + reconnect_width;
    let mode_width = u16::try_from(mode.width()).unwrap_or(u16::MAX);
    let max_status = usize::from(area.width.saturating_sub(mode_width + right_width + 1));
    let status_span = if status_span.width() > max_status {
        let content = status_span.content.to_string();
        let truncated = if max_status > 2 {
            format!("{}\u{2026}", &content[..max_status - 1])
        } else {
            String::new()
        };
        Span::styled(truncated, status_span.style)
    } else {
        status_span
    };
    let used = mode_width + u16::try_from(status_span.width()).unwrap_or(u16::MAX);
    let padding = area.width.saturating_sub(used + right_width);
    let pad_span = Span::raw(" ".repeat(usize::from(padding)));

    let mut spans = vec![mode, status_span, pad_span];
    if let Some(rc) = reconnect {
        spans.push(rc);
    }
    spans.push(balance_span);
    spans.push(block_span);

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

use taolk::util::format_number;
