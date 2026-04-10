use crate::app::App;
use crate::ui::hintbar;
use crate::ui::icons;
use crate::ui::palette;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use taolk::event::ConnState;
use taolk::util::format_number;

fn reconnect_pill(state: ConnState) -> Option<Span<'static>> {
    match state {
        ConnState::Connected => None,
        ConnState::Reconnecting { in_secs } => Some(Span::styled(
            format!(" {} reconnecting in {in_secs}s ", icons::SYNC),
            Style::default()
                .fg(palette::ERROR)
                .add_modifier(Modifier::REVERSED | Modifier::BOLD),
        )),
    }
}

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let left = if let Some((status, is_error)) = app.current_status() {
        if app.is_busy() {
            let spinner = app.spinner_1();
            Line::from(Span::styled(
                format!(" {spinner} {status} "),
                Style::default().fg(palette::ACCENT),
            ))
        } else if is_error {
            Line::from(Span::styled(
                format!(" {} {status} ", icons::ERROR),
                Style::default().fg(palette::ERROR),
            ))
        } else {
            Line::from(Span::styled(
                format!(" {} {status} ", icons::CHECK),
                palette::strong(),
            ))
        }
    } else if !app.search_query.is_empty() {
        Line::from(Span::styled(
            format!(" /{} ", app.search_query),
            Style::default().fg(palette::ACCENT),
        ))
    } else {
        hintbar::hints(app)
    };

    let highlight_frames: u32 = 8;

    let bal = app.session.balance.unwrap_or(0);
    let balance_str = format!(
        " {} ",
        taolk::util::format_balance_short(
            bal,
            app.session.token_decimals,
            &app.session.token_symbol
        )
    );
    let balance_fresh = app.frame.wrapping_sub(app.balance_changed_at) < highlight_frames;
    let balance_style = if balance_fresh {
        if app.balance_decreased {
            Style::default().fg(palette::ERROR)
        } else {
            Style::default().fg(palette::SUCCESS)
        }
    } else {
        Style::default()
    };

    let block_str = format!(
        " {} {} ",
        icons::BLOCK,
        format_number(u128::from(app.session.block_number))
    );
    let block_fresh = app.frame.wrapping_sub(app.block_changed_at) < highlight_frames;
    let block_style = if block_fresh {
        palette::strong()
    } else {
        palette::dim()
    };

    let reconnect = reconnect_pill(app.connection);
    let reconnect_width = reconnect
        .as_ref()
        .map_or(0, |s| u16::try_from(s.width()).unwrap_or(u16::MAX));

    let locked_str = if app.locked_outbound.is_empty() {
        String::new()
    } else {
        format!(" {} {} (U) ", icons::LOCK_CLOCK, app.locked_outbound.len())
    };

    let right_width = u16::try_from(locked_str.chars().count()).unwrap_or(0)
        + u16::try_from(balance_str.chars().count()).unwrap_or(0)
        + u16::try_from(block_str.chars().count()).unwrap_or(0)
        + reconnect_width;

    let cols =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(right_width)]).split(area);

    frame.render_widget(Paragraph::new(left), cols[0]);

    let mut right_spans: Vec<Span<'static>> = Vec::new();
    if let Some(rc) = reconnect {
        right_spans.push(rc);
    }
    if !locked_str.is_empty() {
        right_spans.push(Span::styled(
            locked_str,
            Style::default()
                .fg(palette::WARNING)
                .add_modifier(Modifier::REVERSED | Modifier::BOLD),
        ));
    }
    right_spans.push(Span::styled(balance_str, balance_style));
    right_spans.push(Span::styled(block_str, block_style));

    frame.render_widget(Paragraph::new(Line::from(right_spans)), cols[1]);
}
