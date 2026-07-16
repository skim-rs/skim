use std::time::Instant;

/// Default inline info separator
pub const DEFAULT_SEPARATOR: &str = "  < ";
pub(crate) const SPINNER_DURATION: u32 = 200;
pub(crate) const SPINNERS_UNICODE: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub(crate) fn spinner_char(start: Instant) -> char {
    let spinner_elapsed_ms = start.elapsed().as_millis();
    let index = ((spinner_elapsed_ms / u128::from(SPINNER_DURATION)) % (SPINNERS_UNICODE.len() as u128)) as usize;
    SPINNERS_UNICODE[index]
}

/// Simplified display mode for the info/status line
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub enum InfoDisplay {
    /// Display info in a separate line (default)
    #[default]
    Default,
    /// Display info inline with the input
    Inline,
    /// Hide the info display
    Hidden,
    /// Inline and right-aligned
    InlineRight,
}
impl InfoDisplay {
    pub(crate) fn is_inline(&self) -> bool {
        matches!(self, InfoDisplay::Inline | InfoDisplay::InlineRight)
    }
}

/// Full display mode for the info/status line
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct Info {
    /// The `InfoDisplay`
    pub display: InfoDisplay,
    /// The separator, specified if the display is inline
    pub separator: Option<String>,
}

impl Info {
    pub(crate) fn separator(&self) -> Option<&str> {
        self.separator.as_deref()
    }
}

impl From<InfoDisplay> for Info {
    fn from(value: InfoDisplay) -> Self {
        let is_inline = value.is_inline();
        Self {
            display: value,
            separator: if is_inline {
                Some(String::from(DEFAULT_SEPARATOR))
            } else {
                None
            },
        }
    }
}

impl From<&str> for Info {
    fn from(s: &str) -> Self {
        use InfoDisplay::{Default, Hidden, Inline, InlineRight};
        let mut parts = s.split(':');

        let display = match parts.next() {
            None | Some("default") => Default,
            Some("inline") => Inline,
            Some("inline-right") => InlineRight,
            Some("hidden") => Hidden,
            Some(x) => panic!(
                "Failed to parse {x} as an InfoDisplay. Possible options are `default`, `inline`, `inline-right` or `hidden`"
            ),
        };
        let separator = if display.is_inline() {
            parts.next().or(Some(DEFAULT_SEPARATOR)).map(String::from)
        } else {
            None
        };
        Self { display, separator }
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn spinner_char_returns_first_frame_immediately() {
        // No elapsed time → index 0.
        assert_eq!(spinner_char(Instant::now()), SPINNERS_UNICODE[0]);
    }

    #[test]
    fn info_display_is_inline() {
        assert!(InfoDisplay::Inline.is_inline());
        assert!(InfoDisplay::InlineRight.is_inline());
        assert!(!InfoDisplay::Default.is_inline());
        assert!(!InfoDisplay::Hidden.is_inline());
    }

    #[test]
    fn info_from_display_sets_separator_only_when_inline() {
        let inline = Info::from(InfoDisplay::Inline);
        assert_eq!(inline.separator(), Some(DEFAULT_SEPARATOR));

        let right = Info::from(InfoDisplay::InlineRight);
        assert_eq!(right.separator(), Some(DEFAULT_SEPARATOR));

        let default = Info::from(InfoDisplay::Default);
        assert_eq!(default.separator(), None);

        let hidden = Info::from(InfoDisplay::Hidden);
        assert_eq!(hidden.separator(), None);
    }

    #[test]
    fn info_from_str_parses_each_mode() {
        assert_eq!(Info::from("default").display, InfoDisplay::Default);
        assert_eq!(Info::from("inline").display, InfoDisplay::Inline);
        assert_eq!(Info::from("inline-right").display, InfoDisplay::InlineRight);
        assert_eq!(Info::from("hidden").display, InfoDisplay::Hidden);
    }

    #[test]
    fn info_from_str_uses_custom_and_default_separator() {
        // Inline with an explicit separator after the colon.
        assert_eq!(Info::from("inline: | ").separator(), Some(" | "));
        // Inline without a separator falls back to the default.
        assert_eq!(Info::from("inline").separator(), Some(DEFAULT_SEPARATOR));
        // Non-inline modes never carry a separator.
        assert_eq!(Info::from("hidden").separator(), None);
    }

    #[test]
    #[should_panic(expected = "Failed to parse")]
    fn info_from_str_panics_on_unknown_mode() {
        let _ = Info::from("bogus");
    }
}
