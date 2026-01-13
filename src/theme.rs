//! Handle the color theme
use std::sync::LazyLock;

use ratatui::style::{Color, Modifier, Style};

use crate::options::SkimOptions;

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
#[derive(Copy, Clone, Debug, Default)]
pub struct ColorTheme {
    fg:                   Color,
    bg:                   Color,
    normal_effect:        Modifier,
    matched:              Color,
    matched_bg:           Color,
    matched_effect:       Modifier,
    current:              Color,
    current_bg:           Color,
    current_effect:       Modifier,
    current_match:        Color,
    current_match_bg:     Color,
    current_match_effect: Modifier,
    query_fg:             Color,
    query_bg:             Color,
    query_effect:         Modifier,
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
            // Check for NO_COLOR environment variable
            match std::env::var_os("NO_COLOR") {
                Some(no_color) if !no_color.is_empty() => ColorTheme::none(),
                _ => ColorTheme::dark256(),
            }
        }
    }

    fn none() -> Self {
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
            ..ColorTheme::none()
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
            border:           Color::Black,
            ..ColorTheme::none()
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
            ..ColorTheme::none()
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
            ..ColorTheme::none()
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
            ..ColorTheme::none()
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
                    "none" | "empty" => ColorTheme::none(),
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

    pub fn normal(&self) -> Style {
        Style::new()
            .fg(self.fg)
            .bg(self.bg)
            .add_modifier(self.normal_effect)
    }

    pub fn matched(&self) -> Style {
        Style::new()
            .fg(self.matched)
            .bg(self.matched_bg)
            .add_modifier(self.matched_effect)
    }

    pub fn current(&self) -> Style {
        Style::new()
            .fg(self.current)
            .bg(self.current_bg)
            .add_modifier(self.current_effect)
    }

    pub fn current_match(&self) -> Style {
        Style::new()
            .fg(self.current_match)
            .bg(self.current_match_bg)
            .add_modifier(self.current_match_effect)
    }

    pub fn query(&self) -> Style {
        Style::new()
            .fg(self.query_fg)
            .bg(self.query_bg)
            .add_modifier(self.query_effect)
    }

    pub fn spinner(&self) -> Style {
        Style::new()
            .fg(self.spinner)
            .bg(self.bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn info(&self) -> Style {
        Style::new()
            .fg(self.info)
            .bg(self.bg)
            .add_modifier(Modifier::empty())
    }

    pub fn prompt(&self) -> Style {
        Style::new()
            .fg(self.prompt)
            .bg(self.bg)
            .add_modifier(Modifier::empty())
    }

    pub fn cursor(&self) -> Style {
        Style::new()
            .fg(self.cursor)
            .bg(self.current_bg)
            .add_modifier(Modifier::empty())
    }

    pub fn selected(&self) -> Style {
        Style::new()
            .fg(self.selected)
            .bg(self.current_bg)
            .add_modifier(Modifier::empty())
    }

    pub fn header(&self) -> Style {
        Style::new()
            .fg(self.header)
            .bg(self.bg)
            .add_modifier(Modifier::empty())
    }

    pub fn border(&self) -> Style {
        Style::new()
            .fg(self.border)
            .bg(self.bg)
            .add_modifier(Modifier::empty())
    }
}
