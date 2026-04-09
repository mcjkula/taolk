use crate::app::App;
use crate::config::ColorMode;
use crate::ui::chrome;
use crate::ui::hintbar;
use crate::ui::theme::{Theme, apply_mode, theme_for};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use taolk::event::ConnState;
use taolk::util::format_number;

fn reconnect_pill(state: ConnState, theme: &Theme, mode: ColorMode) -> Option<Span<'static>> {
    match state {
        ConnState::Connected => None,
        ConnState::Reconnecting { in_secs } => Some(Span::styled(
            format!(" reconnecting in {in_secs}s "),
            Style::default()
                .fg(apply_mode(mode, theme.bg))
                .bg(apply_mode(mode, theme.error))
                .add_modifier(Modifier::BOLD),
        )),
    }
}

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = theme_for(app.theme);
    let mode = app.color_mode;

    let left = if let Some((status, is_error)) = app.current_status() {
        if app.is_busy() {
            let spinner = app.spinner_1();
            Line::from(Span::styled(
                format!(" {spinner} {status} "),
                Style::default().fg(apply_mode(mode, theme.accent)),
            ))
        } else if is_error {
            Line::from(Span::styled(
                format!(" \u{2717} {status} "),
                Style::default().fg(apply_mode(mode, theme.error)),
            ))
        } else {
            Line::from(Span::styled(
                format!(" \u{2713} {status} "),
                Style::default().fg(apply_mode(mode, theme.text_strong)),
            ))
        }
    } else if !app.search_query.is_empty() {
        Line::from(Span::styled(
            format!(" /{} ", app.search_query),
            Style::default().fg(apply_mode(mode, theme.accent)),
        ))
    } else {
        hintbar::hints(app)
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
            apply_mode(mode, theme.error)
        } else {
            apply_mode(mode, theme.success)
        }
    } else {
        apply_mode(mode, theme.text)
    };
    let balance_span = Span::styled(balance_str.clone(), Style::default().fg(balance_color));

    let block_str = format!(" #{} ", format_number(u128::from(app.session.block_number)));
    let block_fresh = app.frame.wrapping_sub(app.block_changed_at) < highlight_frames;
    let block_color = if block_fresh {
        apply_mode(mode, theme.text_strong)
    } else {
        apply_mode(mode, theme.text_dim)
    };
    let block_span = Span::styled(block_str.clone(), Style::default().fg(block_color));

    let reconnect = reconnect_pill(app.connection, theme, mode);
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
            .fg(apply_mode(mode, theme.bg))
            .bg(apply_mode(mode, theme.warning))
            .add_modifier(Modifier::BOLD),
    );

    let right_width = u16::try_from(locked_str.chars().count()).unwrap_or(u16::MAX)
        + u16::try_from(balance_str.len()).unwrap_or(u16::MAX)
        + u16::try_from(block_str.len()).unwrap_or(u16::MAX)
        + reconnect_width;

    let left_width = u16::try_from(left.width()).unwrap_or(u16::MAX);
    let padding = area.width.saturating_sub(left_width + right_width);
    let pad_span = Span::raw(" ".repeat(usize::from(padding)));

    let mut spans: Vec<Span<'static>> = left.spans.into_iter().collect();
    spans.push(pad_span);
    if let Some(rc) = reconnect {
        spans.push(rc);
    }
    if !locked_str.is_empty() {
        spans.push(locked_span);
    }
    spans.push(balance_span);
    spans.push(block_span);

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(chrome::fill_style(theme, mode)),
        area,
    );
}
