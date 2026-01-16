//! Header display widget for skim's TUI.
//!
//! This module provides the header widget that displays static text above the item list.
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
use unicode_width::UnicodeWidthChar;

/// Header widget for displaying static text above the item list
#[derive(Clone)]
pub struct Header {
    /// The header string with expanded tabstop, as will be displayed on screen
    pub header: String,
    theme: Arc<ColorTheme>,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            header: Default::default(),
            theme: Arc::new(*DEFAULT_THEME),
        }
    }
}

impl Header {
    /// Sets the color theme for the header
    pub fn theme(mut self, theme: Arc<ColorTheme>) -> Self {
        self.theme = theme;
        self
    }
}

/// Expands tab characters to spaces based on tabstop width and current position
fn apply_tabstop(text: &str, tabstop: usize) -> String {
    let mut result = String::new();
    let mut current_width = 0;

    for ch in text.chars() {
        if ch == '\t' {
            let tab_width = tabstop - (current_width % tabstop);
            result.push_str(&" ".repeat(tab_width));
            current_width += tab_width;
        } else {
            result.push(ch);
            current_width += ch.width_cjk().unwrap_or(0);
        }
    }

    result
}

impl SkimWidget for Header {
    fn from_options(options: &SkimOptions, theme: Arc<ColorTheme>) -> Self {
        let tabstop = max(1, options.tabstop);
        let header = options.header.clone().unwrap_or_default();

        // Expand tabs once during initialization
        let expanded_header = apply_tabstop(&header, tabstop);

        Self {
            header: expanded_header,
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

        Paragraph::new(self.header.as_str()).render(area, buf);

        SkimRender::default()
    }
}
