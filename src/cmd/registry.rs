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
    Command {
        name: "help",
        glyph: icons::HELP,
        summary: "Show the help overlay",
        run: run_help,
    },
    Command {
        name: "quit",
        glyph: icons::EXIT,
        summary: "Exit taolk",
        run: run_quit,
    },
    Command {
        name: "sidebar",
        glyph: icons::MENU,
        summary: "Toggle the sidebar",
        run: run_sidebar,
    },
    Command {
        name: "search",
        glyph: icons::MAGNIFY,
        summary: "Search messages in the current view",
        run: run_search,
    },
    Command {
        name: "new",
        glyph: icons::THREADS,
        summary: "Start a new thread with a contact",
        run: run_new,
    },
    Command {
        name: "message",
        glyph: icons::OUTBOX,
        summary: "Send a standalone message",
        run: run_message,
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
];

fn run_help(app: &mut App, _: &[&str]) -> CmdResult {
    app.enter_overlay(Overlay::Help);
    Ok(())
}

fn run_quit(app: &mut App, _: &[&str]) -> CmdResult {
    app.running = false;
    Ok(())
}

fn run_sidebar(app: &mut App, _: &[&str]) -> CmdResult {
    app.show_sidebar = !app.show_sidebar;
    Ok(())
}

fn run_search(app: &mut App, _: &[&str]) -> CmdResult {
    app.search_query.clear();
    app.enter_overlay(Overlay::Search);
    Ok(())
}

fn run_new(app: &mut App, _: &[&str]) -> CmdResult {
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

fn run_channels(app: &mut App, _: &[&str]) -> CmdResult {
    app.channel_dir_cursor = 0;
    app.channel_dir_input.clear();
    app.scroll_offset = 0;
    app.view = View::ChannelDir;
    app.focus = Focus::Timeline;
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
