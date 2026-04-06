use crate::app::{App, Mode};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

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

    // Highlight recently changed values: bright for ~2 seconds (8 frames), then fade
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

    let block_str = format!(" #{} ", format_number(app.session.block_number));
    let block_fresh = app.frame.wrapping_sub(app.block_changed_at) < highlight_frames;
    let block_color = if block_fresh {
        Color::White
    } else {
        Color::DarkGray
    };
    let block_span = Span::styled(&block_str, Style::default().fg(block_color));

    let right_width = balance_str.len() as u16 + block_str.len() as u16;
    let mode_width = mode.width() as u16;
    // Truncate status if it would overflow
    let max_status = area.width.saturating_sub(mode_width + right_width + 1) as usize;
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
    let used = mode_width + status_span.width() as u16;
    let padding = area.width.saturating_sub(used + right_width);
    let pad_span = Span::raw(" ".repeat(padding as usize));

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            mode,
            status_span,
            pad_span,
            balance_span,
            block_span,
        ])),
        area,
    );
}

use taolk::util::format_number;
