//! Header component implementation using ratatui
//! 
//! The Header component displays static header text (specified by --header)
//! and "reserved" header lines (--header-lines) at the top of the interface.

use crate::ansi::{ANSIParser, AnsiString};
use crate::item::ItemPool;
use crate::theme::ColorTheme;
use crate::util::str_lines;
use crate::SkimOptions;
use defer_drop::DeferDrop;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Paragraph, StatefulWidget, Widget},
};
use std::cmp::max;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct HeaderState {
    /// Static header lines from --header option
    pub header_lines: Vec<AnsiString<'static>>,
    /// Reserved items from --header-lines option
    pub reserved_items: Vec<String>,
    /// Tab stop width for rendering
    pub tabstop: usize,
    /// Whether layout is reversed
    pub reverse: bool,
    /// Color theme for styling
    pub theme: Arc<ColorTheme>,
}

impl Default for HeaderState {
    fn default() -> Self {
        Self {
            header_lines: Vec::new(),
            reserved_items: Vec::new(),
            tabstop: 8,
            reverse: false,
            theme: Arc::new(crate::theme::DEFAULT_THEME.clone()),
        }
    }
}

impl HeaderState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update configuration from SkimOptions
    pub fn with_options(&mut self, options: &SkimOptions) {
        self.tabstop = max(1, options.tabstop);
        
        if options.layout.starts_with("reverse") {
            self.reverse = true;
        }

        // Parse static header text
        if let Some(header) = &options.header {
            if !header.is_empty() {
                let mut parser = ANSIParser::default();
                self.header_lines = str_lines(header)
                    .into_iter()
                    .map(|l| parser.parse_ansi(l))
                    .collect();
            }
        }
    }

    /// Update reserved items from item pool
    pub fn update_reserved_items(&mut self, item_pool: &Arc<DeferDrop<ItemPool>>) {
        self.reserved_items = item_pool
            .reserved()
            .iter()
            .map(|item| item.text().to_string())
            .collect();
    }

    /// Get total number of header lines
    pub fn total_lines(&self) -> usize {
        self.header_lines.len() + self.reserved_items.len()
    }

    /// Check if header has any content to display
    pub fn is_empty(&self) -> bool {
        self.header_lines.is_empty() && self.reserved_items.is_empty()
    }

    /// Convert AnsiString to ratatui Text with proper styling
    fn ansi_to_text(&self, ansi_string: &AnsiString) -> Text<'static> {
        let mut spans = Vec::new();
        
        for (ch, attr) in ansi_string.iter() {
            // Convert ANSI attributes to ratatui styles
            let style = Style::default()
                .fg(attr.fg.map(|c| c.into()).unwrap_or(Color::White))
                .bg(attr.bg.map(|c| c.into()).unwrap_or(Color::Reset))
                .add_modifier(attr.effect.into());
            spans.push(Span::styled(ch.to_string(), style));
        }

        Text::from(Line::from(spans))
    }

    /// Get header color from theme
    fn get_header_color(&self, theme: &crate::ui::RatatuiTheme) -> Color {
        theme.header
    }
}

/// Header widget for displaying static and reserved header lines
pub struct Header;

impl StatefulWidget for Header {
    type State = HeaderState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Don't render if no content or area too small
        if state.is_empty() || area.height == 0 {
            return;
        }

        let total_lines = state.total_lines() as u16;
        
        // Ensure we don't try to render more lines than available space
        let lines_to_render = total_lines.min(area.height);
        
        // Create layout for header lines
        let constraints: Vec<Constraint> = (0..lines_to_render)
            .map(|_| Constraint::Length(1))
            .collect();

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        let mut line_index = 0;

        // Render static header lines (from --header)
        for (idx, header_line) in state.header_lines.iter().enumerate() {
            if line_index >= layout.len() {
                break;
            }

            let text = state.ansi_to_text(header_line);
            let paragraph = Paragraph::new(text)
                .block(Block::default());

            let render_area = if state.reverse {
                layout[idx]
            } else {
                layout[layout.len() - state.total_lines() + idx]
            };

            paragraph.render(render_area, buf);
            line_index += 1;
        }

        // Render reserved items (from --header-lines)
        for (idx, item_text) in state.reserved_items.iter().enumerate() {
            if line_index >= layout.len() {
                break;
            }

            let style = Style::default().fg(Color::White); // Simplified for now
            let text = Text::from(Line::from(Span::styled(item_text.clone(), style)));
            let paragraph = Paragraph::new(text)
                .block(Block::default());

            let render_area = if state.reverse {
                layout[state.header_lines.len() + idx]
            } else {
                layout[layout.len() - state.reserved_items.len() + idx]
            };

            paragraph.render(render_area, buf);
            line_index += 1;
        }
    }
}

/// Handle header-specific events (currently no events to handle)
pub fn handle_header_event(
    _state: &mut HeaderState,
    _event: &crate::ui::events::SkimEvent,
) -> Option<crate::ui::events::SkimMessage> {
    // Header is static content, no events to handle
    None
}

/// Render the header component
pub fn render_header(state: &HeaderState, frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
    let mut header_state = state.clone();
    Header.render(area, frame.buffer_mut(), &mut header_state);
}

/// Render the header component with theme
pub fn render_header_with_theme(
    state: &HeaderState, 
    theme: &crate::ui::RatatuiTheme,
    frame: &mut ratatui::Frame, 
    area: ratatui::layout::Rect
) {
    let mut header_state = state.clone();
    // Apply theme colors (theme parameter is available for future use)
    let _header_color = header_state.get_header_color(theme);
    Header.render(area, frame.buffer_mut(), &mut header_state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::SkimOptionsBuilder;

    #[test]
    fn test_header_state_creation() {
        let state = HeaderState::new();
        assert!(state.is_empty());
        assert_eq!(state.total_lines(), 0);
        assert_eq!(state.tabstop, 8);
        assert!(!state.reverse);
    }

    #[test]
    fn test_header_with_options() {
        let options = SkimOptionsBuilder::default()
            .header(Some("Test Header\nLine 2".to_string()))
            .tabstop(4)
            .layout("reverse".to_string())
            .build()
            .unwrap();

        let mut state = HeaderState::new();
        state.with_options(&options);

        assert!(!state.is_empty());
        assert_eq!(state.total_lines(), 2);
        assert_eq!(state.tabstop, 4);
        assert!(state.reverse);
        assert_eq!(state.header_lines.len(), 2);
    }

    #[test]
    fn test_empty_header() {
        let options = SkimOptionsBuilder::default().build().unwrap();
        let mut state = HeaderState::new();
        state.with_options(&options);

        assert!(state.is_empty());
        assert_eq!(state.total_lines(), 0);
    }
}