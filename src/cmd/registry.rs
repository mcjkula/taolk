use crate::app::{App, Focus, Overlay, View};

pub type CmdResult = Result<(), String>;

pub struct Command {
    pub name: &'static str,
    pub summary: &'static str,
    pub run: fn(&mut App, &[&str]) -> CmdResult,
}

pub const COMMANDS: &[Command] = &[
    Command {
        name: "help",
        summary: "Show the help overlay",
        run: run_help,
    },
    Command {
        name: "quit",
        summary: "Exit taolk",
        run: run_quit,
    },
    Command {
        name: "theme",
        summary: "Switch theme (mocha|latte|tokyo-night|gruvbox-dark|rose-pine|monochrome)",
        run: run_theme,
    },
    Command {
        name: "sidebar",
        summary: "Toggle the sidebar",
        run: run_sidebar,
    },
    Command {
        name: "search",
        summary: "Search messages in the current view",
        run: run_search,
    },
    Command {
        name: "new",
        summary: "Start a new thread with a contact",
        run: run_new,
    },
    Command {
        name: "message",
        summary: "Send a standalone message",
        run: run_message,
    },
    Command {
        name: "channels",
        summary: "Browse the channel directory",
        run: run_channels,
    },
    Command {
        name: "inbox",
        summary: "Jump to the inbox view",
        run: run_inbox,
    },
    Command {
        name: "outbox",
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

fn run_theme(app: &mut App, args: &[&str]) -> CmdResult {
    let name = args.first().copied().unwrap_or("");
    let choice =
        taolk::config::ThemeChoice::parse(name).ok_or_else(|| format!("unknown theme: {name}"))?;
    app.theme = choice;
    app.set_status(format!("theme \u{2192} {}", choice.as_str()));
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
        }
    }
}
