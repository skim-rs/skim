//! Handle the color theme
use std::sync::LazyLock;

use crate::options::SkimOptions;
use crossterm::style::{Attribute, Attributes, Color};

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
    fg:                   Option<Color>,
    bg:                   Option<Color>,
    normal_effect:        Attributes,
    matched:              Option<Color>,
    matched_bg:           Option<Color>,
    matched_effect:       Attributes,
    current:              Option<Color>,
    current_bg:           Option<Color>,
    current_effect:       Attributes,
    current_match:        Option<Color>,
    current_match_bg:     Option<Color>,
    current_match_effect: Attributes,
    query_fg:             Option<Color>,
    query_bg:             Option<Color>,
    query_effect:         Attributes,
    spinner:              Option<Color>,
    info:                 Option<Color>,
    prompt:               Option<Color>,
    cursor:               Option<Color>,
    selected:             Option<Color>,
    header:               Option<Color>,
    border:               Option<Color>,
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
            fg:                   None,
            bg:                   None,
            normal_effect:        Attributes::none(),
            matched:              None,
            matched_bg:           None,
            matched_effect:       Attributes::none(),
            current:              None,
            current_bg:           None,
            current_effect:       Attributes::none(),
            current_match:        None,
            current_match_bg:     None,
            current_match_effect: Attributes::none(),
            query_fg:             None,
            query_bg:             None,
            query_effect:         Attributes::none(),
            spinner:              None,
            info:                 None,
            prompt:               None,
            cursor:               None,
            selected:             None,
            header:               None,
            border:               None,
        }
    }

    fn bw() -> Self {
        ColorTheme {
            matched_effect:       Attributes::none().with(Attribute::Underlined),
            current_effect:       Attributes::none().with(Attribute::Reverse),
            current_match_effect: Attributes::none().with(Attribute::Underlined).with(Attribute::Reverse),
            ..ColorTheme::empty()
        }
    }

    fn default16() -> Self {
        ColorTheme {
            matched:          Some(Color::Green),
            matched_bg:       Some(Color::Black),
            current:          Some(Color::Yellow),
            current_bg:       Some(Color::Black),
            current_match:    Some(Color::Green),
            current_match_bg: Some(Color::Black),
            spinner:          Some(Color::Green),
            info:             Some(Color::White),
            prompt:           Some(Color::Blue),
            cursor:           Some(Color::Red),
            selected:         Some(Color::Magenta),
            header:           Some(Color::Cyan),
            border:           Some(Color::Grey),
            ..ColorTheme::empty()
        }
    }

    fn dark256() -> Self {
        ColorTheme {
            matched:          Color::AnsiValue(108),
            matched_bg:       Color::AnsiValue(0),
            current:          Color::AnsiValue(254),
            current_bg:       Color::AnsiValue(236),
            current_match:    Color::AnsiValue(151),
            current_match_bg: Color::AnsiValue(236),
            spinner:          Color::AnsiValue(148),
            info:             Color::AnsiValue(144),
            prompt:           Color::AnsiValue(110),
            cursor:           Color::AnsiValue(161),
            selected:         Color::AnsiValue(168),
            header:           Color::AnsiValue(109),
            border:           Color::AnsiValue(59),
            ..ColorTheme::empty()
        }
    }

    fn molokai256() -> Self {
        ColorTheme {
            matched:          Color::AnsiValue(234),
            matched_bg:       Color::AnsiValue(186),
            current:          Color::AnsiValue(254),
            current_bg:       Color::AnsiValue(236),
            current_match:    Color::AnsiValue(234),
            current_match_bg: Color::AnsiValue(186),
            spinner:          Color::AnsiValue(148),
            info:             Color::AnsiValue(144),
            prompt:           Color::AnsiValue(110),
            cursor:           Color::AnsiValue(161),
            selected:         Color::AnsiValue(168),
            header:           Color::AnsiValue(109),
            border:           Color::AnsiValue(59),
            ..ColorTheme::empty()
        }
    }

    fn light256() -> Self {
        ColorTheme {
            matched:          Some(Color::AnsiValue(0)),
            matched_bg:       Some(Color::AnsiValue(220)),
            current:          Some(Color::AnsiValue(237)),
            current_bg:       Some(Color::AnsiValue(251)),
            current_match:    Some(Color::AnsiValue(66)),
            current_match_bg: Some(Color::AnsiValue(251)),
            spinner:          Some(Color::AnsiValue(65)),
            info:             Some(Color::AnsiValue(101)),
            prompt:           Some(Color::AnsiValue(25)),
            cursor:           Some(Color::AnsiValue(161)),
            selected:         Some(Color::AnsiValue(168)),
            header:           Some(Color::AnsiValue(31)),
            border:           Some(Color::AnsiValue(145)),
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
                Some(Color::Rgb { r, g, b })
            } else {
                let Ok(c) = color[1].parse::<u8>()
                    .map(Color::AnsiValue) else {
                    None
                };
                Some(c)
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

    pub fn normal(&self) -> Attributes {
        Attr {
            fg: self.fg,
            bg: self.bg,
            effect: self.normal_effect,
        }
    }

    pub fn matched(&self) -> Attr {
        Attr {
            fg: self.matched,
            bg: self.matched_bg,
            effect: self.matched_effect,
        }
    }

    pub fn current(&self) -> Attr {
        Attr {
            fg: self.current,
            bg: self.current_bg,
            effect: self.current_effect,
        }
    }

    pub fn current_match(&self) -> Attr {
        Attr {
            fg: self.current_match,
            bg: self.current_match_bg,
            effect: self.current_match_effect,
        }
    }

    pub fn query(&self) -> Attr {
        Attr {
            fg: self.query_fg,
            bg: self.query_bg,
            effect: self.query_effect,
        }
    }

    pub fn spinner(&self) -> Attr {
        Attr {
            fg: self.spinner,
            bg: self.bg,
            effect: Effect::BOLD,
        }
    }

    pub fn info(&self) -> Attr {
        Attr {
            fg: self.info,
            bg: self.bg,
            effect: Effect::empty(),
        }
    }

    pub fn prompt(&self) -> Attr {
        Attr {
            fg: self.prompt,
            bg: self.bg,
            effect: Effect::empty(),
        }
    }

    pub fn cursor(&self) -> Attr {
        Attr {
            fg: self.cursor,
            bg: self.current_bg,
            effect: Effect::empty(),
        }
    }

    pub fn selected(&self) -> Attr {
        Attr {
            fg: self.selected,
            bg: self.current_bg,
            effect: Effect::empty(),
        }
    }

    pub fn header(&self) -> Attr {
        Attr {
            fg: self.header,
            bg: self.bg,
            effect: Effect::empty(),
        }
    }

    pub fn border(&self) -> Attr {
        Attr {
            fg: self.border,
            bg: self.bg,
            effect: Effect::empty(),
        }
    }
}
