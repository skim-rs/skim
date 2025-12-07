use crate::tui::{Direction, Size};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
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
    pub offset: Option<String>,
}

impl Default for PreviewLayout {
    fn default() -> Self {
        Self {
            direction: Direction::Right,
            size: Size::Percent(50),
            hidden: false,
            offset: None,
        }
    }
}

impl From<&str> for PreviewLayout {
    fn from(value: &str) -> Self {
        let mut res: Self = PreviewLayout::default();
        // Parse the remainder which can be: size:offset:hidden, offset:hidden, size:hidden, etc.
        let parts: Vec<&str> = value.split(':').collect();

        for part in parts {
            if part.is_empty() {
                continue;
            }

            if part.starts_with('+') {
                // This is an offset expression
                res.offset = Some(part.to_string());
            } else if part == "hidden" {
                res.hidden = true;
            } else if part == "nohidden" {
                res.hidden = false;
            } else {
                // Try to parse as size
                if let Ok(size) = part.try_into() {
                    res.size = size;
                }
                if let Ok(dir) = part.try_into() {
                    res.direction = dir;
                }
            }
        }
        res
    }
}

// impl PreviewLayout {

// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_layout_direction_only() {
        let layout = PreviewLayout::from("left");
        assert_eq!(layout.direction, Direction::Left);
        assert_eq!(layout.size, Size::Percent(50)); // default
        assert_eq!(layout.hidden, false);
        assert_eq!(layout.offset, None);

        let layout = PreviewLayout::from("right");
        assert_eq!(layout.direction, Direction::Right);

        let layout = PreviewLayout::from("up");
        assert_eq!(layout.direction, Direction::Up);

        let layout = PreviewLayout::from("down");
        assert_eq!(layout.direction, Direction::Down);
    }

    #[test]
    fn test_preview_layout_with_size() {
        let layout = PreviewLayout::from("left:30%");
        assert_eq!(layout.direction, Direction::Left);
        assert_eq!(layout.size, Size::Percent(30));
        assert_eq!(layout.hidden, false);
        assert_eq!(layout.offset, None);

        let layout = PreviewLayout::from("right:40");
        assert_eq!(layout.direction, Direction::Right);
        assert_eq!(layout.size, Size::Fixed(40));
    }

    #[test]
    fn test_preview_layout_with_offset() {
        let layout = PreviewLayout::from("left:+123");
        assert_eq!(layout.direction, Direction::Left);
        assert_eq!(layout.offset, Some("+123".to_string()));

        let layout = PreviewLayout::from("left:+{2}");
        assert_eq!(layout.direction, Direction::Left);
        assert_eq!(layout.offset, Some("+{2}".to_string()));

        let layout = PreviewLayout::from("left:+{2}-2");
        assert_eq!(layout.direction, Direction::Left);
        assert_eq!(layout.offset, Some("+{2}-2".to_string()));
    }

    #[test]
    fn test_preview_layout_with_size_and_offset() {
        let layout = PreviewLayout::from("left:50%:+{2}");
        assert_eq!(layout.direction, Direction::Left);
        assert_eq!(layout.size, Size::Percent(50));
        assert_eq!(layout.offset, Some("+{2}".to_string()));

        let layout = PreviewLayout::from("right:40:+123");
        assert_eq!(layout.direction, Direction::Right);
        assert_eq!(layout.size, Size::Fixed(40));
        assert_eq!(layout.offset, Some("+123".to_string()));
    }

    #[test]
    fn test_preview_layout_with_hidden() {
        let layout = PreviewLayout::from("left:hidden");
        assert_eq!(layout.direction, Direction::Left);
        assert_eq!(layout.hidden, true);

        let layout = PreviewLayout::from("right:50%:hidden");
        assert_eq!(layout.direction, Direction::Right);
        assert_eq!(layout.size, Size::Percent(50));
        assert_eq!(layout.hidden, true);
    }

    #[test]
    fn test_preview_layout_complex() {
        let layout = PreviewLayout::from("left:30%:+{2}-5:hidden");
        assert_eq!(layout.direction, Direction::Left);
        assert_eq!(layout.size, Size::Percent(30));
        assert_eq!(layout.offset, Some("+{2}-5".to_string()));
        assert_eq!(layout.hidden, true);
    }
}

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
