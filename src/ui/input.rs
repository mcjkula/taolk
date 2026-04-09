use crate::app::{App, Focus, Overlay};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

pub(super) fn render_hints(pairs: &[(&str, &str)], width: usize) -> Line<'static> {
    let mut spans = Vec::new();
    let mut used: usize = 1;
    for (key, label) in pairs {
        let needed = key.len() + 1 + label.len() + 2;
        if used + needed > width {
            break;
        }
        spans.push(Span::styled(
            format!(" {key}"),
            Style::default().fg(Color::Cyan),
        ));
        spans.push(Span::styled(
            format!(" {label} "),
            Style::default().fg(Color::DarkGray),
        ));
        used += needed;
    }
    Line::from(spans)
}

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
            Style::default().fg(Color::DarkGray),
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
                Style::default().fg(Color::White),
            ));
            if ok_end < visible.len() {
                spans.push(Span::styled(
                    visible[ok_end..].to_string(),
                    Style::default().fg(Color::Red),
                ));
            }
        } else {
            spans.push(Span::styled(
                visible.to_string(),
                Style::default().fg(Color::Red),
            ));
        }
    } else {
        spans.push(Span::styled(
            visible.to_string(),
            Style::default().fg(Color::White),
        ));
    }

    if end < text_len {
        spans.push(Span::styled(
            "\u{2026}",
            Style::default().fg(Color::DarkGray),
        ));
    }

    if let Some(lim) = limit {
        let counter_color = if text_len > lim {
            Color::Red
        } else {
            Color::DarkGray
        };
        spans.push(Span::styled(
            format!(" {}/{}", text_len, lim),
            Style::default().fg(counter_color),
        ));
    }

    (spans, cursor_x)
}

fn key_hints(width: usize) -> Line<'static> {
    render_hints(&[("Enter", "next"), ("Esc", "cancel")], width)
}

fn render_single_input(
    frame: &mut Frame,
    app: &App,
    prompt: &str,
    placeholder: &str,
    limit: Option<usize>,
    sep: Line<'_>,
    area: Rect,
) {
    let prompt_span = Span::styled(prompt, Style::default().fg(Color::DarkGray));
    let prompt_width = prompt.len() + 1;

    if app.input.is_empty() {
        let input_line = Line::from(vec![
            Span::raw(" "),
            prompt_span,
            Span::styled(placeholder, Style::default().fg(Color::DarkGray)),
        ]);
        frame.render_widget(
            Paragraph::new(vec![sep, key_hints(usize::from(area.width)), input_line]),
            area,
        );
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
    frame.render_widget(
        Paragraph::new(vec![sep, key_hints(usize::from(area.width)), input_line]),
        area,
    );

    let cursor_x = area.x + u16::try_from(prompt_width).unwrap_or(u16::MAX) + cursor_off;
    let cursor_y = area.y + 2;
    if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

pub(super) fn compose_hints(width: usize, multiline: bool) -> Line<'static> {
    if multiline {
        render_hints(
            &[
                ("Enter", "send"),
                ("Ctrl+N", "newline"),
                ("\u{2191}\u{2193}", "lines"),
                ("Esc", "cancel"),
            ],
            width,
        )
    } else {
        render_hints(
            &[("Enter", "send"), ("Ctrl+N", "newline"), ("Esc", "cancel")],
            width,
        )
    }
}

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let sep = Line::styled(
        "\u{2500}".repeat(usize::from(area.width)),
        Style::default().fg(Color::DarkGray),
    );

    match app.overlay {
        Some(Overlay::Search) => {
            render_single_input(frame, app, "/", "Search messages...", None, sep, area);
        }
        Some(Overlay::CreateChannel) => {
            render_single_input(
                frame,
                app,
                "Channel name: ",
                &format!("max {} characters", samp::CHANNEL_NAME_MAX),
                Some(samp::CHANNEL_NAME_MAX),
                sep,
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
                sep,
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
                let prefix_len = 31;
                let ss58_max = (usize::from(area.width)).saturating_sub(prefix_len);
                let selector = Line::from(vec![
                    Span::raw(" "),
                    Span::styled("[p] ", Style::default().fg(Color::Cyan)),
                    Span::styled("public  ", Style::default().fg(Color::White)),
                    Span::styled("[e] ", Style::default().fg(Color::Cyan)),
                    Span::styled("encrypted  ", Style::default().fg(Color::White)),
                    Span::styled(
                        fit(&format!("to {ss58}"), ss58_max),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]);
                let type_hints = render_hints(
                    &[("p", "public"), ("e", "encrypted"), ("Esc", "cancel")],
                    usize::from(area.width),
                );
                frame.render_widget(Paragraph::new(vec![sep, type_hints, selector]), area);
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

            let is_channel = app.is_pending_channel();
            let action = if is_channel { "Create?" } else { "Send?" };
            let esc_label = if is_channel { " back" } else { " edit" };

            let (preview, is_empty_preview) = if let Some(name) = &app.pending_channel_name {
                let desc = app.pending_channel_desc.as_deref().unwrap_or("");
                let text = if desc.is_empty() {
                    format!("  #{name}")
                } else {
                    format!("  #{name} -- {desc}")
                };
                let max = usize::from(area.width) - 2;
                let s = if text.len() > max {
                    format!("{}\u{2026}", &text[..max.saturating_sub(1)])
                } else {
                    text
                };
                (s, false)
            } else if let Some(text) = &app.pending_text {
                let first = text.lines().next().unwrap_or("");
                let max = (usize::from(area.width)).saturating_sub(16);
                let display = if first.len() > max {
                    format!("\"{}\u{2026}\"", &first[..max.saturating_sub(3)])
                } else {
                    format!("\"{first}\"")
                };
                if text.is_empty() {
                    (" empty".to_string(), true)
                } else {
                    (format!(" {display} ({} chars)", text.len()), false)
                }
            } else {
                (String::new(), false)
            };
            let preview_style = if is_empty_preview {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC)
            } else {
                Style::default().fg(Color::White)
            };
            let preview_line = Line::from(vec![Span::styled(preview, preview_style)]);

            let confirm_line = Line::from(vec![
                Span::raw(" "),
                Span::styled(format!("{action} "), Style::default().fg(Color::White)),
                Span::styled(format!("{fee_text}  "), Style::default().fg(Color::Cyan)),
                Span::styled("Enter", Style::default().fg(Color::Cyan)),
                Span::styled(" confirm  ", Style::default().fg(Color::DarkGray)),
                Span::styled("Esc", Style::default().fg(Color::Cyan)),
                Span::styled(esc_label, Style::default().fg(Color::DarkGray)),
            ]);
            frame.render_widget(Paragraph::new(vec![sep, preview_line, confirm_line]), area);
        }
        Some(Overlay::SenderPicker) => {
            let hints = render_hints(
                &[
                    ("\u{2191}\u{2193}", "navigate"),
                    ("Enter", "copy"),
                    ("Esc", "cancel"),
                ],
                usize::from(area.width),
            );
            frame.render_widget(Paragraph::new(vec![sep, Line::raw(""), hints]), area);
        }
        Some(Overlay::Help) => {
            let hints = render_hints(&[("any key", "close")], usize::from(area.width));
            frame.render_widget(Paragraph::new(vec![sep, Line::raw(""), hints]), area);
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
                    Span::styled("> ", Style::default().fg(Color::DarkGray)),
                    Span::styled(visible, Style::default().fg(Color::DarkGray)),
                    Span::styled(suffix, Style::default().fg(Color::Cyan)),
                ])
            } else {
                let w = usize::from(area.width);
                match app.view {
                    crate::app::View::Thread(_) | crate::app::View::Channel(_) => render_hints(
                        &[
                            ("i", "compose"),
                            ("/", "search"),
                            ("r", "refresh"),
                            ("u", "leave"),
                        ],
                        w,
                    ),
                    crate::app::View::ChannelDir => {
                        if !app.channel_dir_input.is_empty() {
                            let prompt = " ID: ";
                            let avail = w.saturating_sub(prompt.len() + 1);
                            let (text_spans, cursor_off) = visible_input(
                                &app.channel_dir_input,
                                app.channel_dir_input.len(),
                                avail,
                                None,
                            );
                            let mut spans =
                                vec![Span::styled(prompt, Style::default().fg(Color::DarkGray))];
                            spans.extend(text_spans);
                            let input_line = Line::from(spans);
                            let id_hints =
                                render_hints(&[("Enter", "subscribe"), ("Esc", "clear")], w);
                            frame.render_widget(
                                Paragraph::new(vec![sep, id_hints, input_line]),
                                area,
                            );
                            let cursor_x = area.x
                                + u16::try_from(prompt.len()).unwrap_or(u16::MAX)
                                + cursor_off;
                            let cursor_y = area.y + 2;
                            if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
                                frame.set_cursor_position((cursor_x, cursor_y));
                            }
                            return;
                        }
                        render_hints(
                            &[
                                ("\u{2191}\u{2193}", "navigate"),
                                ("Enter", "join/leave"),
                                ("c", "create"),
                                ("0-9", "enter ID"),
                                ("Esc", "back"),
                            ],
                            w,
                        )
                    }
                    _ => render_hints(
                        &[
                            ("m", "message"),
                            ("n", "thread"),
                            ("g", "group"),
                            ("c", "channels"),
                        ],
                        w,
                    ),
                }
            };
            frame.render_widget(Paragraph::new(vec![sep, Line::raw(""), input_line]), area);
        }
        Some(Overlay::CommandPalette) | Some(Overlay::FuzzyJump) => {
            let hints = render_hints(
                &[
                    ("\u{2191}\u{2193}", "navigate"),
                    ("Enter", "select"),
                    ("Esc", "cancel"),
                ],
                usize::from(area.width),
            );
            frame.render_widget(Paragraph::new(vec![sep, Line::raw(""), hints]), area);
        }
    }
}

fn render_picker_input(frame: &mut Frame, app: &App, sep: Line<'_>, area: Rect) {
    let w = usize::from(area.width);

    if app.input.is_empty() {
        let hints = render_hints(
            &[("j/k", "navigate"), ("Enter", "select"), ("Esc", "cancel")],
            w,
        );
        let prompt = Line::from(vec![
            Span::styled(" To: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "type to search or paste address",
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        frame.render_widget(Paragraph::new(vec![sep, hints, prompt]), area);
        let cursor_x = area.x + 5;
        let cursor_y = area.y + 2;
        if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    } else {
        let hints = render_hints(&[("Enter", "select"), ("Esc", "clear")], w);
        let avail = w.saturating_sub(6);
        let (text_spans, cursor_off) =
            visible_input(app.input.as_str(), app.input.cursor(), avail, None);
        let mut spans = vec![Span::styled(" To: ", Style::default().fg(Color::DarkGray))];
        spans.extend(text_spans);
        let input_line = Line::from(spans);
        frame.render_widget(Paragraph::new(vec![sep, hints, input_line]), area);
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
        let base_hints: Vec<(&str, &str)> = if selected_count >= 2 {
            vec![
                ("\u{2191}\u{2193}", "navigate"),
                ("Enter", "toggle"),
                ("Tab", "done"),
                ("Esc", "cancel"),
            ]
        } else {
            vec![
                ("\u{2191}\u{2193}", "navigate"),
                ("Enter", "toggle"),
                ("Esc", "cancel"),
            ]
        };
        let hints = render_hints(&base_hints, w);
        let prompt = Line::from(vec![
            Span::styled(
                format!(" Members ({selected_count}): "),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                "type to search or paste address",
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        frame.render_widget(Paragraph::new(vec![sep, hints, prompt]), area);
        let cursor_x = area.x
            + u16::try_from(format!(" Members ({selected_count}): ").len()).unwrap_or(u16::MAX);
        let cursor_y = area.y + 2;
        if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    } else {
        let hints = render_hints(&[("Enter", "add"), ("Esc", "clear")], w);
        let avail = w.saturating_sub(10);
        let prompt_str = format!(" ({selected_count}): ");
        let (text_spans, cursor_off) =
            visible_input(app.input.as_str(), app.input.cursor(), avail, None);
        let mut spans = vec![Span::styled(
            &prompt_str,
            Style::default().fg(Color::DarkGray),
        )];
        spans.extend(text_spans);
        let input_line = Line::from(spans);
        frame.render_widget(Paragraph::new(vec![sep, hints, input_line]), area);
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
