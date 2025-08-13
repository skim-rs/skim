use crate::{
    binds::{parse_keymaps, KeyMap}, tui::{Direction, Size}, SkimOptions
};
use color_eyre::{eyre::Context, owo_colors::OwoColorize, Result};
use regex::Regex;

pub struct PreviewLayout {
  pub direction: Direction,
  pub size: Size,
  pub hidden: bool,
}

impl Default for PreviewLayout {
    fn default() -> Self {
        Self { direction: Direction::Right, size: Size::Percent(50), hidden: false }
    }
}

impl From<&str> for PreviewLayout {
    fn from(value: &str) -> Self {
        let mut res: Self = PreviewLayout::default();
        if let Some((dir, remainder)) =  value.split_once(':') {
          res.direction = dir.into();
          let mut size = remainder;
          if let Some((size_p, remainder)) = remainder.split_once(':') {
            if let Some((hidden, _)) = remainder.split_once(':') {
              res.hidden = (hidden == "hidden");
            }
            size = size_p;
          }
          res.size = size.try_into().expect("Invalid size {size}");
        }
        res
    }
}

pub struct TuiOptions {
    pub keymap: KeyMap,
    pub preview: Option<String>,
    pub preview_window: PreviewLayout,
    pub delimiter: Regex,
    pub min_query_length: Option<usize>,
    pub(crate) use_regex: bool,
}

impl Default for TuiOptions {
    fn default() -> Self {
        Self {
            keymap: crate::binds::get_default_key_map(),
            preview: None,
            preview_window: PreviewLayout::default(),
            delimiter: Regex::new(r"[\t\n ]+").unwrap(),
            min_query_length: None,
            use_regex: false,
        }
    }
}

impl TryFrom<&SkimOptions> for TuiOptions {
    type Error = color_eyre::Report;
    fn try_from(value: &SkimOptions) -> Result<Self> {
        Ok(Self {
            keymap: parse_keymaps(value.bind.iter().map(String::as_str))?,
            preview: value.preview.clone(),
            preview_window: value.preview_window.as_str().into(),
            delimiter: Regex::new(&value.delimiter)?,
            min_query_length: None, // TODO
            use_regex: value.regex,
        })
    }
}
