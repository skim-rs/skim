use std::num::ParseIntError;

pub use app::App;
pub use event::Event;
use thiserror::Error;
pub use tui::Tui;
mod app;
pub mod event;
pub mod header;
mod input;
pub mod item_list;
pub mod options;
mod preview;
mod statusline;
mod tui;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    Percent(u16),
    Fixed(u16),
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum SizeParseError {
    #[error("Error parsing {0}: {1:?}")]
    ParseError(String, ParseIntError),
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
        assert_eq!(internal_error.kind(), &IntErrorKind::NegOverflow);
        assert_eq!(err_value, String::from("-10"));
    }
    #[test]
    fn percent_neg() {
        let SizeParseError::ParseError(err_value, internal_error) = Size::try_from("-10%").unwrap_err() else {
            assert!(false);
            return;
        };
        assert_eq!(internal_error.kind(), &IntErrorKind::NegOverflow);
        assert_eq!(err_value, String::from("-10"));
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
