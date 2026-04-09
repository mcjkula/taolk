use crate::app::{App, Overlay, View};
use crate::ui::palette;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use taolk::conversation::InboxMessage;

const BASE58_CHARS: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

fn is_base58(b: u8) -> bool {
    BASE58_CHARS.contains(&b)
}

fn is_ss58_at(bytes: &[u8], pos: usize) -> bool {
    pos + 48 <= bytes.len()
        && bytes[pos] == b'5'
        && bytes[pos..pos + 48].iter().all(|b| is_base58(*b))
        && (pos + 48 == bytes.len() || !is_base58(bytes[pos + 48]))
}

fn render_body_line(text: &str, base_color: Color, my_ss58: &str) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let bytes = text.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        let at_boundary = pos == 0 || bytes[pos - 1].is_ascii_whitespace();

        if at_boundary {
            let remaining = &text[pos..];

            if remaining.starts_with("https://") || remaining.starts_with("http://") {
                let end = remaining
                    .find(|c: char| c.is_whitespace())
                    .unwrap_or(remaining.len());
                let url = &remaining[..end];
                let osc = format!("\x1b]8;;{url}\x07{url}\x1b]8;;\x07");
                spans.push(Span::styled(
                    osc,
                    Style::default()
                        .fg(palette::ACCENT_ALT)
                        .add_modifier(Modifier::UNDERLINED),
                ));
                pos += end;
                continue;
            }

            if bytes[pos] == b'@' && is_ss58_at(bytes, pos + 1) {
                let is_self = &text[pos + 1..pos + 49] == my_ss58;
                let color = if is_self {
                    palette::ACCENT
                } else {
                    palette::ACCENT_ALT
                };
                spans.push(Span::styled(
                    text[pos..pos + 49].to_string(),
                    Style::default()
                        .fg(color)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ));
                pos += 49;
                continue;
            }

            if is_ss58_at(bytes, pos) {
                spans.push(Span::styled(
                    text[pos..pos + 48].to_string(),
                    Style::default()
                        .fg(palette::MUTED)
                        .add_modifier(Modifier::UNDERLINED),
                ));
                pos += 48;
                continue;
            }

            if let Some(after_block) = remaining.strip_prefix("block:") {
                let digits = after_block
                    .bytes()
                    .take_while(|b| b.is_ascii_digit())
                    .count();
                if digits > 0 {
                    let end = 6 + digits;
                    spans.push(Span::styled(
                        remaining[..end].to_string(),
                        Style::default()
                            .fg(palette::ACCENT)
                            .add_modifier(Modifier::UNDERLINED),
                    ));
                    pos += end;
                    continue;
                }
            }

            if let Some(after_ext) = remaining.strip_prefix("ext:") {
                let d1 = after_ext.bytes().take_while(|b| b.is_ascii_digit()).count();
                if d1 > 0 && 4 + d1 < remaining.len() && remaining.as_bytes()[4 + d1] == b':' {
                    let d2 = remaining[4 + d1 + 1..]
                        .bytes()
                        .take_while(|b| b.is_ascii_digit())
                        .count();
                    if d2 > 0 {
                        let end = 4 + d1 + 1 + d2;
                        spans.push(Span::styled(
                            remaining[..end].to_string(),
                            Style::default()
                                .fg(palette::ACCENT)
                                .add_modifier(Modifier::UNDERLINED),
                        ));
                        pos += end;
                        continue;
                    }
                }
            }
        }

        let word_end = text[pos..]
            .find(|c: char| c.is_whitespace())
            .map(|p| pos + p)
            .unwrap_or(text.len());
        let span_end = text[word_end..]
            .find(|c: char| !c.is_whitespace())
            .map(|p| word_end + p)
            .unwrap_or(text.len());
        spans.push(Span::styled(
            text[pos..span_end].to_string(),
            Style::default().fg(base_color),
        ));
        pos = span_end;
    }

    if spans.is_empty() {
        Line::styled(text.to_string(), Style::default().fg(base_color))
    } else {
        Line::from(spans)
    }
}

fn wrap_text(text: &str, max_width: usize, indent: usize) -> Vec<String> {
    if max_width == 0 || text.len() <= max_width {
        return vec![text.to_string()];
    }

    let mut result = Vec::new();
    let mut remaining = text.to_string();
    let mut first = true;

    while !remaining.is_empty() {
        let width = if first {
            max_width
        } else {
            max_width.saturating_sub(indent)
        };
        first = false;

        if remaining.len() <= width + indent * usize::from(!result.is_empty()) {
            if result.is_empty() {
                result.push(remaining);
            } else {
                result.push(format!("{:indent$}{remaining}", ""));
            }
            break;
        }

        let effective = width;
        let search_area = &remaining[..effective.min(remaining.len())];
        let split_at = search_area
            .rfind(' ')
            .unwrap_or(effective.min(remaining.len()));

        if split_at == 0 {
            let brk = effective.min(remaining.len());
            if result.is_empty() {
                result.push(remaining[..brk].to_string());
            } else {
                result.push(format!("{:indent$}{}", "", &remaining[..brk]));
            }
            remaining = remaining[brk..].trim_start().to_string();
        } else {
            if result.is_empty() {
                result.push(remaining[..split_at].to_string());
            } else {
                result.push(format!("{:indent$}{}", "", &remaining[..split_at]));
            }
            remaining = remaining[split_at..].trim_start().to_string();
        }
    }

    if result.is_empty() {
        vec![text.to_string()]
    } else {
        result
    }
}

fn date_separator(date_str: &str) -> Line<'static> {
    let label = format!(" {date_str} ");
    let dashes = 3;
    let sep = format!(
        "{}\u{2500}{}{}\u{2500}{}",
        " ",
        "\u{2500}".repeat(dashes),
        label,
        "\u{2500}".repeat(dashes)
    );
    Line::styled(
        sep,
        Style::default()
            .fg(palette::MUTED)
            .add_modifier(Modifier::ITALIC),
    )
}

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    app.sender_click_regions.borrow_mut().clear();
    if app.overlay == Some(Overlay::Compose)
        || (app.overlay == Some(Overlay::Message) && app.msg_recipient.is_none())
    {
        render_contact_picker(frame, app, area);
        return;
    }
    if app.overlay == Some(Overlay::CreateGroupMembers) {
        render_group_member_picker(frame, app, area);
        return;
    }
    if app.overlay == Some(Overlay::SenderPicker) {
        render_sender_picker(frame, app, area);
        return;
    }

    match app.view {
        View::Inbox => {
            render_standalone(frame, app, &app.session.inbox, "Inbox", "From", None, area)
        }
        View::Outbox => render_standalone(
            frame,
            app,
            &app.session.outbox,
            "Sent",
            "To",
            pending_text(app, View::Outbox),
            area,
        ),
        View::Thread(i) => render_thread(frame, app, i, area),
        View::Channel(i) => render_channel(frame, app, i, area),
        View::Group(i) => render_group(frame, app, i, area),
        View::ChannelDir => render_channel_dir(frame, app, area),
    }
}

fn pending_text(app: &App, view: View) -> Option<&str> {
    if app.sending && app.pending_view == Some(view) {
        app.pending_text.as_deref()
    } else {
        None
    }
}

fn render_standalone(
    frame: &mut Frame,
    app: &App,
    messages: &[InboxMessage],
    title: &str,
    direction: &str,
    pending: Option<&str>,
    area: Rect,
) {
    let mut lines: Vec<Line> = vec![
        header_line(title, "", usize::from(area.width)),
        separator(area.width),
    ];

    {
        for msg in messages.iter().rev() {
            lines.push(Line::raw(""));
            let time = msg
                .timestamp
                .with_timezone(&chrono::Local)
                .format(&app.date_format)
                .to_string();
            let (type_icon, type_label, badge_bg) = if msg.content_type == 0x00 {
                (super::icons::PUBLIC, "public", Color::Cyan)
            } else {
                (super::icons::ENCRYPTED, "encrypted", Color::Magenta)
            };

            lines.push(Line::from(vec![
                Span::styled(format!(" {time} "), Style::default().fg(palette::MUTED)),
                Span::styled(
                    format!(" {type_icon} {type_label} "),
                    Style::default()
                        .fg(Color::White)
                        .bg(badge_bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("{direction}: "),
                    Style::default().fg(palette::MUTED),
                ),
                Span::styled(
                    truncate(&msg.peer_ss58, 20),
                    Style::default()
                        .fg(Color::Reset)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));

            if msg.body.is_empty() {
                lines.push(Line::from(vec![Span::styled(
                    "   empty",
                    Style::default()
                        .fg(palette::MUTED)
                        .add_modifier(Modifier::ITALIC),
                )]));
            } else {
                let my_ss58 = app.session.ss58();
                for text_line in msg.body.lines() {
                    lines.push(render_body_line(
                        &format!("   {text_line}"),
                        Color::Reset,
                        my_ss58,
                    ));
                }
            }
        }
    }

    if let Some(text) = pending {
        let spinner = app.spinner_16();
        let (type_icon, type_label) = if app.msg_type == Some(0x01) {
            (super::icons::PUBLIC, "public")
        } else {
            (super::icons::ENCRYPTED, "encrypted")
        };
        let recipient_label = app
            .msg_recipient
            .as_ref()
            .map(|(_, ss58)| ss58.clone())
            .unwrap_or_default();

        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled(format!(" {spinner}  "), Style::default().fg(palette::MUTED)),
            Span::styled(
                format!(" {type_icon} {type_label} "),
                Style::default()
                    .fg(palette::MUTED)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled("To: ", Style::default().fg(palette::MUTED)),
            Span::styled(
                truncate(&recipient_label, 20),
                Style::default()
                    .fg(palette::MUTED)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        for text_line in text.lines() {
            lines.push(Line::styled(
                format!("   {text_line}"),
                Style::default().fg(palette::MUTED),
            ));
        }
    }

    render_scrolled(frame, lines, 0, area);
}

fn title_header_line(
    title: String,
    title_color: Color,
    id_str: String,
    width: usize,
) -> Line<'static> {
    let id_reserve = if id_str.is_empty() {
        0
    } else {
        id_str.len() + 1
    };
    let title_display_width = title.chars().count();
    let left_used = 1 + title_display_width;
    let pad = width.saturating_sub(left_used + id_reserve);
    Line::from(vec![
        Span::styled(
            format!(" {title}"),
            Style::default()
                .fg(title_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(pad)),
        Span::styled(id_str, Style::default().fg(palette::MUTED)),
    ])
}

fn render_threaded(
    frame: &mut Frame,
    app: &App,
    mut lines: Vec<Line<'static>>,
    messages: &[crate::conversation::ThreadMessage],
    view: View,
    area: Rect,
) {
    let mut pending_clicks = Vec::new();
    render_messages(
        &mut lines,
        messages,
        app,
        usize::from(area.width),
        &mut pending_clicks,
    );
    render_pending(&mut lines, app, view);
    record_sender_clicks(app, &pending_clicks, lines.len(), app.scroll_offset, area);
    render_scrolled(frame, lines, app.scroll_offset, area);
}

fn render_thread(frame: &mut Frame, app: &App, thread_idx: usize, area: Rect) {
    let thread = match app.session.threads.get(thread_idx) {
        Some(t) => t,
        None => return,
    };

    let w = usize::from(area.width);
    let id_str = if thread.thread_ref.is_zero() {
        String::new()
    } else {
        format!(
            "{}:{}",
            thread.thread_ref.block().get(),
            thread.thread_ref.index().get()
        )
    };
    let id_reserve = if id_str.is_empty() {
        0
    } else {
        id_str.len() + 1
    };
    let peer = truncate(&thread.peer_ss58, w.saturating_sub(2 + id_reserve)).to_string();

    let lines = vec![
        title_header_line(peer, Color::Reset, id_str, w),
        separator(area.width),
    ];

    render_threaded(
        frame,
        app,
        lines,
        &thread.messages,
        View::Thread(thread_idx),
        area,
    );
}

fn render_channel(frame: &mut Frame, app: &App, chan_idx: usize, area: Rect) {
    let channel = match app.session.channels.get(chan_idx) {
        Some(c) => c,
        None => return,
    };

    let w = usize::from(area.width);
    let id_str = format!(
        "{}:{}",
        channel.channel_ref.block().get(),
        channel.channel_ref.index().get()
    );
    let id_reserve = id_str.len() + 1;
    let title = format!(
        "#{}",
        truncate(&channel.name, w.saturating_sub(2 + id_reserve))
    );

    let mut lines = vec![title_header_line(title, Color::Reset, id_str, w)];

    if !channel.description.is_empty() {
        let desc_max = (usize::from(area.width)).saturating_sub(3);
        lines.push(Line::from(vec![Span::styled(
            format!(" {} ", truncate(&channel.description, desc_max)),
            Style::default().fg(palette::MUTED),
        )]));
    }

    lines.push(separator(area.width));

    render_threaded(
        frame,
        app,
        lines,
        &channel.messages,
        View::Channel(chan_idx),
        area,
    );
}

fn render_group(frame: &mut Frame, app: &App, group_idx: usize, area: Rect) {
    let group = match app.session.groups.get(group_idx) {
        Some(g) => g,
        None => return,
    };

    let w = usize::from(area.width);
    let id_str = if group.group_ref.is_zero() {
        String::new()
    } else {
        format!(
            "{}:{}",
            group.group_ref.block().get(),
            group.group_ref.index().get()
        )
    };
    let id_reserve = if id_str.is_empty() {
        0
    } else {
        id_str.len() + 1
    };
    let title = group_member_title(group, app, w.saturating_sub(2 + id_reserve));

    let lines = vec![
        title_header_line(title, palette::ACCENT, id_str, w),
        separator(area.width),
    ];

    render_threaded(
        frame,
        app,
        lines,
        &group.messages,
        View::Group(group_idx),
        area,
    );
}

fn group_member_title(group: &crate::conversation::Group, app: &App, title_max: usize) -> String {
    let mut member_order: Vec<usize> = (0..group.members.len()).collect();
    if let Some(pos) = member_order
        .iter()
        .position(|&i| group.members[i] == group.creator_pubkey)
    {
        let c = member_order.remove(pos);
        member_order.insert(0, c);
    }
    let mut title = String::new();
    for (i, &mi) in member_order.iter().enumerate() {
        let pk = &group.members[mi];
        let label = if *pk == app.session.pubkey() {
            "You".to_string()
        } else {
            taolk::util::ss58_short(pk)
        };
        let label = if *pk == group.creator_pubkey {
            format!("{label}{}", super::icons::CREATOR)
        } else {
            label
        };
        let remaining = member_order.len() - i - 1;
        let suffix_len = if remaining > 0 {
            format!("+{remaining}").len()
        } else {
            0
        };
        let sep = if title.is_empty() { "" } else { "," };
        let candidate_width =
            title.chars().count() + sep.len() + label.chars().count() + suffix_len;
        if candidate_width > title_max && i > 0 {
            title = format!("{title}+{}", remaining + 1);
            break;
        }
        if !title.is_empty() {
            title.push(',');
        }
        title.push_str(&label);
    }
    title
}

fn render_channel_dir(frame: &mut Frame, app: &App, area: Rect) {
    let count = app.session.known_channels.len();
    let mut lines: Vec<Line> = vec![
        header_line(
            "Channels",
            &format!("{count} available"),
            usize::from(area.width),
        ),
        separator(area.width),
    ];

    if app.session.known_channels.is_empty() && app.channel_dir_input.is_empty() {
        lines.push(Line::raw(""));
        lines.push(dim("  No channels discovered yet"));
        lines.push(dim("  Channels appear here as they are created on-chain"));
    }

    for (i, info) in app.session.known_channels.iter().enumerate() {
        let selected = i == app.channel_dir_cursor && app.channel_dir_input.is_empty();
        let subscribed = app.session.is_subscribed(&info.channel_ref);

        let indicator = if selected { "> " } else { "  " };
        let check = if subscribed { " \u{F012C}" } else { "" };
        let id_str = format!(
            " {}:{}",
            info.channel_ref.block().get(),
            info.channel_ref.index().get()
        );
        let name_str = format!("#{}", info.name);

        let name_color = if subscribed {
            palette::MUTED
        } else if selected {
            Color::Reset
        } else {
            palette::ACCENT
        };
        let name_mod = if selected {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };

        lines.push(Line::from(vec![
            Span::styled(indicator, Style::default().fg(palette::ACCENT)),
            Span::styled("  ", Style::default().fg(palette::MUTED)),
            Span::styled(
                name_str,
                Style::default().fg(name_color).add_modifier(name_mod),
            ),
            Span::styled(id_str, Style::default().fg(palette::MUTED)),
            Span::styled(check, Style::default().fg(palette::SUCCESS)),
        ]));

        if !info.description.is_empty() {
            let desc_max = (usize::from(area.width)).saturating_sub(5);
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    truncate(&info.description, desc_max),
                    Style::default().fg(palette::MUTED),
                ),
            ]));
        }
    }

    render_scrolled(frame, lines, app.scroll_offset, area);
}

fn render_contact_picker(frame: &mut Frame, app: &App, area: Rect) {
    let contacts = app.filtered_contacts();
    let total = app.session.known_contacts().len();
    let w = usize::from(area.width);

    let mut lines: Vec<Line> = vec![
        header_line("Contacts", &format!("{total} known"), w),
        separator(area.width),
    ];

    if total == 0 {
        lines.push(Line::raw(""));
        lines.push(dim("  No known contacts yet"));
        lines.push(dim("  Paste an SS58 address below to message someone"));
    } else if contacts.is_empty() && !app.input.is_empty() {
        lines.push(Line::raw(""));
        lines.push(dim("  No matches"));
    }

    for (i, (_, pubkey)) in contacts.iter().enumerate() {
        let selected = i == app.contact_idx % contacts.len().max(1) && app.input.is_empty();
        let full_addr = taolk::util::ss58_from_pubkey(pubkey);
        let addr_max = w.saturating_sub(4);

        let indicator = if selected { "> " } else { "  " };
        let addr_color = if selected {
            Color::Reset
        } else {
            palette::ACCENT
        };
        let addr_mod = if selected {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };

        lines.push(Line::from(vec![
            Span::styled(indicator, Style::default().fg(palette::ACCENT)),
            Span::styled(
                truncate(&full_addr, addr_max),
                Style::default().fg(addr_color).add_modifier(addr_mod),
            ),
        ]));
    }

    render_scrolled(frame, lines, app.scroll_offset, area);
}

fn render_sender_picker(frame: &mut Frame, app: &App, area: Rect) {
    let senders = &app.picker_senders;
    let total = senders.len();
    let w = usize::from(area.width);

    let mut lines: Vec<Line> = vec![
        header_line("Copy SS58", &format!("{total} senders"), w),
        separator(area.width),
    ];

    if total == 0 {
        lines.push(Line::raw(""));
        lines.push(dim("  No senders in this view"));
    }

    for (i, (short, pk)) in senders.iter().enumerate() {
        let selected = i == app.contact_idx % total.max(1);
        let display = match pk {
            Some(pk) => taolk::util::ss58_from_pubkey(pk),
            None => format!("{short}  (full SS58 unavailable)"),
        };
        let addr_max = w.saturating_sub(4);
        let indicator = if selected { "> " } else { "  " };
        let addr_color = if selected {
            Color::Reset
        } else if pk.is_some() {
            palette::ACCENT
        } else {
            palette::MUTED
        };
        let addr_mod = if selected {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };

        lines.push(Line::from(vec![
            Span::styled(indicator, Style::default().fg(palette::ACCENT)),
            Span::styled(
                truncate(&display, addr_max),
                Style::default().fg(addr_color).add_modifier(addr_mod),
            ),
        ]));
    }

    render_scrolled(frame, lines, 0, area);
}

fn render_group_member_picker(frame: &mut Frame, app: &App, area: Rect) {
    let contacts = app.filtered_contacts();
    let total = app.session.known_contacts().len();
    let selected_count = app.pending_group_members.len();
    let w = usize::from(area.width);

    let mut lines: Vec<Line> = vec![
        header_line(
            "Select Members",
            &format!("{selected_count} selected, {total} known"),
            w,
        ),
        separator(area.width),
    ];

    if total == 0 {
        lines.push(Line::raw(""));
        lines.push(dim("  No known contacts yet"));
    } else if contacts.is_empty() && !app.input.is_empty() {
        lines.push(Line::raw(""));
        lines.push(dim("  No matches"));
    }

    for (i, (_, pubkey)) in contacts.iter().enumerate() {
        let cursor = i == app.contact_idx % contacts.len().max(1) && app.input.is_empty();
        let is_member = app.pending_group_members.iter().any(|(pk, _)| pk == pubkey);
        let is_self = *pubkey == app.session.pubkey();
        let full_addr = taolk::util::ss58_from_pubkey(pubkey);
        let addr_max = w.saturating_sub(6);

        let indicator = if cursor { "> " } else { "  " };
        let check = if is_member { "\u{F012C} " } else { "  " };
        let addr_color = if is_self {
            palette::MUTED
        } else if cursor {
            Color::Reset
        } else if is_member {
            palette::SUCCESS
        } else {
            Color::Reset
        };
        let addr_mod = if cursor {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };

        lines.push(Line::from(vec![
            Span::styled(indicator, Style::default().fg(palette::ACCENT)),
            Span::styled(check, Style::default().fg(palette::SUCCESS)),
            Span::styled(
                truncate(&full_addr, addr_max),
                Style::default().fg(addr_color).add_modifier(addr_mod),
            ),
        ]));
    }

    render_scrolled(frame, lines, app.scroll_offset, area);
}

fn render_pending(lines: &mut Vec<Line<'static>>, app: &App, view: View) {
    if app.sending
        && app.pending_view == Some(view)
        && let Some(text) = &app.pending_text
    {
        let first_line = text.lines().next().unwrap_or("");
        let indent = 7 + 3 + 2;
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} ", app.spinner_5()),
                Style::default().fg(palette::MUTED),
            ),
            Span::styled(
                "You  ",
                Style::default()
                    .fg(palette::MUTED)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(first_line.to_string(), Style::default().fg(palette::MUTED)),
        ]));
        for body_line in text.lines().skip(1) {
            lines.push(Line::styled(
                format!("{:indent$}{body_line}", ""),
                Style::default().fg(palette::MUTED),
            ));
        }
    }
}

fn render_messages(
    lines: &mut Vec<Line<'static>>,
    messages: &[taolk::conversation::ThreadMessage],
    app: &App,
    width: usize,
    pending_clicks: &mut Vec<(usize, u16, u16, String)>,
) {
    let my_ss58 = app.session.ss58();
    let mut last_date: Option<chrono::NaiveDate> = None;
    let mut last_sender: Option<&str> = None;
    let mut last_indent: usize = 7;

    for (i, msg) in messages.iter().enumerate() {
        let msg_date = msg.timestamp.with_timezone(&chrono::Local).date_naive();
        if last_date.is_some() && last_date != Some(msg_date) {
            let date_str = msg_date.format("%B %-d, %Y").to_string();
            lines.push(date_separator(&date_str));
            last_sender = None;
        }
        last_date = Some(msg_date);

        if msg.has_gap {
            let pulse = if app.frame % 8 < 4 {
                palette::MUTED
            } else {
                palette::WARNING
            };
            let gap_str = format!(
                " {} Earlier messages may be missing \u{00B7} press r to load",
                super::icons::HISTORY
            );
            let gap_text = truncate(&gap_str, width);
            lines.push(Line::from(vec![Span::styled(
                gap_text,
                Style::default().fg(pulse),
            )]));
            last_sender = None;
        }

        let search = &app.search_query;
        let is_empty_body = msg.body.is_empty();
        let has_match =
            !search.is_empty() && msg.body.to_lowercase().contains(&search.to_lowercase());
        let body_color = if has_match {
            palette::ACCENT
        } else {
            Color::Reset
        };
        let display_body = if is_empty_body { "empty" } else { &msg.body };
        let first_body_line = display_body.lines().next().unwrap_or("");

        let same_sender = last_sender == Some(&msg.sender_ss58);
        let time_gap = if i > 0 {
            (msg.timestamp - messages[i - 1].timestamp)
                .num_minutes()
                .abs()
                > 5
        } else {
            false
        };

        let body_color = if is_empty_body {
            palette::MUTED
        } else {
            body_color
        };

        if is_empty_body && same_sender && !time_gap {
            lines.push(Line::from(vec![Span::styled(
                format!("{:last_indent$}empty", ""),
                Style::default()
                    .fg(palette::MUTED)
                    .add_modifier(Modifier::ITALIC),
            )]));
        } else if is_empty_body {
            let time = msg
                .timestamp
                .with_timezone(&chrono::Local)
                .format(&app.timestamp_format)
                .to_string();
            let (name, name_color) = if msg.is_mine {
                ("You".to_string(), palette::ACCENT)
            } else {
                (
                    truncate(&msg.sender_ss58, 16),
                    palette::sender_color(&msg.sender_ss58),
                )
            };
            let indent = 7 + name.len() + 2;
            last_indent = indent;
            if !msg.is_mine {
                pending_clicks.push((
                    lines.len(),
                    7,
                    7 + u16::try_from(name.len()).unwrap_or(u16::MAX),
                    msg.sender_ss58.clone(),
                ));
            }
            lines.push(Line::from(vec![
                Span::styled(format!(" {time} "), Style::default().fg(palette::MUTED)),
                Span::styled(
                    format!("{name}  "),
                    Style::default().fg(name_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "empty",
                    Style::default()
                        .fg(palette::MUTED)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
        } else if same_sender && !time_gap {
            for body_line in display_body.lines() {
                let prefixed = format!("{:last_indent$}{body_line}", "");
                for wrapped in wrap_text(&prefixed, width, last_indent) {
                    lines.push(render_body_line(&wrapped, body_color, my_ss58));
                }
            }
        } else if same_sender && time_gap {
            let time = msg
                .timestamp
                .with_timezone(&chrono::Local)
                .format(&app.timestamp_format)
                .to_string();
            let body_avail = width.saturating_sub(last_indent);
            let first_wrapped = if first_body_line.len() > body_avail && body_avail > 0 {
                let split = first_body_line[..body_avail]
                    .rfind(' ')
                    .unwrap_or(body_avail);
                let (a, b) = first_body_line.split_at(split);
                (a.to_string(), Some(b.trim_start().to_string()))
            } else {
                (first_body_line.to_string(), None)
            };

            let pad = last_indent.saturating_sub(7);
            let mut spans = vec![
                Span::styled(format!(" {time} "), Style::default().fg(palette::MUTED)),
                Span::styled(format!("{:pad$}", ""), Style::default()),
            ];
            let rendered = render_body_line(&first_wrapped.0, body_color, my_ss58);
            for span in rendered.spans {
                spans.push(span);
            }
            lines.push(Line::from(spans));

            if let Some(overflow) = first_wrapped.1 {
                for wrapped in wrap_text(
                    &format!("{:last_indent$}{overflow}", ""),
                    width,
                    last_indent,
                ) {
                    lines.push(render_body_line(&wrapped, body_color, my_ss58));
                }
            }
            for body_line in msg.body.lines().skip(1) {
                let prefixed = format!("{:last_indent$}{body_line}", "");
                for wrapped in wrap_text(&prefixed, width, last_indent) {
                    lines.push(render_body_line(&wrapped, body_color, my_ss58));
                }
            }
        } else {
            if i > 0 {
                lines.push(Line::raw(""));
            }

            let time = msg
                .timestamp
                .with_timezone(&chrono::Local)
                .format(&app.timestamp_format)
                .to_string();
            let (name, name_color) = if msg.is_mine {
                ("You".to_string(), palette::ACCENT)
            } else {
                (
                    truncate(&msg.sender_ss58, 16),
                    palette::sender_color(&msg.sender_ss58),
                )
            };

            let indent = 7 + name.len() + 2;
            last_indent = indent;

            let header_width = indent;
            let body_avail = width.saturating_sub(header_width);
            let first_wrapped = if first_body_line.len() > body_avail && body_avail > 0 {
                let split = first_body_line[..body_avail]
                    .rfind(' ')
                    .unwrap_or(body_avail);
                let (a, b) = first_body_line.split_at(split);
                (a.to_string(), Some(b.trim_start().to_string()))
            } else {
                (first_body_line.to_string(), None)
            };

            if !msg.is_mine {
                pending_clicks.push((
                    lines.len(),
                    7,
                    7 + u16::try_from(name.len()).unwrap_or(u16::MAX),
                    msg.sender_ss58.clone(),
                ));
            }
            let mut first_spans = vec![
                Span::styled(format!(" {time} "), Style::default().fg(palette::MUTED)),
                Span::styled(
                    format!("{name}  "),
                    Style::default().fg(name_color).add_modifier(Modifier::BOLD),
                ),
            ];
            let body_rendered = render_body_line(&first_wrapped.0, body_color, my_ss58);
            for span in body_rendered.spans {
                first_spans.push(span);
            }
            lines.push(Line::from(first_spans));

            if let Some(overflow) = first_wrapped.1 {
                for wrapped in wrap_text(&format!("{:indent$}{overflow}", ""), width, indent) {
                    lines.push(render_body_line(&wrapped, body_color, my_ss58));
                }
            }

            for body_line in msg.body.lines().skip(1) {
                let prefixed = format!("{:indent$}{body_line}", "");
                for wrapped in wrap_text(&prefixed, width, indent) {
                    lines.push(render_body_line(&wrapped, body_color, my_ss58));
                }
            }
        }

        last_sender = Some(&msg.sender_ss58);
    }
}

fn header_line(title: &str, right: &str, width: usize) -> Line<'static> {
    let title_max = width.saturating_sub(right.len() + 3);
    let truncated = truncate(title, title_max);
    let pad = width.saturating_sub(truncated.len() + 2 + right.len());
    Line::from(vec![
        Span::styled(
            format!(" {truncated}"),
            Style::default()
                .fg(Color::Reset)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(pad)),
        Span::styled(right.to_string(), Style::default().fg(palette::MUTED)),
    ])
}

fn separator(width: u16) -> Line<'static> {
    Line::styled(
        "\u{2500}".repeat(usize::from(width)),
        Style::default().fg(palette::MUTED),
    )
}

fn dim(text: &str) -> Line<'static> {
    Line::styled(text.to_string(), Style::default().fg(palette::MUTED))
}

fn render_scrolled(frame: &mut ratatui::Frame, lines: Vec<Line<'_>>, scroll: usize, area: Rect) {
    let visible = usize::from(area.height);
    let bottom = lines.len().saturating_sub(visible);
    let start = bottom.saturating_sub(scroll);
    let end = (start + visible).min(lines.len());
    frame.render_widget(Paragraph::new(lines[start..end].to_vec()), area);
}

fn record_sender_clicks(
    app: &App,
    pending: &[(usize, u16, u16, String)],
    lines_len: usize,
    scroll: usize,
    area: Rect,
) {
    let visible = usize::from(area.height);
    let bottom = lines_len.saturating_sub(visible);
    let start = bottom.saturating_sub(scroll);
    let end = (start + visible).min(lines_len);
    let mut regions = app.sender_click_regions.borrow_mut();
    for (line_idx, c0, c1, ss58) in pending {
        if *line_idx >= start && *line_idx < end {
            let row = area.y + u16::try_from(*line_idx - start).unwrap_or(u16::MAX);
            regions.push((row, area.x + *c0, area.x + *c1, ss58.clone()));
        }
    }
}

use taolk::util::truncate;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_base58_accepts_alphabet() {
        assert!(is_base58(b'1'));
        assert!(is_base58(b'A'));
        assert!(is_base58(b'z'));
    }

    #[test]
    fn is_base58_rejects_zero_and_capital_o() {
        assert!(!is_base58(b'0'));
        assert!(!is_base58(b'O'));
        assert!(!is_base58(b'I'));
        assert!(!is_base58(b'l'));
    }

    #[test]
    fn is_ss58_at_finds_at_zero() {
        let ss58 = "5FHneW46xGXgs5AUiveU4sbTyGBzmstUspZC92UhjJM694ty";
        assert_eq!(ss58.len(), 48);
        assert!(is_ss58_at(ss58.as_bytes(), 0));
    }

    #[test]
    fn is_ss58_at_rejects_non_5_prefix() {
        let bad = "1FHneW46xGXgs5AUiveU4sbTyGBzmstUspZC92UhjJM694ty";
        assert!(!is_ss58_at(bad.as_bytes(), 0));
    }

    #[test]
    fn is_ss58_at_rejects_short_buffer() {
        assert!(!is_ss58_at(b"5short", 0));
    }

    #[test]
    fn wrap_text_short_returns_single_line() {
        let lines = wrap_text("hello", 80, 0);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "hello");
    }

    #[test]
    fn wrap_text_long_text_wraps_at_word_boundary() {
        let lines = wrap_text("the quick brown fox", 10, 0);
        assert!(lines.len() >= 2);
        for line in &lines {
            assert!(line.len() <= 12, "line too long: {line:?}");
        }
    }

    #[test]
    fn wrap_text_no_spaces_hard_breaks() {
        let lines = wrap_text("aaaaaaaaaaaaaaaa", 5, 0);
        assert!(lines.len() >= 2);
    }

    #[test]
    fn wrap_text_continuation_lines_are_indented() {
        let lines = wrap_text("the quick brown fox jumps over", 10, 4);
        assert!(lines.len() >= 2);
        for line in lines.iter().skip(1) {
            assert!(line.starts_with("    "), "expected indent on: {line:?}");
        }
    }

    #[test]
    fn wrap_text_empty_returns_single_empty() {
        let lines = wrap_text("", 80, 0);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "");
    }
}
