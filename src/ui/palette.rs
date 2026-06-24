// The theme system below is consumed by later commits; until then some items
// are unused. The allow is removed once startup wires it in.
#![allow(dead_code)]

use ratatui::style::{Color, Modifier, Style};

pub fn dim() -> Style {
    Style::default().fg(theme().muted)
}

pub fn strong() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

pub fn sender_color(ss58: &str) -> Color {
    let hash: u8 = ss58.bytes().fold(0u8, |acc, b| acc.wrapping_add(b));
    let senders = theme().senders;
    senders[usize::from(hash) % senders.len()]
}

// --- Selectable color themes ---------------------------------------------

/// Semantic UI colors. `TERMINAL` follows the terminal's own ANSI palette;
/// `DARK` and `LIGHT` are fixed RGB so they render with the same contrast on
/// any terminal.
pub struct ColorTheme {
    pub accent: Color,
    pub accent_alt: Color,
    pub error: Color,
    pub warning: Color,
    pub success: Color,
    pub muted: Color,
    pub senders: [Color; 8],
}

/// Follows the terminal's 16-color palette. Matches your terminal theme, but
/// can be low-contrast on schemes like Solarized.
pub const TERMINAL: ColorTheme = ColorTheme {
    accent: Color::Cyan,
    accent_alt: Color::Magenta,
    error: Color::Red,
    warning: Color::Yellow,
    success: Color::Green,
    muted: Color::DarkGray,
    senders: [
        Color::Cyan,
        Color::Green,
        Color::Magenta,
        Color::Blue,
        Color::Yellow,
        Color::LightCyan,
        Color::LightMagenta,
        Color::LightBlue,
    ],
};

/// Fixed-RGB dark theme with guaranteed contrast on dark backgrounds.
pub const DARK: ColorTheme = ColorTheme {
    accent: Color::Rgb(0x7a, 0xa2, 0xf7),
    accent_alt: Color::Rgb(0xbb, 0x9a, 0xf7),
    error: Color::Rgb(0xf7, 0x76, 0x8e),
    warning: Color::Rgb(0xe0, 0xaf, 0x68),
    success: Color::Rgb(0x9e, 0xce, 0x6a),
    muted: Color::Rgb(0x70, 0x7a, 0xa0),
    senders: [
        Color::Rgb(0x7a, 0xa2, 0xf7),
        Color::Rgb(0x9e, 0xce, 0x6a),
        Color::Rgb(0xbb, 0x9a, 0xf7),
        Color::Rgb(0x2a, 0xc3, 0xde),
        Color::Rgb(0xe0, 0xaf, 0x68),
        Color::Rgb(0xf7, 0x76, 0x8e),
        Color::Rgb(0x73, 0xda, 0xca),
        Color::Rgb(0xff, 0x9e, 0x64),
    ],
};

/// Fixed-RGB light theme with guaranteed contrast on light backgrounds.
pub const LIGHT: ColorTheme = ColorTheme {
    accent: Color::Rgb(0x2e, 0x7d, 0xe9),
    accent_alt: Color::Rgb(0x98, 0x54, 0xf1),
    error: Color::Rgb(0xf5, 0x2a, 0x65),
    warning: Color::Rgb(0x8c, 0x6c, 0x3e),
    success: Color::Rgb(0x58, 0x75, 0x39),
    muted: Color::Rgb(0x61, 0x72, 0xb0),
    senders: [
        Color::Rgb(0x2e, 0x7d, 0xe9),
        Color::Rgb(0x58, 0x75, 0x39),
        Color::Rgb(0x98, 0x54, 0xf1),
        Color::Rgb(0x00, 0x7a, 0x88),
        Color::Rgb(0x8c, 0x6c, 0x3e),
        Color::Rgb(0xf5, 0x2a, 0x65),
        Color::Rgb(0x33, 0x8c, 0x6c),
        Color::Rgb(0xb1, 0x5c, 0x00),
    ],
};

/// The three selectable color themes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColorStyle {
    Terminal,
    Dark,
    Light,
}

impl ColorStyle {
    /// Parse a config/env value; `None` for anything unrecognised.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "terminal" => Some(Self::Terminal),
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            _ => None,
        }
    }

    /// The theme this style selects.
    pub fn theme(self) -> &'static ColorTheme {
        match self {
            Self::Terminal => &TERMINAL,
            Self::Dark => &DARK,
            Self::Light => &LIGHT,
        }
    }
}

static ACTIVE: std::sync::OnceLock<&'static ColorTheme> = std::sync::OnceLock::new();

/// The active color theme. Defaults to `DARK` until `init` runs.
pub fn theme() -> &'static ColorTheme {
    ACTIVE.get().copied().unwrap_or(&DARK)
}

/// Set the active theme once. The first call wins.
pub fn init(t: &'static ColorTheme) {
    let _ = ACTIVE.set(t);
}

/// Pure resolution. Precedence: `TAOLK_COLORS` env, then `ui.colors` config,
/// then `light` if the terminal background looks light, else `dark`.
pub fn resolve_with(
    cfg_value: &str,
    colors_env: Option<&str>,
    bg_is_light: Option<bool>,
) -> &'static ColorTheme {
    if let Some(style) = colors_env.and_then(ColorStyle::parse) {
        return style.theme();
    }
    if let Some(style) = ColorStyle::parse(cfg_value) {
        return style.theme();
    }
    if bg_is_light == Some(true) {
        &LIGHT
    } else {
        &DARK
    }
}

/// Resolve from the real environment (`TAOLK_COLORS`, `COLORFGBG`).
pub fn resolve(cfg_value: &str) -> &'static ColorTheme {
    let colors = std::env::var("TAOLK_COLORS").ok();
    // COLORFGBG is "fg;bg" (sometimes "fg;default;bg"); a white background index
    // (7 or 15) means a light terminal. Best-effort; absent on most terminals.
    let bg_is_light = std::env::var("COLORFGBG").ok().and_then(|v| {
        v.rsplit(';')
            .next()
            .and_then(|b| b.trim().parse::<u8>().ok())
            .map(|bg| matches!(bg, 7 | 15))
    });
    resolve_with(cfg_value, colors.as_deref(), bg_is_light)
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
        assert!(theme().senders.contains(&c));
    }

    // `muted` differs across all three themes, so it identifies which one we got.
    #[test]
    fn colorstyle_parse_is_case_insensitive() {
        assert_eq!(ColorStyle::parse("dark"), Some(ColorStyle::Dark));
        assert_eq!(ColorStyle::parse("TERMINAL"), Some(ColorStyle::Terminal));
        assert_eq!(ColorStyle::parse(" light "), Some(ColorStyle::Light));
        assert_eq!(ColorStyle::parse("x"), None);
    }

    #[test]
    fn colorstyle_theme_maps() {
        assert_eq!(ColorStyle::Dark.theme().muted, DARK.muted);
        assert_eq!(ColorStyle::Terminal.theme().muted, TERMINAL.muted);
        assert_eq!(ColorStyle::Light.theme().muted, LIGHT.muted);
    }

    #[test]
    fn resolve_with_follows_precedence() {
        // env wins over everything
        assert_eq!(
            resolve_with("dark", Some("light"), Some(false)).muted,
            LIGHT.muted
        );
        // config value
        assert_eq!(
            resolve_with("terminal", None, Some(true)).muted,
            TERMINAL.muted
        );
        // light background -> light
        assert_eq!(resolve_with("", None, Some(true)).muted, LIGHT.muted);
        // default is dark
        assert_eq!(resolve_with("", None, Some(false)).muted, DARK.muted);
        // bad config falls through to default
        assert_eq!(resolve_with("bogus", None, None).muted, DARK.muted);
    }

    #[test]
    fn builtin_themes_use_fixed_rgb() {
        // Built-in themes must not use ANSI named colors, or they'd depend on the
        // terminal palette again.
        for t in [&DARK, &LIGHT] {
            let mut all = vec![
                t.accent,
                t.accent_alt,
                t.error,
                t.warning,
                t.success,
                t.muted,
            ];
            all.extend_from_slice(&t.senders);
            for c in all {
                assert!(
                    matches!(c, Color::Rgb(..)),
                    "built-in color must be RGB, got {c:?}"
                );
            }
        }
    }
}
