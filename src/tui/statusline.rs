#[cfg(feature = "cli")]
use clap::ValueEnum;
#[cfg(feature = "cli")]
use clap::builder::PossibleValue;

/// Display mode for the info/status line
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub enum InfoDisplay {
    /// Display info in a separate line (default)
    #[default]
    Default,
    /// Display info inline with the input
    Inline,
    /// Hide the info display
    Hidden,
}

#[cfg(feature = "cli")]
impl ValueEnum for InfoDisplay {
    fn value_variants<'a>() -> &'a [Self] {
        use InfoDisplay::*;
        &[Default, Inline, Hidden]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        use InfoDisplay::*;
        match self {
            Default => Some(PossibleValue::new("default")),
            Inline => Some(PossibleValue::new("inline")),
            Hidden => Some(PossibleValue::new("hidden")),
        }
    }
}
