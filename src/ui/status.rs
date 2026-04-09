use crate::app::{App, Focus, Overlay};
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

fn pill(label: &'static str) -> Span<'static> {
    Span::styled(
        label,
        Style::default()
            .fg(Color::White)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
}

fn mode_label(app: &App) -> Span<'static> {
    match app.overlay {
        Some(Overlay::Help) => pill(" HELP "),
        Some(Overlay::Confirm) => pill(" CONFIRM "),
        Some(Overlay::Compose) => pill(" NEW THREAD "),
        Some(Overlay::Message) => pill(" MESSAGE "),
        Some(Overlay::CreateChannel) => pill(" CREATE CHANNEL "),
        Some(Overlay::CreateChannelDesc) => pill(" CHANNEL DESC "),
        Some(Overlay::CreateGroupMembers) => pill(" SELECT MEMBERS "),
        Some(Overlay::Search) => pill(" SEARCH "),
        Some(Overlay::SenderPicker) => pill(" COPY SS58 "),
        None => match app.focus {
            Focus::Composer => pill(" INSERT "),
            Focus::Timeline => pill(" NORMAL "),
        },
    }
}

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let mode = mode_label(app);

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

    let locked_str = if app.locked_outbound.is_empty() {
        String::new()
    } else {
        format!(" \u{1F512} {} (U) ", app.locked_outbound.len())
    };
    let locked_span = Span::styled(
        locked_str.clone(),
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    let right_width = u16::try_from(locked_str.chars().count()).unwrap_or(u16::MAX)
        + u16::try_from(balance_str.len()).unwrap_or(u16::MAX)
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
    if !locked_str.is_empty() {
        spans.push(locked_span);
    }
    spans.push(balance_span);
    spans.push(block_span);

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

use taolk::util::format_number;
