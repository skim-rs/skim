//! Handle the color theme
use std::sync::LazyLock;

use crate::options::SkimOptions;
use crate::ui::tuikit_compat::*;
use ratatui::style::{Color, Modifier};

pub static DEFAULT_THEME: LazyLock<ColorTheme> = LazyLock::new(ColorTheme::dark256);

/// The color scheme of skim's UI
///
/// <pre>
/// +----------------+
/// | >selected line |  --> selected & normal(fg/bg) & matched
/// |> current line  |  --> cursor & current & current_match
/// |  normal line   |
/// |\ 8/10          |  --> spinner & info
/// |> query         |  --> prompt & query
/// +----------------+
/// </pre>
#[rustfmt::skip]
#[derive(Copy, Clone, Debug)]
pub struct ColorTheme {
    fg:                   Color,
    bg:                   Color,
    normal_effect:        Effect,
    matched:              Color,
    matched_bg:           Color,
    matched_effect:       Effect,
    current:              Color,
    current_bg:           Color,
    current_effect:       Effect,
    current_match:        Color,
    current_match_bg:     Color,
    current_match_effect: Effect,
    query_fg:             Color,
    query_bg:             Color,
    query_effect:         Effect,
    spinner:              Color,
    info:                 Color,
    prompt:               Color,
    cursor:               Color,
    selected:             Color,
    header:               Color,
    border:               Color,
}

#[rustfmt::skip]
#[allow(dead_code)]
impl ColorTheme {
    pub fn init_from_options(options: &SkimOptions) -> ColorTheme {
        // register
        if let Some(color) = options.color.clone() {
            ColorTheme::from_options(&color)
        } else {
            ColorTheme::dark256()
        }
    }

    fn empty() -> Self {
        ColorTheme {
            fg:                   Color::Reset,
            bg:                   Color::Reset,
            normal_effect:        Modifier::empty(),
            matched:              Color::Reset,
            matched_bg:           Color::Reset,
            matched_effect:       Modifier::empty(),
            current:              Color::Reset,
            current_bg:           Color::Reset,
            current_effect:       Modifier::empty(),
            current_match:        Color::Reset,
            current_match_bg:     Color::Reset,
            current_match_effect: Modifier::empty(),
            query_fg:             Color::Reset,
            query_bg:             Color::Reset,
            query_effect:         Modifier::empty(),
            spinner:              Color::Reset,
            info:                 Color::Reset,
            prompt:               Color::Reset,
            cursor:               Color::Reset,
            selected:             Color::Reset,
            header:               Color::Reset,
            border:               Color::Reset,
        }
    }

    fn bw() -> Self {
        ColorTheme {
            matched_effect:       Modifier::UNDERLINED,
            current_effect:       Modifier::REVERSED,
            current_match_effect: Modifier::UNDERLINED | Modifier::REVERSED,
            ..ColorTheme::empty()
        }
    }

    fn default16() -> Self {
        ColorTheme {
            matched:          Color::Green,
            matched_bg:       Color::Black,
            current:          Color::Yellow,
            current_bg:       Color::Black,
            current_match:    Color::Green,
            current_match_bg: Color::Black,
            spinner:          Color::Green,
            info:             Color::White,
            prompt:           Color::Blue,
            cursor:           Color::Red,
            selected:         Color::Magenta,
            header:           Color::Cyan,
            border:           Color::DarkGray,
            ..ColorTheme::empty()
        }
    }

    fn dark256() -> Self {
        ColorTheme {
            matched:          Color::Indexed(108),
            matched_bg:       Color::Indexed(0),
            current:          Color::Indexed(254),
            current_bg:       Color::Indexed(236),
            current_match:    Color::Indexed(151),
            current_match_bg: Color::Indexed(236),
            spinner:          Color::Indexed(148),
            info:             Color::Indexed(144),
            prompt:           Color::Indexed(110),
            cursor:           Color::Indexed(161),
            selected:         Color::Indexed(168),
            header:           Color::Indexed(109),
            border:           Color::Indexed(59),
            ..ColorTheme::empty()
        }
    }

    fn molokai256() -> Self {
        ColorTheme {
            matched:          Color::Indexed(234),
            matched_bg:       Color::Indexed(186),
            current:          Color::Indexed(254),
            current_bg:       Color::Indexed(236),
            current_match:    Color::Indexed(234),
            current_match_bg: Color::Indexed(186),
            spinner:          Color::Indexed(148),
            info:             Color::Indexed(144),
            prompt:           Color::Indexed(110),
            cursor:           Color::Indexed(161),
            selected:         Color::Indexed(168),
            header:           Color::Indexed(109),
            border:           Color::Indexed(59),
            ..ColorTheme::empty()
        }
    }

    fn light256() -> Self {
        ColorTheme {
            matched:          Color::Indexed(0),
            matched_bg:       Color::Indexed(220),
            current:          Color::Indexed(237),
            current_bg:       Color::Indexed(251),
            current_match:    Color::Indexed(66),
            current_match_bg: Color::Indexed(251),
            spinner:          Color::Indexed(65),
            info:             Color::Indexed(101),
            prompt:           Color::Indexed(25),
            cursor:           Color::Indexed(161),
            selected:         Color::Indexed(168),
            header:           Color::Indexed(31),
            border:           Color::Indexed(145),
            ..ColorTheme::empty()
        }
    }

    #[allow(clippy::wildcard_in_or_patterns)]
    fn from_options(color: &str) -> Self {
        let mut theme = ColorTheme::dark256();
        for pair in color.split(',') {
            let color: Vec<&str> = pair.split(':').collect();
            if color.len() < 2 {
                theme = match color[0] {
                    "molokai"  => ColorTheme::molokai256(),
                    "light"    => ColorTheme::light256(),
                    "16"       => ColorTheme::default16(),
                    "bw"       => ColorTheme::bw(),
                    "empty"    => ColorTheme::empty(),
                    "dark" | "default" | _ => ColorTheme::dark256(),
                };
                continue;
            }

            let new_color = if color[1].len() == 7 {
                // 256 color
                let r = u8::from_str_radix(&color[1][1..3], 16).unwrap_or(255);
                let g = u8::from_str_radix(&color[1][3..5], 16).unwrap_or(255);
                let b = u8::from_str_radix(&color[1][5..7], 16).unwrap_or(255);
                Color::Rgb(r, g, b)
            } else {
                color[1].parse::<u8>()
                    .map(Color::Indexed)
                    .unwrap_or(Color::Reset)
            };

            match color[0] {
                "fg"                    => theme.fg               = new_color,
                "bg"                    => theme.bg               = new_color,
                "matched" | "hl"        => theme.matched          = new_color,
                "matched_bg"            => theme.matched_bg       = new_color,
                "current" | "fg+"       => theme.current          = new_color,
                "current_bg" | "bg+"    => theme.current_bg       = new_color,
                "current_match" | "hl+" => theme.current_match    = new_color,
                "current_match_bg"      => theme.current_match_bg = new_color,
                "query"                 => theme.query_fg         = new_color,
                "query_bg"              => theme.query_bg         = new_color,
                "spinner"               => theme.spinner          = new_color,
                "info"                  => theme.info             = new_color,
                "prompt"                => theme.prompt           = new_color,
                "cursor" | "pointer"    => theme.cursor           = new_color,
                "selected" | "marker"   => theme.selected         = new_color,
                "header"                => theme.header           = new_color,
                "border"                => theme.border           = new_color,
                _ => {}
            }
        }
        theme
    }

    pub fn normal(&self) -> Attr {
        Attr {
            fg: Some(self.fg),
            bg: Some(self.bg),
            modifiers: self.normal_effect,
            effect: self.normal_effect,
        }
    }

    pub fn matched(&self) -> Attr {
        Attr {
            fg: Some(self.matched),
            bg: Some(self.matched_bg),
            modifiers: self.matched_effect,
            effect: self.matched_effect,
        }
    }

    pub fn current(&self) -> Attr {
        Attr {
            fg: Some(self.current),
            bg: Some(self.current_bg),
            modifiers: self.current_effect,
            effect: self.current_effect,
        }
    }

    pub fn current_match(&self) -> Attr {
        Attr {
            fg: Some(self.current_match),
            bg: Some(self.current_match_bg),
            modifiers: self.current_match_effect,
            effect: self.current_match_effect,
        }
    }

    pub fn query(&self) -> Attr {
        Attr {
            fg: Some(self.query_fg),
            bg: Some(self.query_bg),
            modifiers: self.query_effect,
            effect: self.query_effect,
        }
    }

    pub fn spinner(&self) -> Attr {
        Attr {
            fg: Some(self.spinner),
            bg: Some(self.bg),
            modifiers: Modifier::BOLD,
            effect: Modifier::BOLD,
        }
    }

    pub fn info(&self) -> Attr {
        Attr {
            fg: Some(self.info),
            bg: Some(self.bg),
            modifiers: Modifier::empty(),
            effect: Modifier::empty(),
        }
    }

    pub fn prompt(&self) -> Attr {
        Attr {
            fg: Some(self.prompt),
            bg: Some(self.bg),
            modifiers: Modifier::empty(),
            effect: Modifier::empty(),
        }
    }

    pub fn cursor(&self) -> Attr {
        Attr {
            fg: Some(self.cursor),
            bg: Some(self.current_bg),
            modifiers: Modifier::empty(),
            effect: Modifier::empty(),
        }
    }

    pub fn selected(&self) -> Attr {
        Attr {
            fg: Some(self.selected),
            bg: Some(self.current_bg),
            modifiers: Modifier::empty(),
            effect: Modifier::empty(),
        }
    }

    pub fn header(&self) -> Attr {
        Attr {
            fg: Some(self.header),
            bg: Some(self.bg),
            modifiers: Modifier::empty(),
            effect: Modifier::empty(),
        }
    }

    pub fn border(&self) -> Attr {
        Attr {
            fg: Some(self.border),
            bg: Some(self.bg),
            modifiers: Modifier::empty(),
            effect: Modifier::empty(),
        }
    }
}
