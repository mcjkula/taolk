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

mod icons {
    pub const INBOX: &str = "\u{2709}";
    pub const OUTBOX: &str = "\u{2197}";
    pub const PUBLIC: &str = "\u{25CB}";
    pub const ENCRYPTED: &str = "\u{25CF}";
    pub const THREADS: &str = "\u{21C4}";
    pub const CHANNELS: &str = "\u{2261}";
    pub const GROUPS: &str = "\u{2299}";
    pub const CREATOR: &str = "\u{2605}";
    pub const DRAFT: &str = "\u{270E}";
}

use crate::app::App;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

pub const MIN_WIDTH: u16 = 100;
pub const MIN_HEIGHT: u16 = 28;

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
        "taolk requires at least {MIN_WIDTH}x{MIN_HEIGHT} — current {}x{}",
        area.width, area.height,
    );
    let lines = vec![Line::raw(""), Line::raw(msg)];
    frame.render_widget(Paragraph::new(lines).style(palette::dim()), area);
}
