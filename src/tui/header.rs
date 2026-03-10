//! Header display widget for skim's TUI.
//!
//! This module provides the header widget that displays static text above the item list.
use crate::DisplayContext;
use crate::SkimItem;
use crate::SkimOptions;
use crate::theme::ColorTheme;
use crate::tui::BorderType;
use crate::tui::options::TuiLayout;
use crate::tui::util::char_display_width;
use crate::tui::util::style_line;
use crate::tui::util::style_text;
use crate::tui::widget::{SkimRender, SkimWidget};

use ansi_to_tui::IntoText;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Text;
use ratatui::widgets::Widget;
use ratatui::widgets::{Block, Borders, Paragraph};
use std::cmp::max;
use std::sync::Arc;

/// Header widget for displaying static text above the item list
// The field named `header` in `Header` is intentional — it holds the static
// header string that this widget displays. Renaming it would reduce clarity.
#[allow(clippy::struct_field_names)]
#[derive(Clone, Default)]
pub struct Header {
    /// The static header string (from --header option), with expanded tabstop
    pub header: String,
    /// Dynamic header lines from input (from --header-lines option)
    pub header_lines: Vec<Arc<dyn SkimItem>>,
    /// Fixed number of rows reserved for dynamic header lines (`--header-lines`).
    /// This is set once from the option and never changes, so that the layout
    /// height is stable before the items actually arrive.
    header_lines_count: u16,
    /// The number of spaces to show before the header
    indent_size: u16,
    theme: Arc<ColorTheme>,
    /// Border type, if borders are enabled
    pub border: Option<BorderType>,
    /// Whether to reverse the order of `header_lines` (for default/bottom-to-top layout)
    reverse_lines: bool,
    /// Reverse layout
    reverse: bool,
}

impl Header {
    /// Sets the color theme for the header
    #[must_use]
    pub fn theme(mut self, theme: Arc<ColorTheme>) -> Self {
        self.theme = theme;
        self
    }
    /// Returns the total height (in rows) reserved for this header widget.
    ///
    /// This value is stable at construction time: it is derived purely from
    /// `options.header` (static text) and `options.header_lines` (reserved-item
    /// count), so the layout does not shift as items arrive at runtime.
    #[must_use]
    pub fn height(&self) -> u16 {
        let static_lines = if self.header.is_empty() {
            0
        } else {
            u16::try_from(self.header.lines().count()).unwrap_or(u16::MAX)
        };
        static_lines + self.header_lines_count
    }

    /// Sets the dynamic header lines from input (--header-lines)
    pub fn set_header_lines(&mut self, items: Vec<Arc<dyn SkimItem>>) {
        self.header_lines = items;
        if self.reverse_lines {
            self.header_lines.reverse();
        }
    }
    fn header_text<'a>(&self) -> Text<'a> {
        let mut res = self.header.into_text().unwrap(); //.unwrap_or(Text::from(self.header.clone()));
        style_text(&mut res, self.theme.header);
        res
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
            current_width += char_display_width(ch);
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
            header_lines_count: options
                .header_lines
                .try_into()
                .expect("header_lines count overflows u16"),
            indent_size: (options.selector_icon.chars().count() + options.multi_select_icon.chars().count())
                .try_into()
                .expect("Failed to fit selector lens into an u16"),
            theme,
            border: options.border,
            reverse_lines,
            reverse: options.layout == TuiLayout::Reverse,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) -> SkimRender {
        let block = if let Some(border_type) = self.border {
            Block::default()
                .borders(Borders::ALL)
                .border_type(border_type.into())
                .border_style(self.theme.border)
        } else {
            Block::default()
        }
        .padding(ratatui::widgets::Padding::left(self.indent_size));

        let content_height = if self.header.is_empty() {
            0
        } else {
            self.header_text().lines.len()
        } + self.header_lines.len();

        let container_height = if self.border.is_some() {
            area.height - 2
        } else {
            area.height
        };

        // Combine static header with dynamic header_lines
        let mut combined_header = if self.reverse && !self.header.is_empty() {
            self.header_text()
        } else {
            let mut r = Text::default();
            for _ in 0..(container_height.saturating_sub(content_height.try_into().unwrap())) {
                r.push_line("");
            }
            r
        };

        let display_context = DisplayContext {
            base_style: self.theme.header,
            ..Default::default()
        };

        for item in &self.header_lines {
            let mut line = item.display(display_context.clone());
            style_line(&mut line, self.theme.header);
            combined_header.push_line(line);
        }

        // Add static header (from --header)
        if !self.reverse && !self.header.is_empty() {
            combined_header += self.header_text();
        }

        Paragraph::new(combined_header)
            .style(self.theme.header)
            .block(block)
            .render(area, buf);

        SkimRender::default()
    }
}
