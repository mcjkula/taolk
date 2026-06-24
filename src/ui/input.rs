use crate::app::{App, Focus, Overlay};
use crate::ui::icons;
use crate::ui::palette;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

pub(super) fn fit(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    if max <= 1 {
        return "\u{2026}".to_string();
    }
    format!("{}\u{2026}", &s[..max - 1])
}

pub(super) fn visible_input(
    text: &str,
    cursor: usize,
    width: usize,
    limit: Option<usize>,
) -> (Vec<Span<'static>>, u16) {
    if text.is_empty() {
        return (vec![], 0);
    }

    let text_len = text.len();
    let over_limit = limit.is_some_and(|l| text_len > l);

    let (start, end) = if text_len <= width {
        (0, text_len)
    } else {
        let half = width / 2;
        let s = if cursor <= half {
            0
        } else if cursor + half >= text_len {
            text_len.saturating_sub(width)
        } else {
            cursor.saturating_sub(half)
        };
        let e = (s + width).min(text_len);
        (s, e)
    };

    let visible = &text[start..end];
    let cursor_x = u16::try_from(cursor - start).unwrap_or(u16::MAX);

    let mut spans: Vec<Span<'static>> = Vec::new();

    if start > 0 {
        spans.push(Span::styled(
            "\u{2026}",
            Style::default().fg(palette::MUTED),
        ));
    }

    if over_limit {
        let Some(lim) = limit else {
            return (spans, cursor_x);
        };
        if start < lim {
            let ok_end = (lim - start).min(visible.len());
            spans.push(Span::styled(
                visible[..ok_end].to_string(),
                Style::default().fg(ratatui::style::Color::Reset),
            ));
            if ok_end < visible.len() {
                spans.push(Span::styled(
                    visible[ok_end..].to_string(),
                    Style::default().fg(palette::ERROR),
                ));
            }
        } else {
            spans.push(Span::styled(
                visible.to_string(),
                Style::default().fg(palette::ERROR),
            ));
        }
    } else {
        spans.push(Span::styled(
            visible.to_string(),
            Style::default().fg(ratatui::style::Color::Reset),
        ));
    }

    if end < text_len {
        spans.push(Span::styled(
            "\u{2026}",
            Style::default().fg(palette::MUTED),
        ));
    }

    if let Some(lim) = limit {
        let counter_color = if text_len > lim {
            palette::ERROR
        } else {
            palette::MUTED
        };
        spans.push(Span::styled(
            format!(" {}/{}", text_len, lim),
            Style::default().fg(counter_color),
        ));
    }

    (spans, cursor_x)
}

fn sep_line(width: u16) -> Line<'static> {
    Line::styled(
        "\u{2500}".repeat(usize::from(width)),
        Style::default().fg(palette::MUTED),
    )
}

fn render_single_input(
    frame: &mut Frame,
    app: &App,
    prompt: &str,
    placeholder: &str,
    limit: Option<usize>,
    area: Rect,
) {
    use unicode_width::UnicodeWidthStr;
    let sep = sep_line(area.width);
    let prompt_span = Span::styled(prompt, Style::default().fg(palette::MUTED));
    let prompt_width = UnicodeWidthStr::width(prompt) + 1;

    if app.input.is_empty() {
        let input_line = Line::from(vec![
            Span::raw(" "),
            prompt_span,
            Span::styled(placeholder, Style::default().fg(palette::MUTED)),
        ]);
        frame.render_widget(Paragraph::new(vec![sep, Line::raw(""), input_line]), area);
        let cursor_x = area.x + u16::try_from(prompt_width).unwrap_or(u16::MAX);
        let cursor_y = area.y + 2;
        if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
        return;
    }

    let avail = (usize::from(area.width)).saturating_sub(prompt_width + 1);
    let counter_width = limit.map_or(0, |l| format!(" {}/{}", app.input.len(), l).len());
    let text_width = avail.saturating_sub(counter_width);
    let (text_spans, cursor_off) =
        visible_input(app.input.as_str(), app.input.cursor(), text_width, limit);

    let mut spans = vec![Span::raw(" "), prompt_span];
    spans.extend(text_spans);
    let input_line = Line::from(spans);
    frame.render_widget(Paragraph::new(vec![sep, Line::raw(""), input_line]), area);

    let cursor_x = area.x + u16::try_from(prompt_width).unwrap_or(u16::MAX) + cursor_off;
    let cursor_y = area.y + 2;
    if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

/// Preview lines for the Confirm overlay: the channel being created, or every
/// line of the message being sent (capped). Used both to render the overlay and
/// to size it (`render_main_panel`), so the height matches the content.
pub(super) fn confirm_preview(app: &App, width: u16) -> Vec<Line<'static>> {
    let reset = Style::default().fg(ratatui::style::Color::Reset);
    let muted = Style::default().fg(palette::MUTED);
    let max = usize::from(width).saturating_sub(4);

    if let Some(name) = &app.pending_channel_name {
        let desc = app.pending_channel_desc.as_deref().unwrap_or("");
        let text = if desc.is_empty() {
            format!("  #{name}")
        } else {
            format!("  #{name} -- {desc}")
        };
        return vec![Line::from(Span::styled(fit(&text, max), reset))];
    }

    let Some(text) = &app.pending_text else {
        return vec![Line::raw("")];
    };
    if text.is_empty() {
        return vec![Line::from(Span::styled(
            " empty",
            muted.add_modifier(Modifier::ITALIC),
        ))];
    }

    let total = text.lines().count();
    if total <= 1 {
        let first = text.lines().next().unwrap_or("");
        return vec![Line::from(Span::styled(
            fit(&format!(" \"{first}\" ({} chars)", text.len()), max),
            reset,
        ))];
    }

    // Multi-line message: show each line (capped), then a dim summary line.
    const MAX_LINES: usize = 6;
    let mut lines: Vec<Line<'static>> = text
        .lines()
        .take(MAX_LINES)
        .map(|l| {
            Line::from(Span::styled(
                format!("  {}", fit(l, max.saturating_sub(2))),
                reset,
            ))
        })
        .collect();
    let suffix = if total > MAX_LINES {
        format!(
            "  \u{2026} +{} more lines ({} chars)",
            total - MAX_LINES,
            text.len()
        )
    } else {
        format!("  ({} chars)", text.len())
    };
    lines.push(Line::from(Span::styled(suffix, muted)));
    lines
}

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let sep = sep_line(area.width);

    match app.overlay {
        Some(Overlay::Search) => {
            render_single_input(
                frame,
                app,
                &format!("{} ", icons::MAGNIFY),
                "Search messages...",
                None,
                area,
            );
        }
        Some(Overlay::CreateChannel) => {
            render_single_input(
                frame,
                app,
                "Channel name: ",
                &format!("max {} characters", samp::CHANNEL_NAME_MAX),
                Some(samp::CHANNEL_NAME_MAX),
                area,
            );
        }
        Some(Overlay::CreateChannelDesc) => {
            render_single_input(
                frame,
                app,
                "Description: ",
                "optional",
                Some(samp::CHANNEL_DESC_MAX),
                area,
            );
        }
        Some(Overlay::CreateGroupMembers) => {
            render_group_member_picker(frame, app, sep, area);
        }
        Some(Overlay::Message) => {
            if app.msg_recipient.is_none() {
                render_picker_input(frame, app, sep, area);
            } else {
                let Some((_, ss58)) = app.msg_recipient.as_ref() else {
                    return;
                };
                let prefix_len = 35;
                let ss58_max = (usize::from(area.width)).saturating_sub(prefix_len);
                let selector = Line::from(vec![
                    Span::raw(" "),
                    Span::styled("[p] ", Style::default().fg(palette::ACCENT)),
                    Span::styled(
                        format!("{} public  ", icons::PUBLIC),
                        Style::default().fg(ratatui::style::Color::Reset),
                    ),
                    Span::styled("[e] ", Style::default().fg(palette::ACCENT)),
                    Span::styled(
                        format!("{} encrypted  ", icons::ENCRYPTED),
                        Style::default().fg(ratatui::style::Color::Reset),
                    ),
                    Span::styled(
                        fit(&format!("to {ss58}"), ss58_max),
                        Style::default().fg(palette::MUTED),
                    ),
                ]);
                frame.render_widget(Paragraph::new(vec![sep, Line::raw(""), selector]), area);
            }
        }
        Some(Overlay::Compose) => {
            render_picker_input(frame, app, sep, area);
        }
        Some(Overlay::Confirm) => {
            let fee_text = match &app.pending_fee {
                Some(fee) => format!("Fee: {fee}"),
                None => format!("{} Estimating fee", app.spinner_1()),
            };
            let action = if app.is_pending_channel() {
                "Create?"
            } else {
                "Send?"
            };

            let confirm_line = Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    format!("{action} "),
                    Style::default()
                        .fg(ratatui::style::Color::Reset)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(fee_text, Style::default().fg(palette::ACCENT)),
            ]);

            // Show who the message is going to so the recipient can be verified
            // before sending. Channels/groups have no single address, so keep the
            // plain separator there.
            let recipient = if let Some((_, ss58)) = &app.msg_recipient {
                Some(ss58.clone())
            } else if let crate::app::View::Thread(idx) = app.view {
                app.session.threads.get(idx).map(|t| t.peer_ss58.clone())
            } else {
                None
            };
            let header = match recipient {
                Some(ss58) => Line::from(vec![
                    Span::raw(" "),
                    Span::styled("To ", Style::default().fg(palette::MUTED)),
                    Span::styled(
                        fit(&ss58, usize::from(area.width).saturating_sub(4)),
                        Style::default().fg(palette::ACCENT),
                    ),
                ]),
                None => sep,
            };

            let mut lines = vec![header];
            lines.extend(confirm_preview(app, area.width));
            lines.push(confirm_line);
            frame.render_widget(Paragraph::new(lines), area);
        }
        Some(Overlay::SenderPicker)
        | Some(Overlay::Help)
        | Some(Overlay::CommandPalette)
        | Some(Overlay::FuzzyJump) => {
            frame.render_widget(
                Paragraph::new(vec![sep, Line::raw(""), Line::raw("")]),
                area,
            );
        }
        None if app.focus == Focus::Composer => {
            super::composer::render_composer(frame, app, sep, area);
        }
        None => {
            let input_line = if let Some(draft) = app.current_draft() {
                let suffix = "  [i to continue]";
                let avail = (usize::from(area.width)).saturating_sub(4 + suffix.len());
                let draft_str = draft.to_string();
                let visible = fit(&draft_str, avail);
                Line::from(vec![
                    Span::raw(" "),
                    Span::styled("> ", Style::default().fg(palette::MUTED)),
                    Span::styled(visible, Style::default().fg(palette::MUTED)),
                    Span::styled(suffix, Style::default().fg(palette::ACCENT)),
                ])
            } else if app.view == crate::app::View::ChannelDir && !app.channel_dir_input.is_empty()
            {
                let prompt = " ID: ";
                let avail = usize::from(area.width).saturating_sub(prompt.len() + 1);
                let (text_spans, cursor_off) = visible_input(
                    &app.channel_dir_input,
                    app.channel_dir_input.len(),
                    avail,
                    None,
                );
                let mut spans = vec![Span::styled(prompt, Style::default().fg(palette::MUTED))];
                spans.extend(text_spans);
                let input_line = Line::from(spans);
                frame.render_widget(
                    Paragraph::new(vec![sep.clone(), Line::raw(""), input_line]),
                    area,
                );
                let cursor_x =
                    area.x + u16::try_from(prompt.len()).unwrap_or(u16::MAX) + cursor_off;
                let cursor_y = area.y + 2;
                if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
                    frame.set_cursor_position((cursor_x, cursor_y));
                }
                return;
            } else {
                Line::raw("")
            };
            frame.render_widget(Paragraph::new(vec![sep, Line::raw(""), input_line]), area);
        }
    }
}

fn render_picker_input(frame: &mut Frame, app: &App, sep: Line<'_>, area: Rect) {
    let w = usize::from(area.width);

    if app.input.is_empty() {
        let prompt = Line::from(vec![
            Span::styled(" To: ", Style::default().fg(palette::MUTED)),
            Span::styled(
                "type to search or paste address",
                Style::default().fg(palette::MUTED),
            ),
        ]);
        frame.render_widget(Paragraph::new(vec![sep, Line::raw(""), prompt]), area);
        let cursor_x = area.x + 5;
        let cursor_y = area.y + 2;
        if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    } else {
        let avail = w.saturating_sub(6);
        let (text_spans, cursor_off) =
            visible_input(app.input.as_str(), app.input.cursor(), avail, None);
        let mut spans = vec![Span::styled(" To: ", Style::default().fg(palette::MUTED))];
        spans.extend(text_spans);
        let input_line = Line::from(spans);
        frame.render_widget(Paragraph::new(vec![sep, Line::raw(""), input_line]), area);
        let cursor_x = area.x + 5 + cursor_off;
        let cursor_y = area.y + 2;
        if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }
}

fn render_group_member_picker(frame: &mut Frame, app: &App, sep: Line<'_>, area: Rect) {
    let w = usize::from(area.width);
    let selected_count = app.pending_group_members.len();

    if app.input.is_empty() {
        let prompt = Line::from(vec![
            Span::styled(
                format!(" Members ({selected_count}): "),
                Style::default().fg(palette::MUTED),
            ),
            Span::styled(
                "type to search or paste address",
                Style::default().fg(palette::MUTED),
            ),
        ]);
        frame.render_widget(Paragraph::new(vec![sep, Line::raw(""), prompt]), area);
        let cursor_x = area.x
            + u16::try_from(format!(" Members ({selected_count}): ").len()).unwrap_or(u16::MAX);
        let cursor_y = area.y + 2;
        if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    } else {
        let avail = w.saturating_sub(10);
        let prompt_str = format!(" ({selected_count}): ");
        let (text_spans, cursor_off) =
            visible_input(app.input.as_str(), app.input.cursor(), avail, None);
        let mut spans = vec![Span::styled(
            &prompt_str,
            Style::default().fg(palette::MUTED),
        )];
        spans.extend(text_spans);
        let input_line = Line::from(spans);
        frame.render_widget(Paragraph::new(vec![sep, Line::raw(""), input_line]), area);
        let cursor_x = area.x + u16::try_from(prompt_str.len()).unwrap_or(u16::MAX) + cursor_off;
        let cursor_y = area.y + 2;
        if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_short_string_unchanged() {
        assert_eq!(fit("hi", 10), "hi");
    }

    #[test]
    fn fit_truncates_with_ellipsis() {
        assert_eq!(fit("hello world", 6), "hello\u{2026}");
    }

    #[test]
    fn fit_max_zero_returns_ellipsis() {
        assert_eq!(fit("hello", 0), "\u{2026}");
    }

    #[test]
    fn fit_max_one_returns_ellipsis() {
        assert_eq!(fit("hello", 1), "\u{2026}");
    }

    #[test]
    fn visible_input_empty_returns_empty_spans() {
        let (spans, cursor_x) = visible_input("", 0, 10, None);
        assert!(spans.is_empty());
        assert_eq!(cursor_x, 0);
    }

    #[test]
    fn visible_input_short_text_no_scroll() {
        let (spans, cursor_x) = visible_input("hello", 5, 20, None);
        assert!(!spans.is_empty());
        assert_eq!(cursor_x, 5);
    }

    #[test]
    fn visible_input_under_limit_no_red_span() {
        let (spans, _) = visible_input("hi", 0, 20, Some(10));
        assert_eq!(spans.len(), 2);
    }

    #[test]
    fn visible_input_over_limit_appends_counter() {
        let (spans, _) = visible_input("toolongtoolong", 0, 20, Some(5));
        assert!(spans.len() >= 2);
    }

    #[test]
    fn visible_input_scrolls_when_text_exceeds_width() {
        let (spans, _) = visible_input("0123456789ABCDEF", 15, 8, None);
        assert!(!spans.is_empty());
    }
}
