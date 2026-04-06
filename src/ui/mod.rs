mod icons;
mod input;
mod messages;
mod sidebar;
mod status;

use crate::app::App;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};

pub fn render(frame: &mut Frame, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),    // main area
            Constraint::Length(1), // status bar
        ])
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
}

fn render_main_panel(frame: &mut Frame, app: &App, area: Rect) {
    use crate::app::Mode;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .border_type(ratatui::widgets::BorderType::Rounded);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Dynamic input height: grows with content in Insert mode (up to 4 text lines)
    let text_lines = if app.mode == Mode::Insert && !app.input.is_empty() {
        app.input.split('\n').count().clamp(1, 4)
    } else {
        1
    };
    let input_height = (text_lines + 2) as u16; // +2 for separator + hints

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(input_height)])
        .split(inner);

    messages::render(frame, app, rows[0]);
    input::render(frame, app, rows[1]);
}
