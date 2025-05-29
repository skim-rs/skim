//! Ratatui-compatible theme system
//!
//! This module provides theme conversion between tuikit and ratatui style systems.
//! It maintains compatibility with skim's existing color schemes while providing
//! modern ratatui styling capabilities.

use ratatui::style::{Color, Modifier, Style};
use std::collections::HashMap;

/// Ratatui-compatible color theme for skim
#[derive(Debug, Clone)]
pub struct RatatuiTheme {
    /// Base foreground color
    pub fg: Color,
    /// Base background color
    pub bg: Color,
    
    // Text matching colors
    /// Color for matched text in normal items
    pub matched: Color,
    /// Background for matched text
    pub matched_bg: Color,
    /// Style modifiers for matched text
    pub matched_style: Modifier,
    
    // Current item colors
    /// Color for current (highlighted) item
    pub current: Color,
    /// Background for current item
    pub current_bg: Color,
    /// Style modifiers for current item
    pub current_style: Modifier,
    
    // Current matched text colors
    /// Color for matched text in current item
    pub current_match: Color,
    /// Background for matched text in current item
    pub current_match_bg: Color,
    /// Style modifiers for matched text in current item
    pub current_match_style: Modifier,
    
    // Query input colors
    /// Query input text color
    pub query_fg: Color,
    /// Query input background color
    pub query_bg: Color,
    /// Query input style modifiers
    pub query_style: Modifier,
    
    // UI element colors
    /// Spinner/loading indicator color
    pub spinner: Color,
    /// Info/status text color
    pub info: Color,
    /// Prompt text color
    pub prompt: Color,
    /// Cursor color
    pub cursor: Color,
    /// Selected item indicator color
    pub selected: Color,
    /// Header text color
    pub header: Color,
    /// Border color
    pub border: Color,
}

impl Default for RatatuiTheme {
    fn default() -> Self {
        Self::dark256()
    }
}

impl RatatuiTheme {
    /// Create a new empty theme with default colors
    pub fn empty() -> Self {
        Self {
            fg: Color::Reset,
            bg: Color::Reset,
            matched: Color::Reset,
            matched_bg: Color::Reset,
            matched_style: Modifier::empty(),
            current: Color::Reset,
            current_bg: Color::Reset,
            current_style: Modifier::empty(),
            current_match: Color::Reset,
            current_match_bg: Color::Reset,
            current_match_style: Modifier::empty(),
            query_fg: Color::Reset,
            query_bg: Color::Reset,
            query_style: Modifier::empty(),
            spinner: Color::Reset,
            info: Color::Reset,
            prompt: Color::Reset,
            cursor: Color::Reset,
            selected: Color::Reset,
            header: Color::Reset,
            border: Color::Reset,
        }
    }
    
    /// Black and white theme
    pub fn bw() -> Self {
        Self {
            matched_style: Modifier::UNDERLINED,
            current_style: Modifier::REVERSED,
            current_match_style: Modifier::UNDERLINED | Modifier::REVERSED,
            ..Self::empty()
        }
    }
    
    /// 16-color theme
    pub fn default16() -> Self {
        Self {
            matched: Color::Green,
            matched_bg: Color::Black,
            current: Color::Yellow,
            current_bg: Color::Black,
            current_match: Color::Green,
            current_match_bg: Color::Black,
            spinner: Color::Green,
            info: Color::White,
            prompt: Color::Blue,
            cursor: Color::Red,
            selected: Color::Magenta,
            header: Color::Cyan,
            border: Color::DarkGray,
            ..Self::empty()
        }
    }
    
    /// 256-color dark theme (default)
    pub fn dark256() -> Self {
        Self {
            matched: Color::Indexed(108),
            matched_bg: Color::Indexed(0),
            current: Color::Indexed(254),
            current_bg: Color::Indexed(236),
            current_match: Color::Indexed(151),
            current_match_bg: Color::Indexed(236),
            spinner: Color::Indexed(148),
            info: Color::Indexed(144),
            prompt: Color::Indexed(110),
            cursor: Color::Indexed(161),
            selected: Color::Indexed(168),
            header: Color::Indexed(109),
            border: Color::Indexed(59),
            ..Self::empty()
        }
    }
    
    /// Molokai 256-color theme
    pub fn molokai256() -> Self {
        Self {
            matched: Color::Indexed(234),
            matched_bg: Color::Indexed(186),
            current: Color::Indexed(254),
            current_bg: Color::Indexed(236),
            current_match: Color::Indexed(234),
            current_match_bg: Color::Indexed(186),
            spinner: Color::Indexed(148),
            info: Color::Indexed(144),
            prompt: Color::Indexed(110),
            cursor: Color::Indexed(161),
            selected: Color::Indexed(168),
            header: Color::Indexed(109),
            border: Color::Indexed(59),
            ..Self::empty()
        }
    }
    
    /// Get style for normal text
    pub fn normal_style(&self) -> Style {
        Style::default()
            .fg(self.fg)
            .bg(self.bg)
    }
    
    /// Get style for matched text
    pub fn matched_style(&self) -> Style {
        Style::default()
            .fg(self.matched)
            .bg(self.matched_bg)
            .add_modifier(self.matched_style)
    }
    
    /// Get style for current item
    pub fn current_style(&self) -> Style {
        Style::default()
            .fg(self.current)
            .bg(self.current_bg)
            .add_modifier(self.current_style)
    }
    
    /// Get style for current matched text
    pub fn current_match_style(&self) -> Style {
        Style::default()
            .fg(self.current_match)
            .bg(self.current_match_bg)
            .add_modifier(self.current_match_style)
    }
    
    /// Get style for query input
    pub fn query_style(&self) -> Style {
        Style::default()
            .fg(self.query_fg)
            .bg(self.query_bg)
            .add_modifier(self.query_style)
    }
    
    /// Get style for spinner
    pub fn spinner_style(&self) -> Style {
        Style::default().fg(self.spinner)
    }
    
    /// Get style for info text
    pub fn info_style(&self) -> Style {
        Style::default().fg(self.info)
    }
    
    /// Get style for prompt text
    pub fn prompt_style(&self) -> Style {
        Style::default().fg(self.prompt)
    }
    
    /// Get style for cursor
    pub fn cursor_style(&self) -> Style {
        Style::default().fg(self.cursor)
    }
    
    /// Get style for selected indicator
    pub fn selected_style(&self) -> Style {
        Style::default().fg(self.selected)
    }
    
    /// Get style for header text
    pub fn header_style(&self) -> Style {
        Style::default().fg(self.header)
    }
    
    /// Get style for borders
    pub fn border_style(&self) -> Style {
        Style::default().fg(self.border)
    }
    
    /// Create theme from color option string (compatible with skim's --color option)
    pub fn from_options(color_str: &str) -> Self {
        let mut theme = Self::dark256(); // Start with default
        
        if color_str == "bw" {
            return Self::bw();
        } else if color_str == "16" {
            return Self::default16();
        } else if color_str == "dark256" {
            return Self::dark256();
        } else if color_str == "molokai256" {
            return Self::molokai256();
        }
        
        // Parse custom color specifications
        // Format: "fg:color,bg:color,matched:color,current:color,..."
        let color_map = parse_color_string(color_str);
        
        for (key, color) in color_map {
            match key.as_str() {
                "fg" => theme.fg = color,
                "bg" => theme.bg = color,
                "matched" => theme.matched = color,
                "matched_bg" => theme.matched_bg = color,
                "current" => theme.current = color,
                "current_bg" => theme.current_bg = color,
                "current_match" => theme.current_match = color,
                "current_match_bg" => theme.current_match_bg = color,
                "query" => theme.query_fg = color,
                "query_bg" => theme.query_bg = color,
                "spinner" => theme.spinner = color,
                "info" => theme.info = color,
                "prompt" => theme.prompt = color,
                "cursor" => theme.cursor = color,
                "selected" => theme.selected = color,
                "header" => theme.header = color,
                "border" => theme.border = color,
                _ => {} // Unknown color key, ignore
            }
        }
        
        theme
    }
}

/// Parse a color string into a map of color assignments
fn parse_color_string(color_str: &str) -> HashMap<String, Color> {
    let mut color_map = HashMap::new();
    
    for assignment in color_str.split(',') {
        let parts: Vec<&str> = assignment.split(':').collect();
        if parts.len() == 2 {
            let key = parts[0].trim().to_lowercase();
            if let Some(color) = parse_color(parts[1].trim()) {
                color_map.insert(key, color);
            }
        }
    }
    
    color_map
}

/// Parse a single color specification
fn parse_color(color_str: &str) -> Option<Color> {
    match color_str.to_lowercase().as_str() {
        "default" | "reset" => Some(Color::Reset),
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "white" => Some(Color::White),
        "gray" | "grey" => Some(Color::DarkGray),
        "dark_gray" | "dark_grey" => Some(Color::DarkGray),
        "light_red" => Some(Color::LightRed),
        "light_green" => Some(Color::LightGreen),
        "light_yellow" => Some(Color::LightYellow),
        "light_blue" => Some(Color::LightBlue),
        "light_magenta" => Some(Color::LightMagenta),
        "light_cyan" => Some(Color::LightCyan),
        
        // RGB colors (format: #rrggbb or rgb(r,g,b))
        color if color.starts_with('#') && color.len() == 7 => {
            parse_hex_color(&color[1..])
        }
        color if color.starts_with("rgb(") && color.ends_with(')') => {
            parse_rgb_color(&color[4..color.len()-1])
        }
        
        // Indexed colors (format: number 0-255)
        color => {
            if let Ok(index) = color.parse::<u8>() {
                Some(Color::Indexed(index))
            } else {
                None
            }
        }
    }
}

/// Parse hex color (#rrggbb)
fn parse_hex_color(hex: &str) -> Option<Color> {
    if hex.len() != 6 {
        return None;
    }
    
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    
    Some(Color::Rgb(r, g, b))
}

/// Parse RGB color (r,g,b)
fn parse_rgb_color(rgb: &str) -> Option<Color> {
    let parts: Vec<&str> = rgb.split(',').collect();
    if parts.len() != 3 {
        return None;
    }
    
    let r = parts[0].trim().parse::<u8>().ok()?;
    let g = parts[1].trim().parse::<u8>().ok()?;
    let b = parts[2].trim().parse::<u8>().ok()?;
    
    Some(Color::Rgb(r, g, b))
}

/// Convert tuikit Color to ratatui Color (for legacy compatibility)
pub fn convert_tuikit_color(tuikit_color: crate::theme::ColorTheme) -> RatatuiTheme {
    // This is a compatibility bridge - for now we'll map to similar colors
    // In a complete migration, this would need the actual tuikit Color enum
    RatatuiTheme::dark256() // Simplified for compilation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_creation() {
        let theme = RatatuiTheme::dark256();
        assert_eq!(theme.matched, Color::Indexed(108));
        assert_eq!(theme.current, Color::Indexed(254));
        
        let bw_theme = RatatuiTheme::bw();
        assert_eq!(bw_theme.matched_style, Modifier::UNDERLINED);
    }
    
    #[test]
    fn test_style_generation() {
        let theme = RatatuiTheme::dark256();
        
        let matched_style = theme.matched_style();
        assert_eq!(matched_style.fg, Some(Color::Indexed(108)));
        assert_eq!(matched_style.bg, Some(Color::Indexed(0)));
        
        let current_style = theme.current_style();
        assert_eq!(current_style.fg, Some(Color::Indexed(254)));
        assert_eq!(current_style.bg, Some(Color::Indexed(236)));
    }
    
    #[test]
    fn test_color_parsing() {
        assert_eq!(parse_color("red"), Some(Color::Red));
        assert_eq!(parse_color("255"), Some(Color::Indexed(255)));
        assert_eq!(parse_color("#ff0000"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(parse_color("rgb(255,0,0)"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(parse_color("invalid"), None);
    }
    
    #[test]
    fn test_hex_color_parsing() {
        assert_eq!(parse_hex_color("ff0000"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(parse_hex_color("00ff00"), Some(Color::Rgb(0, 255, 0)));
        assert_eq!(parse_hex_color("0000ff"), Some(Color::Rgb(0, 0, 255)));
        assert_eq!(parse_hex_color("invalid"), None);
        assert_eq!(parse_hex_color("ff00"), None); // Wrong length
    }
    
    #[test]
    fn test_rgb_color_parsing() {
        assert_eq!(parse_rgb_color("255,0,0"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(parse_rgb_color("0, 255, 0"), Some(Color::Rgb(0, 255, 0)));
        assert_eq!(parse_rgb_color("0,0,255"), Some(Color::Rgb(0, 0, 255)));
        assert_eq!(parse_rgb_color("invalid"), None);
        assert_eq!(parse_rgb_color("255,0"), None); // Missing component
    }
    
    #[test]
    fn test_color_string_parsing() {
        let color_map = parse_color_string("fg:red,bg:blue,matched:green");
        assert_eq!(color_map.get("fg"), Some(&Color::Red));
        assert_eq!(color_map.get("bg"), Some(&Color::Blue));
        assert_eq!(color_map.get("matched"), Some(&Color::Green));
        
        let indexed_map = parse_color_string("spinner:148,info:144");
        assert_eq!(indexed_map.get("spinner"), Some(&Color::Indexed(148)));
        assert_eq!(indexed_map.get("info"), Some(&Color::Indexed(144)));
    }
    
    #[test]
    fn test_theme_from_options() {
        let bw_theme = RatatuiTheme::from_options("bw");
        assert_eq!(bw_theme.matched_style, Modifier::UNDERLINED);
        
        let custom_theme = RatatuiTheme::from_options("fg:red,bg:blue");
        assert_eq!(custom_theme.fg, Color::Red);
        assert_eq!(custom_theme.bg, Color::Blue);
    }
}