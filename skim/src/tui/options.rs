use regex::Regex;
use crate::{
    binds::{parse_keymaps, KeyMap},
    SkimOptions,
};
use color_eyre::Result;

pub struct TuiOptions {
    pub keymap: KeyMap,
    pub preview: Option<String>,
    pub delimiter: Regex
}

impl Default for TuiOptions {
    fn default() -> Self {
        Self {
            keymap: crate::binds::get_default_key_map(),
            preview: None,
            delimiter: Regex::new(r"[\t\n ]+").unwrap()
        }
    }
}

impl TryFrom<&SkimOptions> for TuiOptions {
    type Error = color_eyre::Report;
    fn try_from(value: &SkimOptions) -> Result<Self> {
        Ok(Self {
            keymap: parse_keymaps(value.bind.iter().map(String::as_str))?,
            preview: value.preview.clone(),
            delimiter: Regex::new(&value.delimiter)?
        })
    }
}
