/// Default inline info separator
pub const DEFAULT_SEPARATOR: &str = "  < ";

/// Display mode for the info/status line
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub enum InfoDisplay {
    /// Display info in a separate line (default)
    #[default]
    Default,
    /// Display info inline with the input
    Inline(String),
    /// Hide the info display
    Hidden,
    /// Inline and right-aligned
    InlineRight(String),
}

impl InfoDisplay {
    pub(crate) fn separator(&self) -> Option<String> {
        if let InfoDisplay::Inline(s) = self {
            Some(s.clone())
        } else if let InfoDisplay::InlineRight(s) = self {
            Some(s.clone())
        } else {
            None
        }
    }
}

impl From<&str> for InfoDisplay {
    fn from(s: &str) -> Self {
        use InfoDisplay::{Default, Hidden, Inline, InlineRight};
        let (variant, separator) = s.split_once(':').unwrap_or((s, DEFAULT_SEPARATOR));

        match variant {
            "default" => Default,
            "inline" => Inline(separator.to_string()),
            "inline-right" => InlineRight(separator.to_string()),
            "hidden" => Hidden,
            x => panic!(
                "Failed to parse {x} as an InfoDisplay. Possible options are `default`, `inline[:separator]`, `inline-right:[separator]` or `hidden`"
            ),
        }
    }
}
