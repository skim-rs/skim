//! header of the items
use crate::SkimOptions;
use crate::theme::ColorTheme;
use crate::theme::DEFAULT_THEME;
use crate::tui::widget::{SkimRender, SkimWidget};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use std::cmp::max;
use std::sync::Arc;

#[derive(Clone)]
pub struct Header {
    header: String,
    tabstop: String,
    theme: Arc<ColorTheme>,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            header: Default::default(),
            tabstop: String::from_utf8(vec![b' '; 8]).unwrap(),
            theme: Arc::new(*DEFAULT_THEME),
        }
    }
}

impl Header {
    pub fn theme(mut self, theme: Arc<ColorTheme>) -> Self {
        self.theme = theme;
        self
    }
}

impl SkimWidget for Header {
    fn from_options(options: &SkimOptions, theme: Arc<ColorTheme>) -> Self {
        Self {
            tabstop: String::from_utf8(vec![b' '; max(1, options.tabstop)]).unwrap(),
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

        Paragraph::new(self.header.as_str().replace('\t', &self.tabstop)).render(area, buf); // TODO use actual tabstop

        SkimRender::default()
    }
}
