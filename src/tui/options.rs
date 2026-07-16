use crate::tui::{Direction, Size};

/// Layout configuration for the TUI
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum TuiLayout {
    /// Display from the bottom of the screen
    #[default]
    Default,
    /// Display from the top of the screen
    Reverse,
    /// Display from the top of the screen, prompt at the bottom
    ReverseList,
}

/// Configuration for the preview pane layout
#[derive(Debug, Clone)]
pub struct PreviewLayout {
    /// Direction where preview pane is positioned
    pub direction: Direction,
    /// Size of the preview pane
    pub size: Size,
    /// Whether the preview pane is hidden
    pub hidden: bool,
    /// Optional offset for preview position
    pub offset: Option<String>,
    /// Whether or not to wrap the preview contents
    pub wrap: bool,
    /// Whether or not to run the preview in a PTY
    pub pty: bool,
}

impl Default for PreviewLayout {
    fn default() -> Self {
        Self {
            direction: Direction::Right,
            size: Size::Percent(50),
            hidden: false,
            offset: None,
            wrap: false,
            pty: false,
        }
    }
}

impl From<&str> for PreviewLayout {
    fn from(value: &str) -> Self {
        let mut res: Self = PreviewLayout::default();
        // Parse the remainder which can be: size:offset:hidden, offset:hidden, size:hidden, etc.
        let parts: Vec<&str> = value.split(':').collect();

        for part in parts {
            if part.is_empty() {
                continue;
            }

            if part.starts_with('+') {
                // This is an offset expression
                res.offset = Some(part.to_string());
            } else if part == "hidden" {
                res.hidden = true;
            } else if part == "nohidden" {
                res.hidden = false;
            } else if part == "wrap" {
                res.wrap = true;
            } else if part == "nowrap" {
                res.wrap = false;
            } else if part == "pty" {
                res.pty = true;
            } else if part == "nopty" {
                res.pty = false;
            } else {
                // Try to parse as size
                if let Ok(size) = part.try_into() {
                    res.size = size;
                }
                if let Ok(dir) = part.try_into() {
                    res.direction = dir;
                }
            }
        }
        res
    }
}

// impl PreviewLayout {

// }

#[cfg(test)]
#[path = "options_tests.rs"]
mod tests;
