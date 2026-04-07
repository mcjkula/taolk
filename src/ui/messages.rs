use crate::app::{App, View};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use taolk::conversation::InboxMessage;

// Deterministic sender colors derived from SS58 address.
// Avoids Cyan (reserved for "You"), Red (errors), DarkGray (metadata).
const SENDER_COLORS: &[Color] = &[
    Color::Yellow,
    Color::Green,
    Color::Magenta,
    Color::Blue,
    Color::LightYellow,
    Color::LightGreen,
    Color::LightMagenta,
    Color::LightBlue,
];

fn sender_color(ss58: &str) -> Color {
    let hash: u8 = ss58.bytes().fold(0u8, |acc, b| acc.wrapping_add(b));
    SENDER_COLORS[hash as usize % SENDER_COLORS.len()]
}

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

            // URL: https:// or http:// -- clickable via OSC 8 hyperlink
            if remaining.starts_with("https://") || remaining.starts_with("http://") {
                let end = remaining
                    .find(|c: char| c.is_whitespace())
                    .unwrap_or(remaining.len());
                let url = &remaining[..end];
                let osc = format!("\x1b]8;;{url}\x07{url}\x1b]8;;\x07");
                spans.push(Span::styled(
                    osc,
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::UNDERLINED),
                ));
                pos += end;
                continue;
            }

            // @-mention
            if bytes[pos] == b'@' && is_ss58_at(bytes, pos + 1) {
                let is_self = &text[pos + 1..pos + 49] == my_ss58;
                let color = if is_self { Color::Cyan } else { Color::Yellow };
                spans.push(Span::styled(
                    text[pos..pos + 49].to_string(),
                    Style::default()
                        .fg(color)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ));
                pos += 49;
                continue;
            }

            // Bare SS58
            if is_ss58_at(bytes, pos) {
                spans.push(Span::styled(
                    text[pos..pos + 48].to_string(),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::UNDERLINED),
                ));
                pos += 48;
                continue;
            }

            // Block: block:\d+
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
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::UNDERLINED),
                    ));
                    pos += end;
                    continue;
                }
            }

            // Extrinsic: ext:\d+:\d+
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
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::UNDERLINED),
                        ));
                        pos += end;
                        continue;
                    }
                }
            }
        }

        // No pattern matched -- consume until next whitespace
        let word_end = text[pos..]
            .find(|c: char| c.is_whitespace())
            .map(|p| pos + p)
            .unwrap_or(text.len());
        // Include trailing whitespace in the plain span
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

/// Word-wrap a line at the given max width, returning wrapped segments.
/// First line uses full width; continuation lines are indented by `indent`.
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

        if remaining.len() <= width + indent * (!result.is_empty() as usize) {
            if result.is_empty() {
                result.push(remaining);
            } else {
                result.push(format!("{:indent$}{remaining}", ""));
            }
            break;
        }

        // Find last space before width limit for word boundary
        let effective = width;
        let search_area = &remaining[..effective.min(remaining.len())];
        let split_at = search_area
            .rfind(' ')
            .unwrap_or(effective.min(remaining.len()));

        if split_at == 0 {
            // No space found, hard break
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

/// Format a centered date separator: ─────── April 4, 2026 ───────
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
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    )
}

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    use crate::app::Mode;
    // Contact picker takes over the main panel during address selection
    if app.mode == Mode::Compose || (app.mode == Mode::Message && app.msg_recipient.is_none()) {
        render_contact_picker(frame, app, area);
        return;
    }
    if app.mode == Mode::CreateGroupMembers {
        render_group_member_picker(frame, app, area);
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
        header_line(title, "", area.width as usize),
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
                Span::styled(format!(" {time} "), Style::default().fg(Color::DarkGray)),
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
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    truncate(&msg.peer_ss58, 20),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));

            if msg.body.is_empty() {
                lines.push(Line::from(vec![Span::styled(
                    "   empty",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                )]));
            } else {
                let my_ss58 = app.session.ss58();
                for text_line in msg.body.lines() {
                    lines.push(render_body_line(
                        &format!("   {text_line}"),
                        Color::White,
                        my_ss58,
                    ));
                }
            }
        }
    }

    if let Some(text) = pending {
        let spinner = app.spinner_16();
        // Match confirmed layout: "YYYY-MM-DD HH:MM" = 16 chars
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
            Span::styled(
                format!(" {spinner}  "),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!(" {type_icon} {type_label} "),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled("To: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                truncate(&recipient_label, 20),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        for text_line in text.lines() {
            lines.push(Line::styled(
                format!("   {text_line}"),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    render_scrolled(frame, lines, 0, area);
}

fn render_thread(frame: &mut Frame, app: &App, thread_idx: usize, area: Rect) {
    let thread = match app.session.threads.get(thread_idx) {
        Some(t) => t,
        None => return,
    };

    let w = area.width as usize;
    let th_block = thread.thread_ref.block;
    let th_index = thread.thread_ref.index;
    let id_str = if thread.thread_ref.is_zero() {
        String::new()
    } else {
        format!("{}:{}", th_block, th_index)
    };
    let id_reserve = if id_str.is_empty() {
        0
    } else {
        id_str.len() + 1
    };
    let name_max = w.saturating_sub(2 + id_reserve);
    let peer = truncate(&thread.peer_ss58, name_max);
    let left_used = 1 + peer.len();
    let pad = w.saturating_sub(left_used + id_reserve);
    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled(
                format!(" {peer}"),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(pad)),
            Span::styled(id_str, Style::default().fg(Color::DarkGray)),
        ]),
        separator(area.width),
    ];

    render_messages(&mut lines, &thread.messages, app, area.width as usize);
    render_pending(&mut lines, app, View::Thread(thread_idx));
    render_scrolled(frame, lines, app.scroll_offset, area);
}

fn render_channel(frame: &mut Frame, app: &App, chan_idx: usize, area: Rect) {
    let channel = match app.session.channels.get(chan_idx) {
        Some(c) => c,
        None => return,
    };

    let w = area.width as usize;
    let ref_block = channel.channel_ref.block;
    let ref_index = channel.channel_ref.index;
    let id_str = format!("{}:{}", ref_block, ref_index);
    let id_reserve = id_str.len() + 1;
    let name_max = w.saturating_sub(2 + id_reserve);
    let title = format!("#{}", truncate(&channel.name, name_max));
    let left_used = 1 + title.len();
    let pad = w.saturating_sub(left_used + id_reserve);

    let mut lines: Vec<Line> = vec![Line::from(vec![
        Span::styled(
            format!(" {title}"),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(pad)),
        Span::styled(id_str, Style::default().fg(Color::DarkGray)),
    ])];

    if !channel.description.is_empty() {
        let desc_max = (area.width as usize).saturating_sub(3);
        lines.push(Line::from(vec![Span::styled(
            format!(" {} ", truncate(&channel.description, desc_max)),
            Style::default().fg(Color::DarkGray),
        )]));
    }

    lines.push(separator(area.width));

    render_messages(&mut lines, &channel.messages, app, area.width as usize);
    render_pending(&mut lines, app, View::Channel(chan_idx));
    render_scrolled(frame, lines, app.scroll_offset, area);
}

fn render_group(frame: &mut Frame, app: &App, group_idx: usize, area: Rect) {
    let group = match app.session.groups.get(group_idx) {
        Some(g) => g,
        None => return,
    };

    let w = area.width as usize;
    let ref_block = group.group_ref.block;
    let ref_index = group.group_ref.index;
    let id_str = if group.group_ref.is_zero() {
        String::new()
    } else {
        format!("{}:{}", ref_block, ref_index)
    };
    // Sort member indices: creator first, then rest in order
    let mut member_order: Vec<usize> = (0..group.members.len()).collect();
    if let Some(pos) = member_order
        .iter()
        .position(|&i| group.members[i] == group.creator_pubkey)
    {
        let c = member_order.remove(pos);
        member_order.insert(0, c);
    }
    let id_reserve = if id_str.is_empty() {
        0
    } else {
        id_str.len() + 1
    };
    let title_max = w.saturating_sub(2 + id_reserve);
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
    let title_display_width = title.chars().count();
    let left_used = 1 + title_display_width;
    let pad = w.saturating_sub(left_used + id_reserve);

    let mut lines: Vec<Line> = vec![Line::from(vec![
        Span::styled(
            format!(" {title}"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(pad)),
        Span::styled(id_str, Style::default().fg(Color::DarkGray)),
    ])];

    lines.push(separator(area.width));

    render_messages(&mut lines, &group.messages, app, area.width as usize);
    render_pending(&mut lines, app, View::Group(group_idx));
    render_scrolled(frame, lines, app.scroll_offset, area);
}

fn render_channel_dir(frame: &mut Frame, app: &App, area: Rect) {
    let count = app.session.known_channels.len();
    let mut lines: Vec<Line> = vec![
        header_line(
            "Channels",
            &format!("{count} available"),
            area.width as usize,
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
        let check = if subscribed { " \u{2713}" } else { "" };
        let id_str = format!(" {}:{}", info.channel_ref.block, info.channel_ref.index);
        let name_str = format!("#{}", info.name);

        let name_color = if subscribed {
            Color::DarkGray
        } else if selected {
            Color::White
        } else {
            Color::Cyan
        };
        let name_mod = if selected {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };

        lines.push(Line::from(vec![
            Span::styled(indicator, Style::default().fg(Color::Cyan)),
            Span::styled("  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                name_str,
                Style::default().fg(name_color).add_modifier(name_mod),
            ),
            Span::styled(id_str, Style::default().fg(Color::DarkGray)),
            Span::styled(check, Style::default().fg(Color::Green)),
        ]));

        if !info.description.is_empty() {
            let desc_max = (area.width as usize).saturating_sub(5);
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    truncate(&info.description, desc_max),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    render_scrolled(frame, lines, app.scroll_offset, area);
}

fn render_contact_picker(frame: &mut Frame, app: &App, area: Rect) {
    let contacts = app.filtered_contacts();
    let total = app.session.known_contacts().len();
    let w = area.width as usize;

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
        let addr_color = if selected { Color::White } else { Color::Cyan };
        let addr_mod = if selected {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };

        lines.push(Line::from(vec![
            Span::styled(indicator, Style::default().fg(Color::Cyan)),
            Span::styled(
                truncate(&full_addr, addr_max),
                Style::default().fg(addr_color).add_modifier(addr_mod),
            ),
        ]));
    }

    render_scrolled(frame, lines, app.scroll_offset, area);
}

fn render_group_member_picker(frame: &mut Frame, app: &App, area: Rect) {
    let contacts = app.filtered_contacts();
    let total = app.session.known_contacts().len();
    let selected_count = app.pending_group_members.len();
    let w = area.width as usize;

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
        let check = if is_member { "\u{2713} " } else { "  " };
        let addr_color = if is_self {
            Color::DarkGray
        } else if cursor {
            Color::White
        } else if is_member {
            Color::Green
        } else {
            Color::Gray
        };
        let addr_mod = if cursor {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };

        lines.push(Line::from(vec![
            Span::styled(indicator, Style::default().fg(Color::Cyan)),
            Span::styled(check, Style::default().fg(Color::Green)),
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
        let indent = 7 + 3 + 2; // " ⠿⠒⠒⠒⠒ " + "You" + "  "
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} ", app.spinner_5()),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                "You  ",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(first_line.to_string(), Style::default().fg(Color::DarkGray)),
        ]));
        for body_line in text.lines().skip(1) {
            lines.push(Line::styled(
                format!("{:indent$}{body_line}", ""),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }
}

fn render_messages(
    lines: &mut Vec<Line<'static>>,
    messages: &[taolk::conversation::ThreadMessage],
    app: &App,
    width: usize,
) {
    let my_ss58 = app.session.ss58();
    let mut last_date: Option<chrono::NaiveDate> = None;
    let mut last_sender: Option<&str> = None;
    let mut last_indent: usize = 7; // fallback

    for (i, msg) in messages.iter().enumerate() {
        // Date separator between different days
        let msg_date = msg.timestamp.with_timezone(&chrono::Local).date_naive();
        if last_date.is_some() && last_date != Some(msg_date) {
            let date_str = msg_date.format("%B %-d, %Y").to_string();
            lines.push(date_separator(&date_str));
            last_sender = None; // reset compaction after date separator
        }
        last_date = Some(msg_date);

        // Gap indicator
        if msg.has_gap {
            let pulse = if app.frame % 8 < 4 {
                Color::DarkGray
            } else {
                Color::Cyan
            };
            let gap_text = truncate(
                " \u{25B2} Earlier messages may be missing \u{00B7} press r to load",
                width,
            );
            lines.push(Line::from(vec![Span::styled(
                gap_text,
                Style::default().fg(pulse),
            )]));
            last_sender = None; // reset compaction after gap
        }

        let search = &app.search_query;
        let is_empty_body = msg.body.is_empty();
        let has_match =
            !search.is_empty() && msg.body.to_lowercase().contains(&search.to_lowercase());
        let body_color = if has_match { Color::Cyan } else { Color::White };
        let display_body = if is_empty_body { "empty" } else { &msg.body };
        let first_body_line = display_body.lines().next().unwrap_or("");

        // Compact consecutive: same sender omits timestamp+name
        // But show timestamp if >5 minutes since last message
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
            Color::DarkGray
        } else {
            body_color
        };

        if is_empty_body && same_sender && !time_gap {
            lines.push(Line::from(vec![Span::styled(
                format!("{:last_indent$}empty", ""),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )]));
        } else if is_empty_body {
            // Empty body with header (new sender or time gap): render header then italic "empty"
            let time = msg
                .timestamp
                .with_timezone(&chrono::Local)
                .format(&app.timestamp_format)
                .to_string();
            let (name, name_color) = if msg.is_mine {
                ("You".to_string(), Color::Cyan)
            } else {
                (
                    truncate(&msg.sender_ss58, 16),
                    sender_color(&msg.sender_ss58),
                )
            };
            let indent = 7 + name.len() + 2;
            last_indent = indent;
            lines.push(Line::from(vec![
                Span::styled(format!(" {time} "), Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{name}  "),
                    Style::default().fg(name_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "empty",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
        } else if same_sender && !time_gap {
            // Compact: just body at previous indent
            for body_line in display_body.lines() {
                let prefixed = format!("{:last_indent$}{body_line}", "");
                for wrapped in wrap_text(&prefixed, width, last_indent) {
                    lines.push(render_body_line(&wrapped, body_color, my_ss58));
                }
            }
        } else if same_sender && time_gap {
            // Same sender but time gap: show timestamp, no name
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
                Span::styled(format!(" {time} "), Style::default().fg(Color::DarkGray)),
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
            // Blank line between different sender groups
            if i > 0 {
                lines.push(Line::raw(""));
            }

            let time = msg
                .timestamp
                .with_timezone(&chrono::Local)
                .format(&app.timestamp_format)
                .to_string();
            let (name, name_color) = if msg.is_mine {
                ("You".to_string(), Color::Cyan)
            } else {
                (
                    truncate(&msg.sender_ss58, 16),
                    sender_color(&msg.sender_ss58),
                )
            };

            let indent = 7 + name.len() + 2;
            last_indent = indent;

            // Wrap the first body line within available space after header
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

            // First line: timestamp + name + body (first part)
            let mut first_spans = vec![
                Span::styled(format!(" {time} "), Style::default().fg(Color::DarkGray)),
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

            // Overflow from first line wrap
            if let Some(overflow) = first_wrapped.1 {
                for wrapped in wrap_text(&format!("{:indent$}{overflow}", ""), width, indent) {
                    lines.push(render_body_line(&wrapped, body_color, my_ss58));
                }
            }

            // Continuation lines (explicit \n in message)
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
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(pad)),
        Span::styled(right.to_string(), Style::default().fg(Color::DarkGray)),
    ])
}

fn separator(width: u16) -> Line<'static> {
    Line::styled(
        "\u{2500}".repeat(width as usize),
        Style::default().fg(Color::DarkGray),
    )
}

fn dim(text: &str) -> Line<'static> {
    Line::styled(text.to_string(), Style::default().fg(Color::DarkGray))
}

fn render_scrolled(frame: &mut ratatui::Frame, lines: Vec<Line<'_>>, scroll: usize, area: Rect) {
    let visible = area.height as usize;
    let bottom = lines.len().saturating_sub(visible);
    let start = bottom.saturating_sub(scroll);
    let end = (start + visible).min(lines.len());
    frame.render_widget(Paragraph::new(lines[start..end].to_vec()), area);
}

use taolk::util::truncate;
