//! Handle the color theme
use ratatui::style::{Color, Modifier, Style};

use crate::options::SkimOptions;

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
#[derive(Copy, Clone, Debug)]
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
    /// Scrollbar thumb on the item list
    pub scrollbar: Style,
}

impl Default for ColorTheme {
    /// Theme defaults to Dark256
    fn default() -> Self {
        ColorTheme::dark256()
    }
}

#[allow(dead_code)]
impl ColorTheme {
    /// Setup the theme from the skim options
    #[must_use]
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
        let def = Style::default();
        Self {
            spinner: Style::default().bold(),
            normal: def,
            matched: def,
            current: def,
            current_match: def,
            query: def,
            info: def,
            prompt: def,
            cursor: def,
            selected: def,
            header: def,
            border: def,
            scrollbar: def,
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
            scrollbar: base.scrollbar.fg(Color::Black),
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
            scrollbar: base.scrollbar.fg(Color::Indexed(59)),
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
            scrollbar: base.scrollbar.fg(Color::Indexed(59)),
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
            scrollbar: base.scrollbar.fg(Color::Indexed(145)),
            ..base
        }
    }

    #[allow(unused_variables)]
    fn catppuccin_mocha() -> Self {
        let base = ColorTheme::none();
        let text = Color::Rgb(205, 214, 244);
        let subtext0 = Color::Rgb(166, 173, 200);
        let subtext1 = Color::Rgb(186, 194, 222);
        let overlay0 = Color::Rgb(108, 112, 134);
        let surface0 = Color::Rgb(49, 50, 68);
        let blue = Color::Rgb(137, 180, 250);
        let red = Color::Rgb(243, 139, 168);
        let lavender = Color::Rgb(180, 190, 254);
        let sapphire = Color::Rgb(116, 199, 236);
        Self {
            normal: base.normal.fg(text),
            matched: base.matched.fg(blue).underlined(),
            current: base.current.bg(surface0),
            current_match: base.current_match.fg(red).underlined(),
            query: base.query.fg(text),
            spinner: base.spinner.fg(subtext1).bold(),
            info: base.info.fg(subtext1),
            prompt: base.prompt.fg(lavender),
            cursor: base.cursor.fg(red),
            selected: base.selected.fg(red),
            header: base.header.fg(subtext1),
            border: base.header.fg(lavender),
            scrollbar: base.scrollbar.fg(overlay0),
        }
    }
    #[allow(unused_variables)]
    fn catppuccin_macchiato() -> Self {
        let base = ColorTheme::none();
        let text = Color::Rgb(202, 211, 245);
        let subtext0 = Color::Rgb(165, 173, 203);
        let subtext1 = Color::Rgb(184, 192, 224);
        let overlay0 = Color::Rgb(110, 115, 141);
        let surface0 = Color::Rgb(54, 58, 79);
        let blue = Color::Rgb(138, 173, 244);
        let red = Color::Rgb(237, 135, 150);
        let lavender = Color::Rgb(183, 189, 248);
        let sapphire = Color::Rgb(125, 196, 228);
        Self {
            normal: base.normal.fg(text),
            matched: base.matched.fg(blue).underlined(),
            current: base.current.bg(surface0),
            current_match: base.current_match.fg(red).underlined(),
            query: base.query.fg(text),
            spinner: base.spinner.fg(subtext1).bold(),
            info: base.info.fg(subtext1),
            prompt: base.prompt.fg(lavender),
            cursor: base.cursor.fg(red),
            selected: base.selected.fg(red),
            header: base.header.fg(subtext1),
            border: base.header.fg(lavender),
            scrollbar: base.scrollbar.fg(overlay0),
        }
    }
    #[allow(unused_variables)]
    fn catppuccin_latte() -> Self {
        let base = ColorTheme::none();
        let text = Color::Rgb(76, 79, 105);
        let subtext0 = Color::Rgb(108, 111, 133);
        let subtext1 = Color::Rgb(92, 95, 119);
        let overlay0 = Color::Rgb(156, 160, 176);
        let surface0 = Color::Rgb(204, 208, 218);
        let blue = Color::Rgb(30, 102, 245);
        let red = Color::Rgb(210, 15, 57);
        let lavender = Color::Rgb(114, 135, 253);
        let sapphire = Color::Rgb(32, 159, 181);
        Self {
            normal: base.normal.fg(text),
            matched: base.matched.fg(blue).underlined(),
            current: base.current.bg(surface0),
            current_match: base.current_match.fg(red).underlined(),
            query: base.query.fg(text),
            spinner: base.spinner.fg(subtext1).bold(),
            info: base.info.fg(subtext1),
            prompt: base.prompt.fg(lavender),
            cursor: base.cursor.fg(red),
            selected: base.selected.fg(red),
            header: base.header.fg(subtext1),
            border: base.header.fg(lavender),
            scrollbar: base.scrollbar.fg(overlay0),
        }
    }
    #[allow(unused_variables)]
    fn catppuccin_frappe() -> Self {
        let base = ColorTheme::none();
        let text = Color::Rgb(198, 208, 245);
        let subtext0 = Color::Rgb(165, 173, 206);
        let subtext1 = Color::Rgb(181, 191, 226);
        let overlay0 = Color::Rgb(115, 121, 148);
        let surface0 = Color::Rgb(65, 69, 89);
        let blue = Color::Rgb(140, 170, 238);
        let red = Color::Rgb(231, 130, 132);
        let lavender = Color::Rgb(186, 187, 241);
        let sapphire = Color::Rgb(133, 193, 220);
        Self {
            normal: base.normal.fg(text),
            matched: base.matched.fg(blue).underlined(),
            current: base.current.bg(surface0),
            current_match: base.current_match.fg(red).underlined(),
            query: base.query.fg(text),
            spinner: base.spinner.fg(subtext1).bold(),
            info: base.info.fg(subtext1),
            prompt: base.prompt.fg(lavender),
            cursor: base.cursor.fg(red),
            selected: base.selected.fg(red),
            header: base.header.fg(subtext1),
            border: base.header.fg(lavender),
            scrollbar: base.scrollbar.fg(overlay0),
        }
    }

    fn set_color(&mut self, name: &str, spec: &str) {
        let spec_parts: Vec<_> = spec.split(&['+', ':']).collect();

        // Compute modifiers
        let mut modifier = Modifier::empty();
        for part in spec_parts.iter().skip(1) {
            if matches!(*part, "x" | "regular") {
                modifier = Modifier::empty();
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

        let target_style = match component_name {
            "" | "normal" => &mut self.normal,
            "matched" | "hl" => &mut self.matched,
            "current" | "fg+" | "bg+" => &mut self.current,
            "current_match" | "hl+" => &mut self.current_match,
            "query" => &mut self.query,
            "spinner" => &mut self.spinner,
            "info" => &mut self.info,
            "prompt" => &mut self.prompt,
            "cursor" | "pointer" => &mut self.cursor,
            "selected" | "marker" => &mut self.selected,
            "header" => &mut self.header,
            "border" => &mut self.border,
            "scrollbar" => &mut self.scrollbar,
            _ => return,
        };

        // Handle color reset with `-1`
        let raw_color = spec_parts[0];
        // Compute color
        let new_color = if raw_color.len() == 7 && raw_color.starts_with('#') {
            // RGB Hex color
            let r = u8::from_str_radix(&raw_color[1..3], 16).unwrap_or(255);
            let g = u8::from_str_radix(&raw_color[3..5], 16).unwrap_or(255);
            let b = u8::from_str_radix(&raw_color[5..7], 16).unwrap_or(255);
            Some(Color::Rgb(r, g, b))
        } else if raw_color == "-1" {
            Some(Color::Reset)
        } else {
            raw_color.parse::<u8>().ok().map(Color::Indexed).or_else(|| {
                if !raw_color.is_empty() {
                    debug!("Unknown color '{}'", spec_parts[0]);
                }
                None
            })
        };

        let layer_override = if component_name == "bg+" { "bg" } else { layer };
        set_style(target_style, layer_override, new_color, modifier);
    }

    fn from_options(color: &str) -> Self {
        let mut theme = ColorTheme::dark256();
        for pair in color.split(',') {
            if let Some((name, spec)) = pair.split_once(':') {
                theme.set_color(name, spec);
            } else {
                theme = match pair {
                    "molokai" => ColorTheme::molokai256(),
                    "light" => ColorTheme::light256(),
                    "16" => ColorTheme::default16(),
                    "bw" => ColorTheme::bw(),
                    "none" | "empty" => ColorTheme::none(),
                    "dark" | "default" => ColorTheme::dark256(),
                    "catppuccin_mocha" | "catppuccin-mocha" => ColorTheme::catppuccin_mocha(),
                    "catppuccin_macchiato" | "catppuccin-macchiato" => ColorTheme::catppuccin_macchiato(),
                    "catppuccin_latte" | "catppuccin-latte" => ColorTheme::catppuccin_latte(),
                    "catppuccin_frappe" | "catppuccin-frappe" => ColorTheme::catppuccin_frappe(),
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
#[path = "theme_tests.rs"]
mod tests;
