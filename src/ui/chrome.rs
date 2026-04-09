use crate::config::ColorMode;
use crate::ui::symbols;
use crate::ui::theme::{Theme, apply_mode};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Padding};

pub fn panel(theme: &Theme, mode: ColorMode, focused: bool) -> Block<'static> {
    let border_color = if focused {
        theme.border_focus
    } else {
        theme.border
    };
    let mut border_style = Style::default().fg(apply_mode(mode, border_color));
    if focused {
        border_style = border_style.add_modifier(Modifier::BOLD);
    }
    let content_style = Style::default()
        .bg(apply_mode(mode, theme.bg))
        .fg(apply_mode(mode, theme.text));
    Block::bordered()
        .border_type(symbols::PANEL_BORDER)
        .border_style(border_style)
        .style(content_style)
        .padding(Padding::horizontal(1))
}

pub fn surface_panel(theme: &Theme, mode: ColorMode) -> Block<'static> {
    let content_style = Style::default()
        .bg(apply_mode(mode, theme.surface))
        .fg(apply_mode(mode, theme.text));
    let border_style = Style::default()
        .fg(apply_mode(mode, theme.border_focus))
        .add_modifier(Modifier::BOLD);
    Block::bordered()
        .border_type(symbols::PANEL_BORDER)
        .border_style(border_style)
        .style(content_style)
        .padding(Padding::horizontal(1))
}
