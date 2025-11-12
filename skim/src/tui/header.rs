//! header of the items
use crate::SkimOptions;
use crate::theme::ColorTheme;
use crate::theme::DEFAULT_THEME;
use crate::tui::options::TuiLayout;
use crate::tui::widget::{SkimRender, SkimWidget};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::text::Text;
use ratatui::text::ToLine;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use std::cmp::max;
use std::sync::Arc;

#[derive(Clone)]
pub struct Header {
    header: String,
    tabstop: usize,
    reverse: bool,
    theme: Arc<ColorTheme>,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            header: Default::default(),
            tabstop: 8,
            reverse: false,
            theme: Arc::new(*DEFAULT_THEME),
        }
    }
}

impl Header {
    pub fn theme(mut self, theme: Arc<ColorTheme>) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_options(options: &SkimOptions) -> Self {
        Self {
          tabstop: max(1, options.tabstop),
          reverse: options.layout == TuiLayout::Reverse || options.layout == TuiLayout::ReverseList,
          header: options.header.clone().unwrap_or_default(),
          ..Default::default()
        }
    }
}

impl SkimWidget for Header {
    fn from_options(options: &SkimOptions, theme: Arc<ColorTheme>) -> Self {
        Self {
            tabstop: max(1, options.tabstop),
            reverse: options.layout == TuiLayout::Reverse || options.layout == TuiLayout::ReverseList,
            header: options.header.clone().unwrap_or_default(),
            theme,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) -> SkimRender {
        if area.width < 3 {
            panic!("screen width is too small to fit the header");
        }

        if area.height < 1 {
            panic!("screen height is too small to fit the header");
        }

        let header_with_lines = self.header.to_line();
        Paragraph::new(Text::from(header_with_lines)).render(area, buf);

        SkimRender::default()
    }
}
