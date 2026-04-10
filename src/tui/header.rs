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
use crate::tui::util::clip_line_to_chars;
use crate::tui::util::style_line;
use crate::tui::util::style_text;
use crate::tui::widget::{SkimRender, SkimWidget};

use ansi_to_tui::IntoText;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Widget;
use ratatui::widgets::{Block, Borders, Paragraph};
use std::cmp::max;
use std::sync::Arc;

/// Header widget for displaying static text above the item list
// The field named `header` in `Header` is intentional — it holds the static
// header string that this widget displays. Renaming it would reduce clarity.
#[allow(clippy::struct_field_names)]
#[derive(Clone)]
pub struct Header {
    /// The static header string (from --header option), with expanded tabstop
    pub header: String,
    /// Dynamic header lines from input (from --header-lines option)
    pub header_lines: Vec<Arc<dyn SkimItem>>,
    /// Fixed number of rows reserved for dynamic header lines (`--header-lines`).
    /// Used as the row estimate before items arrive; once items are available
    /// `height()` counts the actual sub-lines produced by multiline splitting.
    header_lines_count: u16,
    /// When `--multiline` is active, the separator string used to split each
    /// header-line item into multiple display rows.
    multiline: Option<String>,
    /// The number of spaces to show before the header
    indent_size: u16,
    theme: Arc<ColorTheme>,
    /// Border type
    pub border: BorderType,
    /// Whether to reverse the order of `header_lines` (for default/bottom-to-top layout)
    reverse_lines: bool,
    /// Reverse layout
    reverse: bool,
}

impl Default for Header {
    fn default() -> Self {
        Self::_default()
    }
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
        // Once items have arrived, count actual terminal rows (accounting for
        // multiline splitting).  Before items arrive fall back to the
        // compile-time estimate so the layout is stable on the first frame.
        let dynamic_lines = if self.header_lines.is_empty() {
            self.header_lines_count
        } else if let Some(sep) = self.multiline.as_deref() {
            self.header_lines
                .iter()
                .map(|item| u16::try_from(item.text().split(sep).count().max(1)).unwrap_or(1))
                .sum()
        } else {
            u16::try_from(self.header_lines.len()).unwrap_or(u16::MAX)
        };
        static_lines + dynamic_lines
    }

    /// Sets the dynamic header lines from input (--header-lines)
    pub fn set_header_lines(&mut self, items: Vec<Arc<dyn SkimItem>>) {
        self.header_lines = items;
        if self.reverse_lines {
            self.header_lines.reverse();
        }
    }
    fn header_text<'a>(&self) -> Text<'a> {
        let mut res = self.header.into_text().unwrap();
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
            multiline: options.multiline.as_ref().and_then(std::clone::Clone::clone),
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
        let block = if let Some(border_type) = self.border.into_ratatui() {
            Block::default()
                .borders(Borders::ALL)
                .border_type(border_type)
                .border_style(self.theme.border)
        } else {
            Block::default()
        }
        .padding(ratatui::widgets::Padding::left(self.indent_size));

        let content_height = if self.header.is_empty() {
            0
        } else {
            self.header_text().lines.len()
        } + if let Some(sep) = self.multiline.as_deref() {
            self.header_lines
                .iter()
                .map(|item| item.text().split(sep).count().max(1))
                .sum()
        } else {
            self.header_lines.len()
        };

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
            if let Some(sep) = self.multiline.as_deref() {
                let item_text = item.text();
                let sub_lines: Vec<&str> = item_text.split(sep).collect();

                // First sub-line: call display() so that ANSI styling is applied,
                // then clip the result to the character count of that sub-part.
                let first_char_len = sub_lines.first().map_or(0, |s| s.chars().count());
                let full_display = item.display(display_context.clone());
                let mut first_line = clip_line_to_chars(full_display, first_char_len);
                style_line(&mut first_line, self.theme.header);
                combined_header.push_line(first_line);

                // Remaining sub-lines: plain styled text (no ANSI re-processing needed).
                for sub_text in sub_lines.iter().skip(1) {
                    let mut line = Line::from(vec![Span::styled(sub_text.to_string(), display_context.base_style)]);
                    style_line(&mut line, self.theme.header);
                    combined_header.push_line(line);
                }
            } else {
                let mut line = item.display(display_context.clone());
                style_line(&mut line, self.theme.header);
                combined_header.push_line(line);
            }
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
