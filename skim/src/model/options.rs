use clap::builder::PossibleValue;
use clap::ValueEnum;

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub enum InfoDisplay {
    #[default]
    Default,
    Inline,
    Hidden,
}

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
