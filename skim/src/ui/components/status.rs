//! Status component implementation using ratatui
//! 
//! The Status component displays current information about the fuzzy finder state:
//! - Number of matched/total items
//! - Processing progress with spinner
//! - Multi-selection count
//! - Current item position
//! - Matcher mode and progress

use crate::theme::ColorTheme;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Paragraph, StatefulWidget, Widget},
};
use std::sync::Arc;
use std::time::Duration;

const SPINNER_DURATION: u32 = 200;
const SPINNERS_INLINE: [char; 2] = ['-', '<'];
const SPINNERS_UNICODE: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfoDisplay {
    Default,
    Inline,
    Hidden,
}

impl Default for InfoDisplay {
    fn default() -> Self {
        InfoDisplay::Default
    }
}

#[derive(Debug, Clone)]
pub struct StatusState {
    /// Total number of items
    pub total: usize,
    /// Number of matched items
    pub matched: usize,
    /// Number of processed items (for progress percentage)
    pub processed: usize,
    /// Whether the matcher is currently running
    pub matcher_running: bool,
    /// Whether multi-selection is enabled
    pub multi_selection: bool,
    /// Number of selected items in multi-selection mode
    pub selected: usize,
    /// Index of current item
    pub current_item_idx: usize,
    /// Horizontal scroll offset
    pub hscroll_offset: i64,
    /// Whether input is being read
    pub reading: bool,
    /// Time since last read operation
    pub time_since_read: Duration,
    /// Time since last match operation
    pub time_since_match: Duration,
    /// Current matcher mode string
    pub matcher_mode: String,
    /// Info display mode
    pub info_display: InfoDisplay,
    /// Color theme for styling
    pub theme: Arc<ColorTheme>,
}

impl Default for StatusState {
    fn default() -> Self {
        Self {
            total: 0,
            matched: 0,
            processed: 0,
            matcher_running: false,
            multi_selection: false,
            selected: 0,
            current_item_idx: 0,
            hscroll_offset: 0,
            reading: false,
            time_since_read: Duration::from_secs(0),
            time_since_match: Duration::from_secs(0),
            matcher_mode: String::new(),
            info_display: InfoDisplay::Default,
            theme: Arc::new(crate::theme::DEFAULT_THEME.clone()),
        }
    }
}

impl StatusState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update status information
    pub fn update_status(
        &mut self,
        total: usize,
        matched: usize,
        processed: usize,
        selected: usize,
        current_idx: usize,
    ) {
        self.total = total;
        self.matched = matched;
        self.processed = processed;
        self.selected = selected;
        self.current_item_idx = current_idx;
    }

    /// Set matcher running state
    pub fn set_matcher_running(&mut self, running: bool) {
        self.matcher_running = running;
    }

    /// Set reading state and update time
    pub fn set_reading(&mut self, reading: bool, time_since_read: Duration) {
        self.reading = reading;
        self.time_since_read = time_since_read;
    }

    /// Update time since match
    pub fn set_time_since_match(&mut self, time: Duration) {
        self.time_since_match = time;
    }

    /// Set matcher mode string
    pub fn set_matcher_mode(&mut self, mode: String) {
        self.matcher_mode = mode;
    }

    /// Set horizontal scroll offset
    pub fn set_hscroll_offset(&mut self, offset: i64) {
        self.hscroll_offset = offset;
    }

    /// Get spinner character based on timing and mode
    fn get_spinner_char(&self) -> char {
        if matches!(self.info_display, InfoDisplay::Hidden) {
            return ' ';
        }
        
        let spinner_set: &[char] = match self.info_display {
            InfoDisplay::Default => &SPINNERS_UNICODE,
            InfoDisplay::Inline => &SPINNERS_INLINE,
            InfoDisplay::Hidden => unreachable!(), // Already handled above
        };

        if self.reading && self.time_since_read > Duration::from_millis(50) {
            let mills = (self.time_since_read.as_secs() * 1000) as u32 
                + self.time_since_read.subsec_millis();
            let index = (mills / SPINNER_DURATION) % (spinner_set.len() as u32);
            spinner_set[index as usize]
        } else {
            match self.info_display {
                InfoDisplay::Inline => '<',
                InfoDisplay::Default => ' ',
                InfoDisplay::Hidden => ' ',
            }
        }
    }

    /// Get default status colors (will be replaced with proper theme conversion)
    fn get_info_color(&self) -> Color {
        Color::Blue
    }

    fn get_info_bold_color(&self) -> Color {
        Color::Cyan
    }

    fn get_spinner_color(&self) -> Color {
        Color::Yellow
    }

    fn get_prompt_color(&self) -> Color {
        Color::Green
    }
}

/// Status widget for displaying fuzzy finder status information
pub struct Status;

impl StatefulWidget for Status {
    type State = StatusState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Don't render if hidden
        if state.info_display == InfoDisplay::Hidden || area.width == 0 {
            return;
        }

        let mut spans = Vec::new();

        // Add initial space for inline mode
        if state.info_display == InfoDisplay::Inline {
            spans.push(Span::raw(" "));
        }

        // Add spinner
        let spinner_char = state.get_spinner_char();
        let spinner_style = if state.reading && state.time_since_read > Duration::from_millis(50) {
            Style::default().fg(state.get_spinner_color())
        } else {
            Style::default().fg(state.get_prompt_color())
        };
        spans.push(Span::styled(spinner_char.to_string(), spinner_style));

        // Add matched/total count
        let count_text = format!(" {}/{}", state.matched, state.total);
        spans.push(Span::styled(count_text, Style::default().fg(state.get_info_color())));

        // Add matcher mode if present
        if !state.matcher_mode.is_empty() {
            let mode_text = format!("/{}", state.matcher_mode);
            spans.push(Span::styled(mode_text, Style::default().fg(state.get_info_color())));
        }

        // Add processing percentage if matcher is running
        if state.matcher_running 
            && state.time_since_match > Duration::from_millis(50) 
            && state.total > 0 
        {
            let percentage = state.processed * 100 / state.total;
            let progress_text = format!(" ({}%) ", percentage);
            spans.push(Span::styled(progress_text, Style::default().fg(state.get_info_color())));
        }

        // Add multi-selection count if active
        if state.multi_selection && state.selected > 0 {
            let selected_text = format!(" [{}]", state.selected);
            spans.push(Span::styled(
                selected_text,
                Style::default()
                    .fg(state.get_info_bold_color())
                    .add_modifier(Modifier::BOLD),
            ));
        }

        // Calculate available space for right-aligned cursor info
        let left_content_width: usize = spans.iter()
            .map(|span| span.content.len())
            .sum();

        // Add item cursor position (right-aligned)
        let cursor_text = format!(
            " {}/{}{}",
            state.current_item_idx,
            state.hscroll_offset,
            if state.matcher_running { '.' } else { ' ' }
        );

        // Fill space between left content and right-aligned cursor
        let available_width = area.width as usize;
        if left_content_width + cursor_text.len() < available_width {
            let padding = available_width - left_content_width - cursor_text.len();
            spans.push(Span::raw(" ".repeat(padding)));
        }

        spans.push(Span::styled(
            cursor_text,
            Style::default()
                .fg(state.get_info_bold_color())
                .add_modifier(Modifier::BOLD),
        ));

        let line = Line::from(spans);
        let paragraph = Paragraph::new(Text::from(line))
            .block(Block::default());

        paragraph.render(area, buf);
    }
}

/// Handle status-specific events (currently no events to handle)
pub fn handle_status_event(
    _state: &mut StatusState,
    _event: &crate::ui::events::SkimEvent,
) -> Option<crate::ui::events::SkimMessage> {
    // Status is display-only, no events to handle
    None
}

/// Render the status component
pub fn render_status(state: &StatusState, frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
    let mut status_state = state.clone();
    Status.render(area, frame.buffer_mut(), &mut status_state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_state_creation() {
        let state = StatusState::new();
        assert_eq!(state.total, 0);
        assert_eq!(state.matched, 0);
        assert_eq!(state.selected, 0);
        assert!(!state.matcher_running);
        assert!(!state.reading);
    }

    #[test]
    fn test_status_update() {
        let mut state = StatusState::new();
        state.update_status(100, 50, 75, 5, 10);

        assert_eq!(state.total, 100);
        assert_eq!(state.matched, 50);
        assert_eq!(state.processed, 75);
        assert_eq!(state.selected, 5);
        assert_eq!(state.current_item_idx, 10);
    }

    #[test]
    fn test_spinner_char() {
        let mut state = StatusState::new();
        
        // Not reading, should show default
        let char = state.get_spinner_char();
        assert_eq!(char, ' ');

        // Reading but not long enough, should show default
        state.set_reading(true, Duration::from_millis(10));
        let char = state.get_spinner_char();
        assert_eq!(char, ' ');

        // Reading long enough, should show spinner
        state.set_reading(true, Duration::from_millis(100));
        let char = state.get_spinner_char();
        assert!(SPINNERS_UNICODE.contains(&char));
    }

    #[test]
    fn test_info_display_modes() {
        let mut state = StatusState::new();
        
        // Default mode
        state.info_display = InfoDisplay::Default;
        assert_eq!(state.get_spinner_char(), ' ');

        // Inline mode  
        state.info_display = InfoDisplay::Inline;
        assert_eq!(state.get_spinner_char(), '<');

        // Hidden mode
        state.info_display = InfoDisplay::Hidden;
        assert_eq!(state.get_spinner_char(), ' ');
    }
}