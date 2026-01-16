use crate::tui::{Direction, Size};

/// Layout configuration for the TUI
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

/// Configuration for the preview pane layout
#[derive(Debug, Clone)]
pub struct PreviewLayout {
    /// Direction where preview pane is positioned
    pub direction: Direction,
    /// Size of the preview pane
    pub size: Size,
    /// Whether the preview pane is hidden
    pub hidden: bool,
    /// Optional offset for preview position
    pub offset: Option<String>,
    /// Whether or not to wrap the preview contents
    pub wrap: bool,
}

impl Default for PreviewLayout {
    fn default() -> Self {
        Self {
            direction: Direction::Right,
            size: Size::Percent(50),
            hidden: false,
            offset: None,
            wrap: false,
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
            } else if part == "wrap" {
                res.wrap = true;
            } else if part == "nowrap" {
                res.wrap = false;
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
