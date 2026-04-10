pub mod chat_list;
pub mod chrome;
pub mod composer;
pub mod hintbar;
mod input;
pub mod modal;
pub mod overlay;
pub mod palette;
pub mod statusline;
pub mod symbols;
pub mod timeline;
pub mod welcome;

pub mod icons {
    // Conversation types
    pub const INBOX: &str = "\u{F02FB}";
    pub const OUTBOX: &str = "\u{F048A}";
    pub const THREADS: &str = "\u{F0369}";
    pub const CHANNELS: &str = "\u{F0423}";
    pub const GROUPS: &str = "\u{F0849}";

    // Message attributes
    pub const PUBLIC: &str = "\u{F0FC6}";
    pub const ENCRYPTED: &str = "\u{F033E}";
    pub const CREATOR: &str = "\u{F01A5}";
    pub const DRAFT: &str = "\u{F03EB}";

    // Status indicators
    pub const CHECK: &str = "\u{F012C}";
    pub const ERROR: &str = "\u{F0028}";
    pub const LOCK_CLOCK: &str = "\u{F097F}";
    pub const HISTORY: &str = "\u{F02DA}";
    pub const SYNC: &str = "\u{F04E6}";

    // Chain primitives
    pub const BLOCK: &str = "\u{F01A7}";

    // Identity & secrets
    pub const ACCOUNT: &str = "\u{F0B55}";
    pub const WALLET: &str = "\u{F0BDD}";
    pub const KEY: &str = "\u{F030B}";

    // Commands & affordances
    pub const HELP: &str = "\u{F0625}";
    pub const EXIT: &str = "\u{F0206}";
    pub const MAGNIFY: &str = "\u{F0349}";
    pub const MENU: &str = "\u{F035C}";
    pub const KEYBOARD: &str = "\u{F097B}";
    pub const COG: &str = "\u{F0493}";
    pub const REFRESH: &str = "\u{F0450}";
    pub const SWAP: &str = "\u{F04E1}";
    pub const COPY: &str = "\u{F018F}";
    pub const LOCK_OPEN: &str = "\u{F0340}";

    // Navigation
    // ARROW_UP/DOWN/LEFT/RIGHT are inlined at call sites because Rust const rules
    // forbid combo string composition from const refs. Codepoints:
    //   nf-md-arrow_up    \u{F005D}
    //   nf-md-arrow_down  \u{F0045}
    //   nf-md-arrow_left  \u{F004D}
    //   nf-md-arrow_right \u{F0054}
    pub const CHEVRON_LEFT: &str = "\u{F0141}";
    pub const CHEVRON_RIGHT: &str = "\u{F0142}";
}

use crate::app::App;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

pub const MIN_WIDTH: u16 = 60;
pub const MIN_HEIGHT: u16 = 16;

pub fn render(frame: &mut Frame, app: &App) {
    use crate::app::Overlay;

    let area = frame.area();

    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        render_too_small(frame, area);
        return;
    }

    if app.overlay == Some(Overlay::Help) {
        overlay::help::render(frame, app, area);
        return;
    }

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(area);

    let main_area = outer[0];
    let status_area = outer[1];

    if app.show_sidebar {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(app.sidebar_width), Constraint::Min(30)])
            .split(main_area);

        chat_list::render(frame, app, cols[0]);
        render_main_panel(frame, app, cols[1]);
    } else {
        render_main_panel(frame, app, main_area);
    }

    statusline::render(frame, app, status_area);

    match app.overlay {
        Some(Overlay::CommandPalette) => overlay::palette::render(frame, app),
        Some(Overlay::FuzzyJump) => overlay::jump::render(frame, app),
        _ => {}
    }
}

fn render_main_panel(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.is_composing();
    let block = chrome::panel(focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if welcome::should_render(app) && app.overlay.is_none() {
        welcome::render(frame, app, inner);
        return;
    }

    let text_lines = if app.is_composing() && !app.input.is_empty() {
        app.input.split('\n').count().clamp(1, 4)
    } else {
        1
    };
    let input_height = u16::try_from(text_lines + 2).unwrap_or(6);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(input_height)])
        .split(inner);

    timeline::render(frame, app, rows[0]);
    input::render(frame, app, rows[1]);
}

fn render_too_small(frame: &mut Frame, area: Rect) {
    let msg = format!(
        "{} taolk requires at least {MIN_WIDTH}x{MIN_HEIGHT} — current {}x{}",
        icons::ERROR,
        area.width,
        area.height,
    );
    let lines = vec![Line::raw(""), Line::raw(msg)];
    frame.render_widget(Paragraph::new(lines).style(palette::dim()), area);
}
