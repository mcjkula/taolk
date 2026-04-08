mod help;
mod input;
mod messages;
pub mod modal;
mod sidebar;
mod status;

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
    pub const ERROR: &str = "\u{2717}";
    pub const SUCCESS: &str = "\u{2713}";
}

use crate::app::App;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};

pub fn render(frame: &mut Frame, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(frame.area());

    let main_area = outer[0];
    let status_area = outer[1];

    if app.show_sidebar {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(app.sidebar_width), Constraint::Min(30)])
            .split(main_area);

        sidebar::render(frame, app, cols[0]);
        render_main_panel(frame, app, cols[1]);
    } else {
        render_main_panel(frame, app, main_area);
    }

    status::render(frame, app, status_area);

    if app.mode == crate::app::Mode::Help {
        help::render(frame, frame.area());
    }
}

fn render_main_panel(frame: &mut Frame, app: &App, area: Rect) {
    use crate::app::Mode;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .border_type(ratatui::widgets::BorderType::Rounded);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text_lines = if app.mode == Mode::Insert && !app.input.is_empty() {
        app.input.split('\n').count().clamp(1, 4)
    } else {
        1
    };
    let input_height = u16::try_from(text_lines + 2).unwrap_or(6);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(input_height)])
        .split(inner);

    messages::render(frame, app, rows[0]);
    input::render(frame, app, rows[1]);
}
