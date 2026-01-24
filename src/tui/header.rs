//! Header display widget for skim's TUI.
//!
//! This module provides the header widget that displays static text above the item list.
use crate::SkimItem;
use crate::SkimOptions;
use crate::theme::ColorTheme;
use crate::theme::DEFAULT_THEME;
use crate::tui::BorderType;
use crate::tui::options::TuiLayout;
use crate::tui::widget::{SkimRender, SkimWidget};

use ansi_to_tui::IntoText;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use ratatui::widgets::{Block, Borders, Paragraph};
use std::cmp::max;
use std::sync::Arc;
use unicode_width::UnicodeWidthChar;

/// Header widget for displaying static text above the item list
#[derive(Clone)]
pub struct Header {
    /// The static header string (from --header option), with expanded tabstop
    pub header: String,
    /// Dynamic header lines from input (from --header-lines option)
    pub header_lines: Vec<Arc<dyn SkimItem>>,
    /// The number of spaces to show before the header
    indent_size: u16,
    /// Tabstop width for expanding tabs in header_lines
    tabstop: usize,
    theme: Arc<ColorTheme>,
    /// Border type, if borders are enabled
    pub border: Option<BorderType>,
    /// Whether to reverse the order of header_lines (for default/bottom-to-top layout)
    reverse_lines: bool,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            header: Default::default(),
            header_lines: Vec::new(),
            indent_size: 0,
            tabstop: 8,
            theme: Arc::new(*DEFAULT_THEME),
            border: None,
            reverse_lines: false,
        }
    }
}

impl Header {
    /// Sets the color theme for the header
    pub fn theme(mut self, theme: Arc<ColorTheme>) -> Self {
        self.theme = theme;
        self
    }
    /// Gets the header height (number of lines)
    pub fn height(&self) -> u16 {
        let static_lines = if self.header.is_empty() {
            0
        } else {
            self.header.lines().count()
        };
        let dynamic_lines = self.header_lines.len();
        (static_lines + dynamic_lines)
            .try_into()
            .expect("Failed to fit header height into an u16")
    }

    /// Sets the dynamic header lines from input (--header-lines)
    pub fn set_header_lines(&mut self, items: Vec<Arc<dyn SkimItem>>) {
        self.header_lines = items;
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

        // In default layout (bottom-to-top), header_lines should be reversed
        // to match the visual flow of the item list
        let reverse_lines = options.layout == TuiLayout::Default;

        Self {
            header: expanded_header,
            header_lines: Vec::new(),
            tabstop,
            indent_size: (options.selector_icon.chars().count() + options.multi_select_icon.chars().count())
                .try_into()
                .expect("Failed to fit selector lens into an u16"),
            theme,
            border: options.border,
            reverse_lines,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) -> SkimRender {
        if area.width < 3 {
            panic!("screen width is too small to fit the header");
        }

        if area.height < 1 {
            panic!("screen height is too small to fit the header");
        }

        let block = if let Some(border_type) = self.border {
            Block::default()
                .borders(Borders::ALL)
                .border_type(border_type.into())
                .border_style(self.theme.border)
        } else {
            Block::default()
        }
        .padding(ratatui::widgets::Padding::left(self.indent_size));

        // Combine static header with dynamic header_lines
        let mut combined_header = String::new();

        // Add dynamic header lines (from --header-lines)
        // In default layout (bottom-to-top), reverse the order to match the list
        if self.reverse_lines {
            for item in self.header_lines.iter().rev() {
                let text = item.text();
                combined_header.push_str(&apply_tabstop(&text, self.tabstop));
                combined_header.push('\n');
            }
        } else {
            for item in &self.header_lines {
                let text = item.text();
                combined_header.push_str(&apply_tabstop(&text, self.tabstop));
                combined_header.push('\n');
            }
        }

        // Add static header (from --header)
        if !self.header.is_empty() {
            combined_header.push_str(&self.header);
        } else if combined_header.ends_with('\n') {
            // Remove trailing newline if there's no static header
            combined_header.pop();
        }

        if let Ok(header) = combined_header.into_text() {
            Paragraph::new(header)
                .style(self.theme.header)
                .block(block)
                .render(area, buf);
        } else {
            Paragraph::new(combined_header.as_str())
                .style(self.theme.header)
                .block(block)
                .render(area, buf);
        }

        SkimRender::default()
    }
}
