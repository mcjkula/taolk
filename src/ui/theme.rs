use crate::config::{ColorMode, ThemeChoice};
use ratatui::style::Color;

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub border: Color,
    pub border_focus: Color,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

pub const MOCHA: Theme = Theme {
    border: rgb(0x45, 0x47, 0x5a),
    border_focus: rgb(0x89, 0xb4, 0xfa),
};

pub const LATTE: Theme = Theme {
    border: rgb(0xbc, 0xc0, 0xcc),
    border_focus: rgb(0x1e, 0x66, 0xf5),
};

pub const TOKYO_NIGHT: Theme = Theme {
    border: rgb(0x29, 0x2e, 0x42),
    border_focus: rgb(0x7a, 0xa2, 0xf7),
};

pub const GRUVBOX_DARK: Theme = Theme {
    border: rgb(0x50, 0x49, 0x45),
    border_focus: rgb(0xfa, 0xbd, 0x2f),
};

pub const ROSE_PINE: Theme = Theme {
    border: rgb(0x26, 0x23, 0x3a),
    border_focus: rgb(0xc4, 0xa7, 0xe7),
};

pub const MONOCHROME: Theme = Theme {
    border: Color::Reset,
    border_focus: Color::Reset,
};

pub const fn theme_for(choice: ThemeChoice) -> &'static Theme {
    match choice {
        ThemeChoice::Mocha => &MOCHA,
        ThemeChoice::Latte => &LATTE,
        ThemeChoice::TokyoNight => &TOKYO_NIGHT,
        ThemeChoice::GruvboxDark => &GRUVBOX_DARK,
        ThemeChoice::RosePine => &ROSE_PINE,
        ThemeChoice::Monochrome => &MONOCHROME,
    }
}

pub fn apply_mode(mode: ColorMode, c: Color) -> Color {
    match mode {
        ColorMode::TrueColor => c,
        ColorMode::Ansi256 => quantize_256(c),
        ColorMode::Mono => Color::Reset,
    }
}

fn quantize_256(c: Color) -> Color {
    match c {
        Color::Rgb(r, g, b) => {
            let q = |v: u8| -> u8 {
                if v < 48 {
                    0
                } else if v < 115 {
                    1
                } else {
                    (((u16::from(v) - 35) / 40).min(5)) as u8
                }
            };
            Color::Indexed(16 + 36 * q(r) + 6 * q(g) + q(b))
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_choice_maps_to_its_theme() {
        for (choice, theme) in [
            (ThemeChoice::Mocha, &MOCHA),
            (ThemeChoice::Latte, &LATTE),
            (ThemeChoice::TokyoNight, &TOKYO_NIGHT),
            (ThemeChoice::GruvboxDark, &GRUVBOX_DARK),
            (ThemeChoice::RosePine, &ROSE_PINE),
            (ThemeChoice::Monochrome, &MONOCHROME),
        ] {
            assert!(std::ptr::eq(theme_for(choice), theme));
        }
    }

    #[test]
    fn true_color_is_passthrough() {
        let c = Color::Rgb(0x12, 0x34, 0x56);
        assert_eq!(apply_mode(ColorMode::TrueColor, c), c);
    }

    #[test]
    fn mono_maps_rgb_to_reset() {
        assert_eq!(
            apply_mode(ColorMode::Mono, Color::Rgb(0x12, 0x34, 0x56)),
            Color::Reset
        );
    }

    #[test]
    fn ansi256_quantizes_rgb_to_indexed() {
        let c = apply_mode(ColorMode::Ansi256, Color::Rgb(0xff, 0x00, 0x00));
        assert!(matches!(c, Color::Indexed(_)));
    }

    #[test]
    fn ansi256_leaves_non_rgb_alone() {
        assert_eq!(apply_mode(ColorMode::Ansi256, Color::Reset), Color::Reset);
    }
}
