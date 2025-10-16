use crate::{
    SkimOptions,
    binds::{KeyMap, parse_keymaps},
    tui::{Direction, Size},
};
use clap::ValueEnum;
use color_eyre::Result;
use regex::Regex;

#[derive(Default, Debug, Clone, Copy, clap::ValueEnum, PartialEq, Eq)]
pub enum TuiLayout {
    /// Display from the bottom of the screen
    #[default]
    Default,
    /// Display from the top of the screen
    Reverse,
    /// Display from the top of the screen, prompt at the bottom
    ReverseList,
}

#[derive(Debug, Clone)]
pub struct PreviewLayout {
    pub direction: Direction,
    pub size: Size,
    pub hidden: bool,
}

impl Default for PreviewLayout {
    fn default() -> Self {
        Self {
            direction: Direction::Right,
            size: Size::Percent(50),
            hidden: false,
        }
    }
}

impl From<&str> for PreviewLayout {
    fn from(value: &str) -> Self {
        let mut res: Self = PreviewLayout::default();
        if let Some((dir, remainder)) = value.split_once(':') {
            res.direction = dir.into();
            let mut size = remainder;
            if let Some((size_p, remainder)) = remainder.split_once(':') {
                if let Some((hidden, _)) = remainder.split_once(':') {
                    res.hidden = hidden == "hidden";
                }
                size = size_p;
            }
            res.size = size.try_into().expect("Invalid size {size}");
        }
        res
    }
}

// impl PreviewLayout {

// }

// pub struct TuiOptions {
//     pub keymap: KeyMap,
//     pub preview: Option<String>,
//     pub preview_window: PreviewLayout,
//     pub delimiter: Regex,
//     pub min_query_length: Option<usize>,
//     pub multi: bool,
//     pub use_regex: bool,
//     pub interactive: bool,
//     pub border: bool,
//     pub prompt: String,
//     pub header: Option<String>,
//     pub layout: TuiLayout,
//     pub tabstop: usize,
// }

// impl Default for TuiOptions {
//     fn default() -> Self {
//         Self {
//             keymap: crate::binds::get_default_key_map(),
//             preview: None,
//             preview_window: PreviewLayout::default(),
//             delimiter: Regex::new(r"[\t\n ]+").unwrap(),
//             min_query_length: None,
//             multi: false,
//             use_regex: false,
//             interactive: false,
//             border: false,
//             prompt: String::from("> "),
//             header: None,
//             layout: Default::default(),
//             tabstop: 8,
//         }
//     }
// }

// impl TryFrom<&SkimOptions> for TuiOptions {
//     type Error = color_eyre::Report;
//     fn try_from(value: &SkimOptions) -> Result<Self> {
//         Ok(Self {
//             keymap: parse_keymaps(value.bind.iter().map(String::as_str))?,
//             preview: value.preview.clone(),
//             preview_window: value.preview_window.as_str().into(),
//             delimiter: Regex::new(&value.delimiter)?,
//             min_query_length: None, // TODO
//             multi: value.multi,
//             use_regex: value.regex,
//             interactive: value.interactive,
//             border: value.border,
//             prompt: value.prompt.clone(),
//             header: value.header.clone(),
//             layout: value.layout,
//             tabstop: value.tabstop,
//         })
//     }
// }
