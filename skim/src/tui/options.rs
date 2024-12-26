use crate::{
    binds::{parse_keymaps, KeyMap},
    SkimOptions,
};
use color_eyre::Result;

pub struct TuiOptions {
    pub keymap: KeyMap,
}

impl Default for TuiOptions {
    fn default() -> Self {
        Self {
            keymap: crate::binds::get_default_key_map(),
        }
    }
}

impl TryFrom<&SkimOptions> for TuiOptions {
    type Error = color_eyre::Report;
    fn try_from(value: &SkimOptions) -> Result<Self> {
        Ok(Self {
            keymap: parse_keymaps(value.bind.iter().map(String::as_str))?,
        })
    }
}
