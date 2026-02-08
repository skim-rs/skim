use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::ops::BitOrAssign;
use std::sync::Arc;

use crate::options::SkimOptions;
use crate::theme::ColorTheme;

/// Result of rendering a SkimWidget
#[derive(Debug, Clone, Copy, Default)]
pub struct SkimRender {
    /// Whether the items in the list have been updated
    pub items_updated: bool,
    /// Whether or not we need to reload the preview
    pub run_preview: bool,
}

impl BitOrAssign for SkimRender {
    fn bitor_assign(&mut self, rhs: Self) {
        self.items_updated |= rhs.items_updated;
        self.run_preview |= rhs.run_preview;
    }
}

/// Trait for Skim TUI widgets
pub trait SkimWidget: Sized {
    /// Create a widget from options and theme
    fn from_options(options: &SkimOptions, theme: Arc<ColorTheme>) -> Self;

    /// Render the widget to the buffer
    fn render(&mut self, area: Rect, buf: &mut Buffer) -> SkimRender;
}
