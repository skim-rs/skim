//! Previewer component implementation using ratatui
//! 
//! The Previewer component displays content preview for selected items:
//! - Plain text or ANSI text from items
//! - Command output execution for dynamic preview
//! - Scrollable content with keyboard navigation
//! - Mouse wheel support for scrolling

use crate::ansi::{ANSIParser, AnsiString};
use crate::theme::ColorTheme;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget, Wrap},
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PreviewerState {
    /// Content lines to display (with ANSI support)
    pub content_lines: Vec<AnsiString<'static>>,
    /// Whether the previewer is visible
    pub visible: bool,
    /// Vertical scroll offset
    pub vscroll_offset: usize,
    /// Horizontal scroll offset  
    pub hscroll_offset: usize,
    /// Whether to wrap long lines
    pub wrap: bool,
    /// Width and height of preview area
    pub width: usize,
    pub height: usize,
    /// Current preview command/source info
    pub preview_source: String,
    /// Color theme for styling
    pub theme: Arc<ColorTheme>,
}

impl Default for PreviewerState {
    fn default() -> Self {
        Self {
            content_lines: Vec::new(),
            visible: false,
            vscroll_offset: 0,
            hscroll_offset: 0,
            wrap: false,
            width: 80,
            height: 24,
            preview_source: String::new(),
            theme: Arc::new(crate::theme::DEFAULT_THEME.clone()),
        }
    }
}

impl PreviewerState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set preview content from plain text
    pub fn set_plain_text(&mut self, text: String) {
        self.content_lines = text
            .lines()
            .map(|line| AnsiString::parse(line))
            .collect();
        self.vscroll_offset = 0;
        self.hscroll_offset = 0;
    }

    /// Set preview content from ANSI text
    pub fn set_ansi_text(&mut self, text: String) {
        let mut parser = ANSIParser::default();
        self.content_lines = text
            .lines()
            .map(|line| parser.parse_ansi(line))
            .collect();
        self.vscroll_offset = 0;
        self.hscroll_offset = 0;
    }

    /// Set preview content from pre-parsed ANSI strings
    pub fn set_content_lines(&mut self, lines: Vec<AnsiString<'static>>) {
        self.content_lines = lines;
        self.vscroll_offset = 0;
        self.hscroll_offset = 0;
    }

    /// Show/hide the previewer
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Toggle line wrapping
    pub fn toggle_wrap(&mut self) {
        self.wrap = !self.wrap;
    }

    /// Scroll vertically by the given amount (positive = down, negative = up)
    pub fn scroll_vertical(&mut self, amount: i32) {
        if amount > 0 {
            let max_offset = self.content_lines.len().saturating_sub(self.height);
            self.vscroll_offset = (self.vscroll_offset + amount as usize).min(max_offset);
        } else {
            self.vscroll_offset = self.vscroll_offset.saturating_sub((-amount) as usize);
        }
    }

    /// Scroll horizontally by the given amount (positive = right, negative = left)
    pub fn scroll_horizontal(&mut self, amount: i32) {
        if amount > 0 {
            self.hscroll_offset += amount as usize;
        } else {
            self.hscroll_offset = self.hscroll_offset.saturating_sub((-amount) as usize);
        }
    }

    /// Scroll to page up/down
    pub fn scroll_page(&mut self, pages: i32) {
        let page_size = self.height.max(1);
        self.scroll_vertical(pages * page_size as i32);
    }

    /// Update dimensions
    pub fn update_size(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
    }

    /// Get scroll position as percentage
    pub fn scroll_percentage(&self) -> u16 {
        if self.content_lines.is_empty() {
            return 0;
        }
        let max_offset = self.content_lines.len().saturating_sub(self.height);
        if max_offset == 0 {
            return 0;
        }
        ((self.vscroll_offset as f64 / max_offset as f64) * 100.0) as u16
    }

    /// Convert AnsiString to ratatui Text with proper styling
    fn ansi_to_text(&self, ansi_string: &AnsiString) -> Line<'static> {
        let mut spans = Vec::new();
        
        for (ch, _attr) in ansi_string.iter() {
            // For now, use default styling - proper ANSI conversion will be added with theme migration
            let style = Style::default().fg(self.get_text_color());
            spans.push(Span::styled(ch.to_string(), style));
        }

        Line::from(spans)
    }

    /// Get default text color (will be replaced with proper theme conversion)
    fn get_text_color(&self) -> Color {
        Color::White
    }

    fn get_border_color(&self) -> Color {
        Color::Blue
    }

    fn get_scrollbar_color(&self) -> Color {
        Color::Gray
    }
}

/// Previewer widget for displaying content preview
pub struct Previewer;

impl StatefulWidget for Previewer {
    type State = PreviewerState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Don't render if not visible or too small
        if !state.visible || area.width < 3 || area.height < 3 {
            return;
        }

        // Update state dimensions
        state.update_size(area.width as usize - 2, area.height as usize - 2); // Account for borders

        // Create the main block with borders
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(state.get_border_color()))
            .title("Preview");

        let inner_area = block.inner(area);
        block.render(area, buf);

        // Prepare content for rendering
        let visible_lines: Vec<Line> = state.content_lines
            .iter()
            .skip(state.vscroll_offset)
            .take(inner_area.height as usize)
            .map(|line| state.ansi_to_text(line))
            .collect();

        let text = Text::from(visible_lines);

        // Create paragraph widget
        let paragraph = if state.wrap {
            Paragraph::new(text).wrap(Wrap { trim: false })
        } else {
            Paragraph::new(text)
        };

        // Render the paragraph
        paragraph.render(inner_area, buf);

        // Render scrollbar if content is larger than visible area
        if state.content_lines.len() > inner_area.height as usize {
            let scrollbar_area = Rect {
                x: area.x + area.width - 1,
                y: area.y + 1,
                width: 1,
                height: area.height - 2,
            };

            let mut scrollbar_state = ScrollbarState::default()
                .content_length(state.content_lines.len())
                .position(state.vscroll_offset);

            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .style(Style::default().fg(state.get_scrollbar_color()))
                .render(scrollbar_area, buf, &mut scrollbar_state);
        }

        // Show scroll position in bottom right corner if there's content
        if !state.content_lines.is_empty() {
            let status = format!("{}/{}", state.vscroll_offset + 1, state.content_lines.len());
            let status_x = area.x + area.width.saturating_sub(status.len() as u16 + 1);
            let status_y = area.y + area.height - 1;
            
            if status_x > area.x && status_y >= area.y {
                let status_area = Rect {
                    x: status_x,
                    y: status_y,
                    width: status.len() as u16,
                    height: 1,
                };
                
                let status_paragraph = Paragraph::new(status)
                    .style(Style::default()
                        .fg(Color::White)
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD));
                        
                status_paragraph.render(status_area, buf);
            }
        }
    }
}

/// Handle previewer-specific events
pub fn handle_previewer_event(
    state: &mut PreviewerState,
    event: &crate::ui::events::SkimEvent,
) -> Option<crate::ui::events::SkimMessage> {
    use crate::ui::events::SkimEvent;
    use crossterm::event::{KeyCode, KeyModifiers};

    if !state.visible {
        return None;
    }

    match event {
        SkimEvent::Key(key_event) => {
            match (key_event.code, key_event.modifiers) {
                // Vertical scrolling
                (KeyCode::Up, _) => {
                    state.scroll_vertical(-1);
                    Some(crate::ui::events::SkimMessage::PreviewScroll)
                }
                (KeyCode::Down, _) => {
                    state.scroll_vertical(1);
                    Some(crate::ui::events::SkimMessage::PreviewScroll)
                }
                (KeyCode::PageUp, _) => {
                    state.scroll_page(-1);
                    Some(crate::ui::events::SkimMessage::PreviewScroll)
                }
                (KeyCode::PageDown, _) => {
                    state.scroll_page(1);
                    Some(crate::ui::events::SkimMessage::PreviewScroll)
                }
                
                // Horizontal scrolling
                (KeyCode::Left, _) => {
                    state.scroll_horizontal(-4);
                    Some(crate::ui::events::SkimMessage::PreviewScroll)
                }
                (KeyCode::Right, _) => {
                    state.scroll_horizontal(4);
                    Some(crate::ui::events::SkimMessage::PreviewScroll)
                }
                
                // Home/End
                (KeyCode::Home, _) => {
                    state.vscroll_offset = 0;
                    Some(crate::ui::events::SkimMessage::PreviewScroll)
                }
                (KeyCode::End, _) => {
                    let max_offset = state.content_lines.len().saturating_sub(state.height);
                    state.vscroll_offset = max_offset;
                    Some(crate::ui::events::SkimMessage::PreviewScroll)
                }
                
                // Toggle wrap
                (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                    state.toggle_wrap();
                    Some(crate::ui::events::SkimMessage::PreviewToggleWrap)
                }
                
                _ => None,
            }
        }
        
        SkimEvent::Mouse(mouse_event) => {
            use crossterm::event::{MouseEvent, MouseEventKind};
            match mouse_event.kind {
                MouseEventKind::ScrollUp => {
                    state.scroll_vertical(-3);
                    Some(crate::ui::events::SkimMessage::PreviewScroll)
                }
                MouseEventKind::ScrollDown => {
                    state.scroll_vertical(3);
                    Some(crate::ui::events::SkimMessage::PreviewScroll)
                }
                _ => None,
            }
        }
        
        _ => None,
    }
}

/// Render the previewer component
pub fn render_preview(state: &PreviewerState, frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
    let mut previewer_state = state.clone();
    Previewer.render(area, frame.buffer_mut(), &mut previewer_state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_previewer_state_creation() {
        let state = PreviewerState::new();
        assert!(!state.visible);
        assert!(state.content_lines.is_empty());
        assert_eq!(state.vscroll_offset, 0);
        assert_eq!(state.hscroll_offset, 0);
        assert!(!state.wrap);
    }

    #[test]
    fn test_set_plain_text() {
        let mut state = PreviewerState::new();
        state.set_plain_text("Line 1\nLine 2\nLine 3".to_string());
        
        assert_eq!(state.content_lines.len(), 3);
        assert_eq!(state.vscroll_offset, 0);
        assert_eq!(state.hscroll_offset, 0);
    }

    #[test]
    fn test_scrolling() {
        let mut state = PreviewerState::new();
        state.set_plain_text("Line 1\nLine 2\nLine 3\nLine 4\nLine 5".to_string());
        state.height = 3;

        // Test vertical scrolling
        state.scroll_vertical(2);
        assert_eq!(state.vscroll_offset, 2);

        // Test scroll bounds
        state.scroll_vertical(10);
        assert_eq!(state.vscroll_offset, 2); // Should not exceed max

        state.scroll_vertical(-5);
        assert_eq!(state.vscroll_offset, 0); // Should not go below 0

        // Test horizontal scrolling
        state.scroll_horizontal(5);
        assert_eq!(state.hscroll_offset, 5);

        state.scroll_horizontal(-3);
        assert_eq!(state.hscroll_offset, 2);
    }

    #[test]
    fn test_scroll_percentage() {
        let mut state = PreviewerState::new();
        state.set_plain_text("Line 1\nLine 2\nLine 3\nLine 4\nLine 5".to_string());
        state.height = 3;

        assert_eq!(state.scroll_percentage(), 0);

        state.vscroll_offset = 1;
        assert_eq!(state.scroll_percentage(), 50);

        state.vscroll_offset = 2;
        assert_eq!(state.scroll_percentage(), 100);
    }

    #[test]
    fn test_page_scrolling() {
        let mut state = PreviewerState::new();
        state.set_plain_text((0..20).map(|i| format!("Line {}", i)).collect::<Vec<_>>().join("\n"));
        state.height = 5;

        state.scroll_page(1);
        assert_eq!(state.vscroll_offset, 5);

        state.scroll_page(1);
        assert_eq!(state.vscroll_offset, 10);

        state.scroll_page(-1);
        assert_eq!(state.vscroll_offset, 5);
    }

    #[test]
    fn test_toggle_wrap() {
        let mut state = PreviewerState::new();
        assert!(!state.wrap);

        state.toggle_wrap();
        assert!(state.wrap);

        state.toggle_wrap();
        assert!(!state.wrap);
    }
}