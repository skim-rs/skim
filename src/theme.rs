//! Handle the color theme
use std::sync::LazyLock;

use ratatui::style::{Color, Modifier, Style};

use crate::options::SkimOptions;

/// Theme defaults to Dark256
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
#[derive(Copy, Clone, Debug, Default)]
pub struct ColorTheme {
    /// Non-selected lines and general text
    pub normal: Style,
    /// Matched text on non-current lines
    pub matched: Style,
    /// Current line, non-matched text
    pub current: Style,
    /// Current line, matched text
    pub current_match: Style,
    /// Query text/input
    pub query: Style,
    /// Spinner
    pub spinner: Style,
    /// Info (outside of spinner)
    pub info: Style,
    /// Prompt prefix
    pub prompt: Style,
    /// Cursor/Selector/pointer (prefix of current item)
    pub cursor: Style,
    /// Multi-selector/marker (prefix of selected items)
    pub selected: Style,
    /// Header lines
    pub header: Style,
    /// Border
    pub border: Style,
}

#[allow(dead_code)]
impl ColorTheme {
    /// Setup the theme from the skim options
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
        Self {
            spinner: Style::default().bold(),
            ..ColorTheme::default()
        }
    }

    fn bw() -> Self {
        let base = ColorTheme::none();
        ColorTheme {
            matched: base.matched.underlined(),
            current: base.current.reversed(),
            current_match: base.current_match.reversed().underlined(),
            ..base
        }
    }

    fn default16() -> Self {
        let base = ColorTheme::none();
        ColorTheme {
            matched: base.matched.fg(Color::Green),
            current: base.current.fg(Color::Yellow),
            current_match: base.current_match.fg(Color::Green),
            spinner: base.spinner.fg(Color::Green),
            info: base.info.fg(Color::White),
            prompt: base.prompt.fg(Color::Blue),
            cursor: base.cursor.fg(Color::Red),
            selected: base.selected.fg(Color::Magenta),
            header: base.header.fg(Color::Cyan),
            border: base.border.fg(Color::Black),
            ..base
        }
    }

    fn dark256() -> Self {
        let base = ColorTheme::none();
        ColorTheme {
            matched: base.matched.fg(Color::Indexed(108)).bg(Color::Indexed(0)),
            current: base.current.bg(Color::Indexed(236)),
            current_match: base.current_match.fg(Color::Indexed(151)).bg(Color::Indexed(236)),
            spinner: base.spinner.fg(Color::Indexed(148)),
            info: base.info.fg(Color::Indexed(144)),
            prompt: base.prompt.fg(Color::Indexed(110)),
            cursor: base.cursor.fg(Color::Indexed(161)),
            selected: base.selected.fg(Color::Indexed(168)),
            header: base.header.fg(Color::Indexed(109)),
            border: base.border.fg(Color::Indexed(59)),
            ..base
        }
    }

    fn molokai256() -> Self {
        let base = ColorTheme::none();
        ColorTheme {
            matched: base.matched.fg(Color::Indexed(234)).bg(Color::Indexed(186)),
            current: base.current.bg(Color::Indexed(236)),
            current_match: base.current_match.fg(Color::Indexed(234)).bg(Color::Indexed(186)),
            spinner: base.spinner.fg(Color::Indexed(148)),
            info: base.info.fg(Color::Indexed(144)),
            prompt: base.prompt.fg(Color::Indexed(110)),
            cursor: base.cursor.fg(Color::Indexed(161)),
            selected: base.selected.fg(Color::Indexed(168)),
            header: base.header.fg(Color::Indexed(109)),
            border: base.border.fg(Color::Indexed(59)),
            ..base
        }
    }

    fn light256() -> Self {
        let base = ColorTheme::none();
        ColorTheme {
            matched: base.matched.fg(Color::Indexed(0)).bg(Color::Indexed(220)),
            current: base.current.bg(Color::Indexed(251)),
            current_match: base.current_match.fg(Color::Indexed(66)).bg(Color::Indexed(251)),
            spinner: base.spinner.fg(Color::Indexed(65)),
            info: base.info.fg(Color::Indexed(101)),
            prompt: base.prompt.fg(Color::Indexed(25)),
            cursor: base.cursor.fg(Color::Indexed(161)),
            selected: base.selected.fg(Color::Indexed(168)),
            header: base.header.fg(Color::Indexed(31)),
            border: base.border.fg(Color::Indexed(145)),
            ..base
        }
    }

    fn set_color(&mut self, name: &str, spec: &str) {
        let spec_parts: Vec<_> = spec.split(&['+', ':']).collect();

        // Compute color
        let raw_color = spec_parts[0];
        let new_color = if raw_color.len() == 7 && raw_color.starts_with('#') {
            // RGB Hex color
            let r = u8::from_str_radix(&raw_color[1..3], 16).unwrap_or(255);
            let g = u8::from_str_radix(&raw_color[3..5], 16).unwrap_or(255);
            let b = u8::from_str_radix(&raw_color[5..7], 16).unwrap_or(255);
            Some(Color::Rgb(r, g, b))
        } else {
            raw_color.parse::<u8>().ok().map(Color::Indexed).or_else(|| {
                debug!("Unknown color '{}'", spec_parts[0]);
                None
            })
        };

        // Compute modifiers
        let mut modifier = Modifier::empty();
        for part in spec_parts.iter().skip(1) {
            if matches!(*part, "x" | "regular") {
                modifier = Modifier::empty()
            } else {
                modifier |= match *part {
                    "b" | "bold" => Modifier::BOLD,
                    "u" | "underlined" => Modifier::UNDERLINED,
                    "c" | "crossed-out" => Modifier::CROSSED_OUT,
                    "d" | "dim" => Modifier::DIM,
                    "i" | "italic" => Modifier::ITALIC,
                    "r" | "reverse" => Modifier::REVERSED,
                    m => {
                        debug!("Unknown modifier '{m}'");
                        Modifier::empty()
                    }
                };
            }
        }
        // Apply - check for layer suffixes (_fg, -fg, _bg, -bg, _u, -u, etc.)
        let (component_name, layer) = if name.ends_with("_fg") || name.ends_with("-fg") {
            (&name[..name.len() - 3], "fg")
        } else if name.ends_with("_bg") || name.ends_with("-bg") {
            (&name[..name.len() - 3], "bg")
        } else if name.ends_with("_u") || name.ends_with("-u") {
            (&name[..name.len() - 2], "u")
        } else if name.ends_with("_underline") || name.ends_with("-underline") {
            (&name[..name.len() - 10], "underline")
        } else if name == "bg" {
            ("", "bg")
        } else {
            (name, "fg")
        };

        match component_name {
            "" | "normal" => {
                set_style(&mut self.normal, layer, new_color, modifier);
            }
            "matched" | "hl" => {
                set_style(&mut self.matched, layer, new_color, modifier);
            }
            "current" | "fg+" => {
                set_style(&mut self.current, layer, new_color, modifier);
            }
            "bg+" => {
                set_style(&mut self.current, "bg", new_color, modifier);
            }
            "current_match" | "hl+" => {
                set_style(&mut self.current_match, layer, new_color, modifier);
            }
            "query" => {
                set_style(&mut self.query, layer, new_color, modifier);
            }
            "spinner" => {
                set_style(&mut self.spinner, layer, new_color, modifier);
            }
            "info" => {
                set_style(&mut self.info, layer, new_color, modifier);
            }
            "prompt" => {
                set_style(&mut self.prompt, layer, new_color, modifier);
            }
            "cursor" | "pointer" => {
                set_style(&mut self.cursor, layer, new_color, modifier);
            }
            "selected" | "marker" => {
                set_style(&mut self.selected, layer, new_color, modifier);
            }
            "header" => {
                set_style(&mut self.header, layer, new_color, modifier);
            }
            "border" => {
                set_style(&mut self.border, layer, new_color, modifier);
            }
            _ => {}
        }
    }

    fn from_options(color: &str) -> Self {
        let mut theme = ColorTheme::dark256();
        for pair in color.split(',') {
            if let Some((name, spec)) = pair.split_once(':') {
                theme.set_color(name, spec);
            } else {
                theme = match color {
                    "molokai" => ColorTheme::molokai256(),
                    "light" => ColorTheme::light256(),
                    "16" => ColorTheme::default16(),
                    "bw" => ColorTheme::bw(),
                    "none" | "empty" => ColorTheme::none(),
                    "dark" | "default" => ColorTheme::dark256(),
                    t => {
                        debug!("Unknown color theme '{t}'");
                        ColorTheme::dark256()
                    }
                };
            }
        }
        theme
    }
}

fn set_style(s: &mut Style, layer: &str, color: Option<Color>, modifier: Modifier) {
    if let Some(c) = color {
        *s = match layer {
            "fg" => s.fg(c),
            "bg" => s.bg(c),
            "u" | "underline" => s.underline_color(c),
            _ => *s,
        }
    }
    *s = s.add_modifier(modifier);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_themes() {
        // Test that base themes have expected properties
        let none = ColorTheme::none();
        // Spinner should be bold even in none theme
        assert!(none.spinner.add_modifier.contains(Modifier::BOLD));

        let bw = ColorTheme::bw();
        assert!(bw.matched.add_modifier.contains(Modifier::UNDERLINED));
        assert!(bw.current.add_modifier.contains(Modifier::REVERSED));

        let theme_16 = ColorTheme::default16();
        assert_eq!(theme_16.matched.fg, Some(Color::Green));
        assert_eq!(theme_16.matched.bg, None);

        let dark = ColorTheme::dark256();
        assert_eq!(dark.matched.fg, Some(Color::Indexed(108)));
        assert_eq!(dark.matched.bg, Some(Color::Indexed(0)));

        let molokai = ColorTheme::molokai256();
        assert_eq!(molokai.matched.fg, Some(Color::Indexed(234)));
        assert_eq!(molokai.matched.bg, Some(Color::Indexed(186)));

        let light = ColorTheme::light256();
        assert_eq!(light.matched.fg, Some(Color::Indexed(0)));
        assert_eq!(light.matched.bg, Some(Color::Indexed(220)));
    }

    #[test]
    fn test_from_options_base_themes() {
        // Test base theme names
        let dark = ColorTheme::from_options("dark");
        assert!(dark.matched.fg.is_some());

        let molokai = ColorTheme::from_options("molokai");
        assert!(molokai.matched.fg.is_some());

        let light = ColorTheme::from_options("light");
        assert!(light.matched.fg.is_some());

        let theme_16 = ColorTheme::from_options("16");
        assert!(theme_16.matched.fg.is_some());

        let bw = ColorTheme::from_options("bw");
        assert!(bw.matched.add_modifier.contains(Modifier::UNDERLINED));

        // Test that "none" theme uses reset style (which may have default colors from terminal)
        let none = ColorTheme::from_options("none");
        // Spinner should still be bold even in none theme
        assert!(none.spinner.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_ansi_color_parsing() {
        // Test ANSI color (0-255)
        let theme = ColorTheme::from_options("matched:108");
        assert_eq!(theme.matched.fg, Some(Color::Indexed(108)));

        let theme = ColorTheme::from_options("prompt:25");
        assert_eq!(theme.prompt.fg, Some(Color::Indexed(25)));
    }

    #[test]
    fn test_rgb_hex_color_parsing() {
        // Test RGB hex color (#rrggbb)
        let theme = ColorTheme::from_options("matched:#ff0000");
        assert_eq!(theme.matched.fg, Some(Color::Rgb(255, 0, 0)));

        let theme = ColorTheme::from_options("prompt:#00ff00");
        assert_eq!(theme.prompt.fg, Some(Color::Rgb(0, 255, 0)));

        let theme = ColorTheme::from_options("info:#0000ff");
        assert_eq!(theme.info.fg, Some(Color::Rgb(0, 0, 255)));
    }

    #[test]
    fn test_color_with_modifiers() {
        // Test color with bold modifier
        let theme = ColorTheme::from_options("matched:108:bold");
        assert_eq!(theme.matched.fg, Some(Color::Indexed(108)));
        assert!(theme.matched.add_modifier.contains(Modifier::BOLD));

        // Test color with underline modifier
        let theme = ColorTheme::from_options("matched:108:underlined");
        assert_eq!(theme.matched.fg, Some(Color::Indexed(108)));
        assert!(theme.matched.add_modifier.contains(Modifier::UNDERLINED));

        // Test color with multiple modifiers (using +)
        let theme = ColorTheme::from_options("matched:108:bold:underlined");
        assert_eq!(theme.matched.fg, Some(Color::Indexed(108)));
        assert!(theme.matched.add_modifier.contains(Modifier::BOLD));
        assert!(theme.matched.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn test_modifier_shortcuts() {
        // Test short modifier names
        let theme = ColorTheme::from_options("matched:108:b");
        assert!(theme.matched.add_modifier.contains(Modifier::BOLD));

        let theme = ColorTheme::from_options("matched:108:u");
        assert!(theme.matched.add_modifier.contains(Modifier::UNDERLINED));

        let theme = ColorTheme::from_options("matched:108:i");
        assert!(theme.matched.add_modifier.contains(Modifier::ITALIC));

        let theme = ColorTheme::from_options("matched:108:r");
        assert!(theme.matched.add_modifier.contains(Modifier::REVERSED));

        let theme = ColorTheme::from_options("matched:108:d");
        assert!(theme.matched.add_modifier.contains(Modifier::DIM));

        let theme = ColorTheme::from_options("matched:108:c");
        assert!(theme.matched.add_modifier.contains(Modifier::CROSSED_OUT));
    }

    #[test]
    fn test_regular_modifier_reset() {
        // Test that 'regular' or 'x' resets modifiers
        let theme = ColorTheme::from_options("matched:108:x:bold");
        assert!(theme.matched.add_modifier.contains(Modifier::BOLD));
        assert!(!theme.matched.add_modifier.contains(Modifier::ITALIC));

        let theme = ColorTheme::from_options("matched:108:regular:underlined");
        assert!(theme.matched.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn test_multiple_color_components() {
        // Test multiple color components separated by comma
        let theme = ColorTheme::from_options("matched:108,prompt:25");
        assert_eq!(theme.matched.fg, Some(Color::Indexed(108)));
        assert_eq!(theme.prompt.fg, Some(Color::Indexed(25)));

        let theme = ColorTheme::from_options("matched:#ff0000:bold,prompt:#00ff00:underlined");
        assert_eq!(theme.matched.fg, Some(Color::Rgb(255, 0, 0)));
        assert!(theme.matched.add_modifier.contains(Modifier::BOLD));
        assert_eq!(theme.prompt.fg, Some(Color::Rgb(0, 255, 0)));
        assert!(theme.prompt.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn test_component_name_aliases() {
        // Test that aliases work correctly
        let theme = ColorTheme::from_options("hl:108");
        assert_eq!(theme.matched.fg, Some(Color::Indexed(108)));

        let theme = ColorTheme::from_options("fg+:254");
        assert_eq!(theme.current.fg, Some(Color::Indexed(254)));

        let theme = ColorTheme::from_options("bg+:236");
        assert_eq!(theme.current.bg, Some(Color::Indexed(236)));

        let theme = ColorTheme::from_options("hl+:151");
        // hl+ is an alias for current_match
        assert_eq!(theme.current_match.fg, Some(Color::Indexed(151)));

        let theme = ColorTheme::from_options("pointer:161");
        assert_eq!(theme.cursor.fg, Some(Color::Indexed(161)));

        let theme = ColorTheme::from_options("marker:168");
        assert_eq!(theme.selected.fg, Some(Color::Indexed(168)));
    }

    #[test]
    fn test_background_color() {
        // Test setting background color explicitly
        let theme = ColorTheme::from_options("matched_bg:0");
        assert_eq!(theme.matched.bg, Some(Color::Indexed(0)));

        let theme = ColorTheme::from_options("matched-bg:236");
        assert_eq!(theme.matched.bg, Some(Color::Indexed(236)));
    }

    #[test]
    fn test_base_theme_with_overrides() {
        // Test that base theme can be overridden
        let theme = ColorTheme::from_options("dark,matched:200");
        assert_eq!(theme.matched.fg, Some(Color::Indexed(200)));
        // Other colors should still be from dark theme
        assert!(theme.prompt.fg.is_some());
    }

    #[test]
    fn test_all_component_names() {
        // Test all valid component names with their specific colors
        let theme = ColorTheme::from_options("normal:108");
        assert_eq!(theme.normal.fg, Some(Color::Indexed(108)));

        let theme = ColorTheme::from_options("matched:109");
        assert_eq!(theme.matched.fg, Some(Color::Indexed(109)));

        let theme = ColorTheme::from_options("current:110");
        assert_eq!(theme.current.fg, Some(Color::Indexed(110)));

        let theme = ColorTheme::from_options("current_match:111");
        // current_match should now correctly set current_match.fg
        assert_eq!(theme.current_match.fg, Some(Color::Indexed(111)));

        let theme = ColorTheme::from_options("query:112");
        assert_eq!(theme.query.fg, Some(Color::Indexed(112)));

        let theme = ColorTheme::from_options("spinner:113");
        assert_eq!(theme.spinner.fg, Some(Color::Indexed(113)));

        let theme = ColorTheme::from_options("info:114");
        assert_eq!(theme.info.fg, Some(Color::Indexed(114)));

        let theme = ColorTheme::from_options("prompt:115");
        assert_eq!(theme.prompt.fg, Some(Color::Indexed(115)));

        let theme = ColorTheme::from_options("cursor:116");
        assert_eq!(theme.cursor.fg, Some(Color::Indexed(116)));

        let theme = ColorTheme::from_options("selected:117");
        assert_eq!(theme.selected.fg, Some(Color::Indexed(117)));

        let theme = ColorTheme::from_options("header:118");
        assert_eq!(theme.header.fg, Some(Color::Indexed(118)));

        let theme = ColorTheme::from_options("border:119");
        assert_eq!(theme.border.fg, Some(Color::Indexed(119)));
    }

    #[test]
    fn test_invalid_color_graceful_handling() {
        // Test that invalid color values don't crash
        // When color is invalid (not a number), it returns None and the color isn't set
        // So the theme starts with dark256() and the invalid color spec doesn't change it
        let theme = ColorTheme::from_options("matched:invalid");
        // Should remain the dark256 default since invalid color is ignored
        assert_eq!(theme.matched.fg, Some(Color::Indexed(108)));
        assert_eq!(theme.matched.bg, Some(Color::Indexed(0)));

        // Invalid hex digits in #rrggbb format will use unwrap_or(255) fallback
        // So "#gggggg" becomes Rgb(255, 255, 255) since 'gg' is invalid hex
        let theme = ColorTheme::from_options("matched:#gggggg");
        assert_eq!(theme.matched.fg, Some(Color::Rgb(255, 255, 255)));
        // But the background remains from dark256 theme since we only set fg
        assert_eq!(theme.matched.bg, Some(Color::Indexed(0)));
    }

    #[test]
    fn test_init_from_options() {
        // Test initialization from SkimOptions
        let mut opts = crate::options::SkimOptionsBuilder::default().build().unwrap();
        opts.color = Some("matched:108".to_string());
        let theme = ColorTheme::init_from_options(&opts);
        assert_eq!(theme.matched.fg, Some(Color::Indexed(108)));
    }

    #[test]
    fn test_complex_color_spec() {
        // Test a complex real-world color specification
        let theme =
            ColorTheme::from_options("dark,matched:#00ff00:bold,prompt:#0000ff:underlined,current:#ffff00:italic");
        assert_eq!(theme.matched.fg, Some(Color::Rgb(0, 255, 0)));
        assert!(theme.matched.add_modifier.contains(Modifier::BOLD));
        assert_eq!(theme.prompt.fg, Some(Color::Rgb(0, 0, 255)));
        assert!(theme.prompt.add_modifier.contains(Modifier::UNDERLINED));
        assert_eq!(theme.current.fg, Some(Color::Rgb(255, 255, 0)));
        assert!(theme.current.add_modifier.contains(Modifier::ITALIC));
    }
}
