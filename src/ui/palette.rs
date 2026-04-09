use ratatui::style::{Color, Modifier, Style};

pub const ACCENT: Color = Color::Cyan;
pub const ACCENT_ALT: Color = Color::Magenta;
pub const ERROR: Color = Color::Red;
pub const WARNING: Color = Color::Yellow;
pub const SUCCESS: Color = Color::Green;
pub const MUTED: Color = Color::DarkGray;

pub fn dim() -> Style {
    Style::default().fg(MUTED)
}

pub fn strong() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

pub const SENDER_ROTATION: [Color; 8] = [
    Color::Cyan,
    Color::Green,
    Color::Magenta,
    Color::Blue,
    Color::Yellow,
    Color::LightCyan,
    Color::LightMagenta,
    Color::LightBlue,
];

pub fn sender_color(ss58: &str) -> Color {
    let hash: u8 = ss58.bytes().fold(0u8, |acc, b| acc.wrapping_add(b));
    SENDER_ROTATION[usize::from(hash) % SENDER_ROTATION.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sender_color_is_deterministic() {
        let a = sender_color("5FHneW46xGXgs5AUiveU4sbTyGBzmstUspZC92UhjJM694ty");
        let b = sender_color("5FHneW46xGXgs5AUiveU4sbTyGBzmstUspZC92UhjJM694ty");
        assert_eq!(a, b);
    }

    #[test]
    fn sender_color_is_in_rotation() {
        let c = sender_color("anything");
        assert!(SENDER_ROTATION.contains(&c));
    }
}
