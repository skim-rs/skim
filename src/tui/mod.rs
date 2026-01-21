//! Terminal UI components and rendering.
//!
//! This module provides the terminal user interface components for skim,
//! including the application state, event handling, rendering widgets,
//! and layout management.

use std::num::ParseIntError;

pub use app::App;
pub use event::Event;
pub use preview::PreviewCallback;
use thiserror::Error;
pub use widget::{SkimRender, SkimWidget};
mod app;
mod backend;
mod util;
pub use backend::Tui;
/// Event handling and action definitions
pub mod event;
/// Header display components
pub mod header;
mod input;
/// Item list display and management
pub mod item_list;
/// TUI-specific options and configuration
pub mod options;
mod preview;
/// Status line display
pub mod statusline;
/// Widget rendering utilities
pub mod widget;

/// Represents a size value, either as a percentage or fixed value
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    /// Size as a percentage (0-100)
    Percent(u16),
    /// Fixed size in terminal cells
    Fixed(u16),
}

/// Direction for movement or layout
#[derive(PartialEq, Eq, Clone, Debug, Copy)]
pub enum Direction {
    /// Upward direction
    Up,
    /// Downward direction
    Down,
    /// Left direction
    Left,
    /// Right direction
    Right,
}

impl TryFrom<&str> for Direction {
    type Error = &'static str;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "up" => Ok(Self::Up),
            "down" => Ok(Self::Down),
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            _ => Err("Unknown direction {value}"),
        }
    }
}

/// Error type for parsing size values
#[derive(Error, Debug, PartialEq, Eq)]
pub enum SizeParseError {
    /// Error parsing the size string
    #[error("Error parsing {0}: {1:?}")]
    ParseError(String, ParseIntError),
    /// Percentage value exceeds 100
    #[error("Invalid percentage {0}")]
    InvalidPercent(u16),
}

impl TryFrom<&str> for Size {
    type Error = SizeParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.ends_with("%") {
            let percent = value
                .strip_suffix("%")
                .unwrap_or_default()
                .parse::<u16>()
                .map_err(|e| SizeParseError::ParseError(value.to_string(), e))?;
            if percent > 100 {
                return Err(SizeParseError::InvalidPercent(percent));
            }
            Ok(Self::Percent(percent))
        } else {
            Ok(Self::Fixed(
                value
                    .parse::<u16>()
                    .map_err(|e| SizeParseError::ParseError(value.to_string(), e))?,
            ))
        }
    }
}

impl Default for Size {
    fn default() -> Self {
        Self::Percent(100)
    }
}

#[cfg(test)]
mod size_test {
    use super::*;
    use std::num::IntErrorKind;
    #[test]
    fn fixed_success() {
        assert_eq!(Size::try_from("10"), Ok(Size::Fixed(10u16)));
    }
    #[test]
    fn percent_success() {
        assert_eq!(Size::try_from("10%"), Ok(Size::Percent(10u16)));
    }
    #[test]
    fn fixed_neg() {
        let SizeParseError::ParseError(err_value, internal_error) = Size::try_from("-10").unwrap_err() else {
            assert!(false);
            return;
        };
        assert_eq!(internal_error.kind(), &IntErrorKind::InvalidDigit);
        assert_eq!(err_value, String::from("-10"));
    }
    #[test]
    fn percent_neg() {
        let SizeParseError::ParseError(err_value, internal_error) = Size::try_from("-10%").unwrap_err() else {
            assert!(false);
            return;
        };
        assert_eq!(internal_error.kind(), &IntErrorKind::InvalidDigit);
        assert_eq!(err_value, String::from("-10%"));
    }
    #[test]
    fn percent_over_100() {
        let SizeParseError::InvalidPercent(internal_error) = Size::try_from("110%").unwrap_err() else {
            assert!(false);
            return;
        };
        assert_eq!(internal_error, 110u16);
    }
    #[test]
    fn fixed_invalid_char() {
        let SizeParseError::ParseError(value, internal_error) = Size::try_from("1-0").unwrap_err() else {
            assert!(false);
            return;
        };
        assert_eq!(internal_error.kind(), &IntErrorKind::InvalidDigit);
        assert_eq!(value, String::from("1-0"));
    }
    #[test]
    fn percent_invalid_char() {
        let SizeParseError::ParseError(value, internal_error) = Size::try_from("1-0%").unwrap_err() else {
            assert!(false);
            return;
        };
        assert_eq!(internal_error.kind(), &IntErrorKind::InvalidDigit);
        assert_eq!(value, String::from("1-0%"));
    }
    #[test]
    fn percent_empty() {
        let SizeParseError::ParseError(value, internal_error) = Size::try_from("%").unwrap_err() else {
            assert!(false);
            return;
        };
        assert_eq!(internal_error.kind(), &IntErrorKind::Empty);
        assert_eq!(value, String::from("%"));
    }
}
