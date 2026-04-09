use crate::app::{App, View};
use crate::conversation::Conversation;
use crate::ui::chrome;
use crate::ui::theme::{apply_mode, theme_for};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = theme_for(app.theme);
    let mode = app.color_mode;
    let text_style = Style::default().fg(apply_mode(mode, theme.text));
    let selected_style = Style::default()
        .fg(apply_mode(mode, theme.accent))
        .add_modifier(Modifier::BOLD);
    let dim_style = Style::default().fg(apply_mode(mode, theme.text_dim));
    let accent_style = Style::default().fg(apply_mode(mode, theme.accent));
    let unread_style = Style::default()
        .fg(apply_mode(mode, theme.accent))
        .add_modifier(Modifier::BOLD);
    let title_style = Style::default()
        .fg(apply_mode(mode, theme.accent))
        .add_modifier(Modifier::BOLD);
    let item_style = |selected: bool| if selected { selected_style } else { text_style };
    let indicator = |selected: bool| if selected { "\u{25B8} " } else { "  " };
    let max_name = usize::from(area.width.saturating_sub(8));
    let mut items: Vec<ListItem> = Vec::new();

    let inbox_selected = app.view == View::Inbox;
    let inbox_style = item_style(inbox_selected);
    items.push(ListItem::new(Line::from(vec![
        Span::styled(indicator(inbox_selected), inbox_style),
        Span::styled(format!("{} Inbox", super::icons::INBOX), inbox_style),
        Span::styled(format!(" ({})", app.session.inbox.len()), dim_style),
    ])));

    let outbox_selected = app.view == View::Outbox;
    let outbox_style = item_style(outbox_selected);
    items.push(ListItem::new(Line::from(vec![
        Span::styled(indicator(outbox_selected), outbox_style),
        Span::styled(format!("{} Sent", super::icons::OUTBOX), outbox_style),
        Span::styled(format!(" ({})", app.session.outbox.len()), dim_style),
    ])));

    if !app.session.threads.is_empty() {
        items.push(ListItem::new(Line::raw("")));
        items.push(ListItem::new(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(format!("{} ", super::icons::THREADS), dim_style),
            Span::styled("Threads", dim_style),
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
                    spans.push(Span::styled(format!(" {}", app.spinner_1()), dim_style));
                } else if unread > 0 {
                    spans.push(Span::styled(format!(" ({unread})"), unread_style));
                }
                if !thread.draft.is_empty() {
                    spans.push(Span::styled(
                        format!(" {}", super::icons::DRAFT),
                        accent_style,
                    ));
                }
                items.push(ListItem::new(Line::from(spans)));
            } else {
                items.push(ListItem::new(Line::styled(
                    format!("    {}", truncate(peer_ss58, max_name.saturating_sub(2))),
                    dim_style,
                )));
                for (j, &i) in thread_idxs.iter().enumerate() {
                    let thread = &app.session.threads[i];
                    let selected = matches!(app.view, View::Thread(idx) if idx == i);
                    let style = item_style(selected);
                    let label = if thread.thread_ref.is_zero() {
                        app.spinner_5().to_string()
                    } else {
                        format!(
                            "{}:{}",
                            thread.thread_ref.block().get(),
                            thread.thread_ref.index().get()
                        )
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
                        Span::styled(branch, dim_style),
                        Span::styled(
                            truncate(&label, max_name.saturating_sub(6 + badge_reserve)),
                            style,
                        ),
                    ];
                    if unread > 0 {
                        spans.push(Span::styled(format!(" ({unread})"), unread_style));
                    }
                    if !thread.draft.is_empty() {
                        spans.push(Span::styled(
                            format!(" {}", super::icons::DRAFT),
                            accent_style,
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
            Span::styled(format!("{} ", super::icons::CHANNELS), dim_style),
            Span::styled("Channels", dir_style),
            Span::styled(
                format!(" ({})", app.session.known_channels.len()),
                dim_style,
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
                spans.push(Span::styled(format!(" {}", app.spinner_1()), dim_style));
            } else if unread > 0 {
                spans.push(Span::styled(format!(" ({unread})"), unread_style));
            }
            if !channel.draft.is_empty() {
                spans.push(Span::styled(
                    format!(" {}", super::icons::DRAFT),
                    accent_style,
                ));
            }
            items.push(ListItem::new(Line::from(spans)));
        }
    }

    if !app.session.groups.is_empty() {
        items.push(ListItem::new(Line::raw("")));
        items.push(ListItem::new(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(format!("{} ", super::icons::GROUPS), dim_style),
            Span::styled("Groups", dim_style),
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
                spans.push(Span::styled(format!(" {}", app.spinner_1()), dim_style));
            } else if unread > 0 {
                spans.push(Span::styled(format!(" ({unread})"), unread_style));
            }
            if !group.draft.is_empty() {
                spans.push(Span::styled(
                    format!(" {}", super::icons::DRAFT),
                    accent_style,
                ));
            }
            items.push(ListItem::new(Line::from(spans)));
        }
    }

    let block = chrome::panel(theme, mode, false)
        .title(" \u{03C4}alk ")
        .title_style(title_style);

    frame.render_widget(List::new(items).block(block), area);
}

use taolk::util::truncate;
