use crate::app::{App, View};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let max_name = usize::from(area.width.saturating_sub(8));
    let mut items: Vec<ListItem> = Vec::new();

    let inbox_selected = app.view == View::Inbox;
    let inbox_style = item_style(inbox_selected);
    items.push(ListItem::new(Line::from(vec![
        Span::styled(indicator(inbox_selected), inbox_style),
        Span::styled(format!("{} Inbox", super::icons::INBOX), inbox_style),
        Span::styled(
            format!(" ({})", app.session.inbox.len()),
            Style::default().fg(Color::DarkGray),
        ),
    ])));

    let outbox_selected = app.view == View::Outbox;
    let outbox_style = item_style(outbox_selected);
    items.push(ListItem::new(Line::from(vec![
        Span::styled(indicator(outbox_selected), outbox_style),
        Span::styled(format!("{} Sent", super::icons::OUTBOX), outbox_style),
        Span::styled(
            format!(" ({})", app.session.outbox.len()),
            Style::default().fg(Color::DarkGray),
        ),
    ])));

    if !app.session.threads.is_empty() {
        items.push(ListItem::new(Line::raw("")));
        items.push(ListItem::new(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("{} ", super::icons::THREADS),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled("Threads", Style::default().fg(Color::DarkGray)),
        ])));

        let mut peer_groups: Vec<(String, Vec<usize>)> = Vec::new();
        let mut peer_idx_map: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for (i, thread) in app.session.threads.iter().enumerate() {
            if let Some(&gi) = peer_idx_map.get(&thread.peer_ss58) {
                peer_groups[gi].1.push(i);
            } else {
                let gi = peer_groups.len();
                peer_idx_map.insert(thread.peer_ss58.clone(), gi);
                peer_groups.push((thread.peer_ss58.clone(), vec![i]));
            }
        }
        peer_groups.sort_by(|a, b| {
            let latest_a =
                a.1.iter()
                    .filter_map(|&i| app.session.threads[i].messages.last())
                    .map(|m| (m.block_number, m.ext_index))
                    .max()
                    .unwrap_or((0, 0));
            let latest_b =
                b.1.iter()
                    .filter_map(|&i| app.session.threads[i].messages.last())
                    .map(|m| (m.block_number, m.ext_index))
                    .max()
                    .unwrap_or((0, 0));
            latest_b.cmp(&latest_a)
        });
        for (_, idxs) in &mut peer_groups {
            idxs.sort_by(|&a, &b| {
                let la = app.session.threads[a]
                    .messages
                    .last()
                    .map(|m| (m.block_number, m.ext_index))
                    .unwrap_or((0, 0));
                let lb = app.session.threads[b]
                    .messages
                    .last()
                    .map(|m| (m.block_number, m.ext_index))
                    .unwrap_or((0, 0));
                lb.cmp(&la)
            });
        }

        for (peer_ss58, thread_idxs) in &peer_groups {
            if thread_idxs.len() == 1 {
                let i = thread_idxs[0];
                let thread = &app.session.threads[i];
                let selected = matches!(app.view, View::Thread(idx) if idx == i);
                let style = item_style(selected);
                let unread = thread.unread();
                let badge_reserve = if unread > 0 {
                    format!(" ({unread})").len()
                } else {
                    0
                } + if !thread.draft.is_empty() { 2 } else { 0 };
                let name = truncate(peer_ss58, max_name.saturating_sub(badge_reserve));
                let mut spans = vec![
                    Span::styled(indicator(selected), style),
                    Span::styled(name, style),
                ];
                if thread.thread_ref.is_zero() {
                    spans.push(Span::styled(
                        format!(" {}", app.spinner_1()),
                        Style::default().fg(Color::DarkGray),
                    ));
                } else if unread > 0 {
                    spans.push(Span::styled(
                        format!(" ({unread})"),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
                if !thread.draft.is_empty() {
                    spans.push(Span::styled(
                        format!(" {}", super::icons::DRAFT),
                        Style::default().fg(Color::Cyan),
                    ));
                }
                items.push(ListItem::new(Line::from(spans)));
            } else {
                items.push(ListItem::new(Line::styled(
                    format!("    {}", truncate(peer_ss58, max_name.saturating_sub(2))),
                    Style::default().fg(Color::DarkGray),
                )));
                for (j, &i) in thread_idxs.iter().enumerate() {
                    let thread = &app.session.threads[i];
                    let selected = matches!(app.view, View::Thread(idx) if idx == i);
                    let style = item_style(selected);
                    let label = if thread.thread_ref.is_zero() {
                        app.spinner_5().to_string()
                    } else {
                        format!("{}:{}", thread.thread_ref.block, thread.thread_ref.index)
                    };
                    let is_last = j == thread_idxs.len() - 1;
                    let branch = if is_last {
                        "\u{2514}\u{2500} "
                    } else {
                        "\u{251C}\u{2500} "
                    };
                    let unread = thread.unread();
                    let badge_reserve = if unread > 0 {
                        format!(" ({unread})").len()
                    } else {
                        0
                    } + if !thread.draft.is_empty() { 2 } else { 0 };
                    let mut spans = vec![
                        Span::styled(if selected { " \u{25B8}" } else { "  " }, style),
                        Span::styled(branch, Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            truncate(&label, max_name.saturating_sub(6 + badge_reserve)),
                            style,
                        ),
                    ];
                    if unread > 0 {
                        spans.push(Span::styled(
                            format!(" ({unread})"),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }
                    if !thread.draft.is_empty() {
                        spans.push(Span::styled(
                            format!(" {}", super::icons::DRAFT),
                            Style::default().fg(Color::Cyan),
                        ));
                    }
                    items.push(ListItem::new(Line::from(spans)));
                }
            }
        }
    }

    {
        items.push(ListItem::new(Line::raw("")));
        let dir_selected = app.view == View::ChannelDir;
        let dir_style = item_style(dir_selected);
        items.push(ListItem::new(Line::from(vec![
            Span::styled(indicator(dir_selected), dir_style),
            Span::styled(
                format!("{} ", super::icons::CHANNELS),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled("Channels", dir_style),
            Span::styled(
                format!(" ({})", app.session.known_channels.len()),
                Style::default().fg(Color::DarkGray),
            ),
        ])));

        let mut chan_order: Vec<usize> = (0..app.session.channels.len())
            .filter(|&i| {
                app.session
                    .is_subscribed(&app.session.channels[i].channel_ref)
            })
            .collect();
        chan_order.sort_by(|&a, &b| {
            let la = app.session.channels[a]
                .messages
                .last()
                .map(|m| (m.block_number, m.ext_index))
                .unwrap_or((0, 0));
            let lb = app.session.channels[b]
                .messages
                .last()
                .map(|m| (m.block_number, m.ext_index))
                .unwrap_or((0, 0));
            lb.cmp(&la)
        });

        for &i in &chan_order {
            let channel = &app.session.channels[i];
            let selected = matches!(app.view, View::Channel(idx) if idx == i);
            let style = item_style(selected);
            let unread = channel.unread();
            let badge_reserve = if unread > 0 {
                format!(" ({unread})").len()
            } else {
                0
            } + if !channel.draft.is_empty() { 2 } else { 0 };
            let name = truncate(
                &format!("#{}", channel.name),
                max_name.saturating_sub(badge_reserve),
            );
            let mut spans = vec![
                Span::styled(indicator(selected), style),
                Span::styled(name, style),
            ];
            if channel.channel_ref.is_zero() {
                spans.push(Span::styled(
                    format!(" {}", app.spinner_1()),
                    Style::default().fg(Color::DarkGray),
                ));
            } else if unread > 0 {
                spans.push(Span::styled(
                    format!(" ({unread})"),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            if !channel.draft.is_empty() {
                spans.push(Span::styled(
                    format!(" {}", super::icons::DRAFT),
                    Style::default().fg(Color::Cyan),
                ));
            }
            items.push(ListItem::new(Line::from(spans)));
        }
    }

    if !app.session.groups.is_empty() {
        items.push(ListItem::new(Line::raw("")));
        items.push(ListItem::new(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("{} ", super::icons::GROUPS),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled("Groups", Style::default().fg(Color::DarkGray)),
        ])));

        let mut group_order: Vec<usize> = (0..app.session.groups.len()).collect();
        group_order.sort_by(|&a, &b| {
            let la = app.session.groups[a]
                .messages
                .last()
                .map(|m| (m.block_number, m.ext_index))
                .unwrap_or((0, 0));
            let lb = app.session.groups[b]
                .messages
                .last()
                .map(|m| (m.block_number, m.ext_index))
                .unwrap_or((0, 0));
            lb.cmp(&la)
        });

        for &i in &group_order {
            let group = &app.session.groups[i];
            let selected = matches!(app.view, View::Group(idx) if idx == i);
            let style = item_style(selected);
            let unread = group.unread();
            let mut others: Vec<taolk::types::Pubkey> = group
                .members
                .iter()
                .filter(|pk| **pk != app.session.pubkey())
                .copied()
                .collect();
            if let Some(pos) = others.iter().position(|pk| *pk == group.creator_pubkey) {
                let c = others.remove(pos);
                others.insert(0, c);
            }
            let name = match others.len() {
                0 => "(you)".to_string(),
                1 => taolk::util::ss58_short(&others[0]),
                _ => {
                    let a = &taolk::util::ss58_from_pubkey(&others[0])[..6];
                    let b = &taolk::util::ss58_from_pubkey(&others[1])[..6];
                    if others.len() == 2 {
                        format!("{a},{b}")
                    } else {
                        format!("{a},{b}+{}", others.len() - 2)
                    }
                }
            };
            let mut spans = vec![
                Span::styled(indicator(selected), style),
                Span::styled(name, style),
            ];
            if group.group_ref.is_zero() {
                spans.push(Span::styled(
                    format!(" {}", app.spinner_1()),
                    Style::default().fg(Color::DarkGray),
                ));
            } else if unread > 0 {
                spans.push(Span::styled(
                    format!(" ({unread})"),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            if !group.draft.is_empty() {
                spans.push(Span::styled(
                    format!(" {}", super::icons::DRAFT),
                    Style::default().fg(Color::Cyan),
                ));
            }
            items.push(ListItem::new(Line::from(spans)));
        }
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" \u{03C4}alk ")
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(List::new(items).block(block), area);
}

fn item_style(selected: bool) -> Style {
    if selected {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    }
}

fn indicator(selected: bool) -> &'static str {
    if selected { "▸ " } else { "  " }
}

use taolk::util::truncate;
