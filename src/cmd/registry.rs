use crate::app::{App, Focus, Overlay, View};
use crate::ui::icons;

pub type CmdResult = Result<(), String>;

pub struct Command {
    pub name: &'static str,
    pub glyph: &'static str,
    pub summary: &'static str,
    pub run: fn(&mut App, &[&str]) -> CmdResult,
}

pub const COMMANDS: &[Command] = &[
    // Compose
    Command {
        name: "thread",
        glyph: icons::THREADS,
        summary: "Start a new thread with a contact",
        run: run_thread,
    },
    Command {
        name: "message",
        glyph: icons::OUTBOX,
        summary: "Send a one-off message (not a thread)",
        run: run_message,
    },
    Command {
        name: "group",
        glyph: icons::GROUPS,
        summary: "Create a new group conversation",
        run: run_group,
    },
    // Navigate
    Command {
        name: "search",
        glyph: icons::MAGNIFY,
        summary: "Search messages in the current view",
        run: run_search,
    },
    Command {
        name: "channels",
        glyph: icons::CHANNELS,
        summary: "Browse the channel directory",
        run: run_channels,
    },
    Command {
        name: "inbox",
        glyph: icons::INBOX,
        summary: "Jump to the inbox view",
        run: run_inbox,
    },
    Command {
        name: "outbox",
        glyph: icons::OUTBOX,
        summary: "Jump to the sent view",
        run: run_outbox,
    },
    // View
    Command {
        name: "sidebar",
        glyph: icons::MENU,
        summary: "Toggle the sidebar",
        run: run_sidebar,
    },
    Command {
        name: "help",
        glyph: icons::HELP,
        summary: "Show the help overlay",
        run: run_help,
    },
    // System
    Command {
        name: "refresh",
        glyph: icons::REFRESH,
        summary: "Reload and fill message gaps",
        run: run_refresh,
    },
    Command {
        name: "copy",
        glyph: icons::COPY,
        summary: "Copy a sender's SS58 address",
        run: run_copy,
    },
    Command {
        name: "unlock",
        glyph: icons::LOCK_OPEN,
        summary: "Unlock all locked outbound messages",
        run: run_unlock,
    },
    Command {
        name: "lock",
        glyph: icons::ENCRYPTED,
        summary: "Lock the session",
        run: run_lock,
    },
    Command {
        name: "wallet",
        glyph: icons::SWAP,
        summary: "Switch to a different wallet",
        run: run_wallet,
    },
    Command {
        name: "quit",
        glyph: icons::EXIT,
        summary: "Exit taolk",
        run: run_quit,
    },
];

fn run_help(app: &mut App, _: &[&str]) -> CmdResult {
    app.enter_overlay(Overlay::Help);
    Ok(())
}

fn run_quit(app: &mut App, _: &[&str]) -> CmdResult {
    app.running = false;
    Ok(())
}

fn run_thread(app: &mut App, _: &[&str]) -> CmdResult {
    if !app.check_not_sending() {
        return Err("busy sending".into());
    }
    app.enter_overlay(Overlay::Compose);
    Ok(())
}

fn run_message(app: &mut App, _: &[&str]) -> CmdResult {
    if !app.check_not_sending() {
        return Err("busy sending".into());
    }
    app.enter_overlay(Overlay::Message);
    Ok(())
}

fn run_group(app: &mut App, _: &[&str]) -> CmdResult {
    if !app.check_not_sending() {
        return Err("busy sending".into());
    }
    app.pending_group_members.clear();
    let my_pk = app.session.pubkey();
    let my_ss58 = app.session.my_ss58.clone();
    app.pending_group_members.push((my_pk, my_ss58));
    app.contact_idx = 0;
    app.enter_overlay(Overlay::CreateGroupMembers);
    Ok(())
}

fn run_channels(app: &mut App, _: &[&str]) -> CmdResult {
    app.channel_dir_cursor = 0;
    app.channel_dir_input.clear();
    app.scroll_offset = 0;
    app.view = View::ChannelDir;
    app.focus = Focus::Timeline;
    Ok(())
}

fn run_search(app: &mut App, _: &[&str]) -> CmdResult {
    app.search_query.clear();
    app.enter_overlay(Overlay::Search);
    Ok(())
}

fn run_sidebar(app: &mut App, _: &[&str]) -> CmdResult {
    app.show_sidebar = !app.show_sidebar;
    Ok(())
}

fn run_inbox(app: &mut App, _: &[&str]) -> CmdResult {
    app.view = View::Inbox;
    app.focus = Focus::Timeline;
    app.scroll_offset = 0;
    Ok(())
}

fn run_outbox(app: &mut App, _: &[&str]) -> CmdResult {
    app.view = View::Outbox;
    app.focus = Focus::Timeline;
    app.scroll_offset = 0;
    Ok(())
}

fn run_refresh(app: &mut App, _: &[&str]) -> CmdResult {
    app.refresh_requested = true;
    app.set_status("Refreshing...");
    Ok(())
}

fn run_unlock(app: &mut App, _: &[&str]) -> CmdResult {
    if app.locked_outbound.is_empty() {
        return Err("no locked outbound".into());
    }
    app.pending_unlock_all = true;
    Ok(())
}

fn run_copy(app: &mut App, _: &[&str]) -> CmdResult {
    let senders = app.build_picker_senders();
    if senders.is_empty() {
        return Err("no senders in current view".into());
    }
    app.picker_senders = senders;
    app.contact_idx = 0;
    app.enter_overlay(Overlay::SenderPicker);
    Ok(())
}

fn run_lock(app: &mut App, _: &[&str]) -> CmdResult {
    app.lock_requested = true;
    Ok(())
}

fn run_wallet(app: &mut App, _: &[&str]) -> CmdResult {
    app.wallet_switch_requested = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_is_populated() {
        for c in COMMANDS {
            assert!(!c.name.is_empty());
            assert!(!c.summary.is_empty());
            assert!(!c.glyph.is_empty());
        }
    }
}
