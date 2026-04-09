use crate::ui::palette;
use crate::ui::symbols;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Padding};

pub fn panel(focused: bool) -> Block<'static> {
    let border_style = if focused {
        Style::default()
            .fg(palette::ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette::MUTED)
    };
    Block::bordered()
        .border_type(symbols::PANEL_BORDER)
        .border_style(border_style)
        .padding(Padding::horizontal(1))
}
