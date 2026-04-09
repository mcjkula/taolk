use crate::config::{ColorMode, ThemeChoice};
use ratatui::style::Color;

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub border: Color,
    pub border_focus: Color,
    pub text: Color,
    pub text_dim: Color,
    pub text_strong: Color,
    pub accent: Color,
    pub timestamp: Color,
    pub sender_rotation: [Color; 8],
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

pub const MOCHA: Theme = Theme {
    border: rgb(0x45, 0x47, 0x5a),
    border_focus: rgb(0x89, 0xb4, 0xfa),
    text: rgb(0xcd, 0xd6, 0xf4),
    text_dim: rgb(0x6c, 0x70, 0x86),
    text_strong: rgb(0xf5, 0xe0, 0xdc),
    accent: rgb(0x89, 0xb4, 0xfa),
    timestamp: rgb(0x6c, 0x70, 0x86),
    sender_rotation: [
        rgb(0x89, 0xb4, 0xfa),
        rgb(0xa6, 0xe3, 0xa1),
        rgb(0xf5, 0xc2, 0xe7),
        rgb(0xf9, 0xe2, 0xaf),
        rgb(0x94, 0xe2, 0xd5),
        rgb(0xcb, 0xa6, 0xf7),
        rgb(0xfa, 0xb3, 0x87),
        rgb(0xf3, 0x8b, 0xa8),
    ],
};

pub const LATTE: Theme = Theme {
    border: rgb(0xbc, 0xc0, 0xcc),
    border_focus: rgb(0x1e, 0x66, 0xf5),
    text: rgb(0x4c, 0x4f, 0x69),
    text_dim: rgb(0x9c, 0xa0, 0xb0),
    text_strong: rgb(0x11, 0x11, 0x1b),
    accent: rgb(0x1e, 0x66, 0xf5),
    timestamp: rgb(0x9c, 0xa0, 0xb0),
    sender_rotation: [
        rgb(0x1e, 0x66, 0xf5),
        rgb(0x40, 0xa0, 0x2b),
        rgb(0xea, 0x76, 0xcb),
        rgb(0xdf, 0x8e, 0x1d),
        rgb(0x17, 0x92, 0x99),
        rgb(0x88, 0x39, 0xef),
        rgb(0xfe, 0x64, 0x0b),
        rgb(0xd2, 0x0f, 0x39),
    ],
};

pub const TOKYO_NIGHT: Theme = Theme {
    border: rgb(0x29, 0x2e, 0x42),
    border_focus: rgb(0x7a, 0xa2, 0xf7),
    text: rgb(0xc0, 0xca, 0xf5),
    text_dim: rgb(0x56, 0x5f, 0x89),
    text_strong: rgb(0xff, 0xff, 0xff),
    accent: rgb(0x7a, 0xa2, 0xf7),
    timestamp: rgb(0x56, 0x5f, 0x89),
    sender_rotation: [
        rgb(0x7a, 0xa2, 0xf7),
        rgb(0x9e, 0xce, 0x6a),
        rgb(0xbb, 0x9a, 0xf7),
        rgb(0xe0, 0xaf, 0x68),
        rgb(0x7d, 0xcf, 0xff),
        rgb(0xff, 0x9e, 0x64),
        rgb(0xf7, 0x76, 0x8e),
        rgb(0x73, 0xda, 0xca),
    ],
};

pub const GRUVBOX_DARK: Theme = Theme {
    border: rgb(0x50, 0x49, 0x45),
    border_focus: rgb(0xfa, 0xbd, 0x2f),
    text: rgb(0xeb, 0xdb, 0xb2),
    text_dim: rgb(0x92, 0x83, 0x74),
    text_strong: rgb(0xfb, 0xf1, 0xc7),
    accent: rgb(0xfa, 0xbd, 0x2f),
    timestamp: rgb(0x92, 0x83, 0x74),
    sender_rotation: [
        rgb(0x83, 0xa5, 0x98),
        rgb(0xb8, 0xbb, 0x26),
        rgb(0xd3, 0x86, 0x9b),
        rgb(0xfa, 0xbd, 0x2f),
        rgb(0x8e, 0xc0, 0x7c),
        rgb(0xfe, 0x80, 0x19),
        rgb(0xfb, 0x49, 0x34),
        rgb(0xeb, 0xdb, 0xb2),
    ],
};

pub const ROSE_PINE: Theme = Theme {
    border: rgb(0x26, 0x23, 0x3a),
    border_focus: rgb(0xc4, 0xa7, 0xe7),
    text: rgb(0xe0, 0xde, 0xf4),
    text_dim: rgb(0x6e, 0x6a, 0x86),
    text_strong: rgb(0xff, 0xff, 0xff),
    accent: rgb(0xc4, 0xa7, 0xe7),
    timestamp: rgb(0x6e, 0x6a, 0x86),
    sender_rotation: [
        rgb(0xc4, 0xa7, 0xe7),
        rgb(0x9c, 0xcf, 0xd8),
        rgb(0xeb, 0xbc, 0xba),
        rgb(0xf6, 0xc1, 0x77),
        rgb(0x31, 0x74, 0x8f),
        rgb(0xeb, 0x6f, 0x92),
        rgb(0xe0, 0xde, 0xf4),
        rgb(0x90, 0x8c, 0xaa),
    ],
};

pub const MONOCHROME: Theme = Theme {
    border: Color::Reset,
    border_focus: Color::Reset,
    text: Color::Reset,
    text_dim: Color::Reset,
    text_strong: Color::Reset,
    accent: Color::Reset,
    timestamp: Color::Reset,
    sender_rotation: [Color::Reset; 8],
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
        for (choice, expected_border) in [
            (ThemeChoice::Mocha, MOCHA.border),
            (ThemeChoice::Latte, LATTE.border),
            (ThemeChoice::TokyoNight, TOKYO_NIGHT.border),
            (ThemeChoice::GruvboxDark, GRUVBOX_DARK.border),
            (ThemeChoice::RosePine, ROSE_PINE.border),
            (ThemeChoice::Monochrome, MONOCHROME.border),
        ] {
            assert_eq!(theme_for(choice).border, expected_border);
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
