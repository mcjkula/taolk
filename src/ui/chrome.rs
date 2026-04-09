use crate::config::ColorMode;
use crate::ui::symbols;
use crate::ui::theme::{Theme, apply_mode};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Padding};

pub fn panel(theme: &Theme, mode: ColorMode, focused: bool) -> Block<'static> {
    let color = if focused {
        theme.border_focus
    } else {
        theme.border
    };
    let mut style = Style::default().fg(apply_mode(mode, color));
    if focused {
        style = style.add_modifier(Modifier::BOLD);
    }
    Block::bordered()
        .border_type(symbols::PANEL_BORDER)
        .border_style(style)
        .padding(Padding::horizontal(1))
}
