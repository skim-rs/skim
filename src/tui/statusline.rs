/// Default inline info separator
pub const DEFAULT_SEPARATOR: &str = "  < ";

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
