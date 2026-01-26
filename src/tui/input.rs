use std::ops::{Deref, DerefMut};
use std::time::Instant;

use ratatui::{prelude::*, widgets::Widget};
use unicode_display_width::width as display_width;

use crate::tui::BorderType;
use crate::tui::options::TuiLayout;
use crate::tui::statusline::InfoDisplay;
use crate::tui::widget::{SkimRender, SkimWidget};
use crate::{SkimOptions, theme::ColorTheme};
use std::sync::Arc;

const SPINNER_DURATION: u32 = 200;
const SPINNERS_UNICODE: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Status information to display in the input widget's title
#[derive(Clone, Default)]
pub struct StatusInfo {
    /// Total number of items
    pub total: usize,
    /// Number of matched items
    pub matched: usize,
    /// Number of processed items
    pub processed: usize,
    /// Whether the spinner should be shown (controlled by App with debouncing)
    pub show_spinner: bool,
    /// Current matcher mode (e.g., "RE" for regex)
    pub matcher_mode: String,
    /// Whether multi-selection mode is enabled
    pub multi_selection: bool,
    /// Number of selected items
    pub selected: usize,
    /// Index of the current item
    pub current_item_idx: usize,
    /// Horizontal scroll offset
    pub hscroll_offset: i64,
    /// Start time for calculating spinner animation
    pub start: Option<Instant>,
}

impl StatusInfo {
    /// Build the left-aligned title string (spinner, matched/total, mode, progress, selection)
    /// Used for Default info display mode (separate line)
    pub fn left_title(&self) -> String {
        let mut parts = String::new();

        // Spinner
        if self.show_spinner
            && let Some(start) = self.start
        {
            let spinner_elapsed_ms = start.elapsed().as_millis();
            let index = ((spinner_elapsed_ms / (SPINNER_DURATION as u128)) % (SPINNERS_UNICODE.len() as u128)) as usize;
            parts.push(SPINNERS_UNICODE[index]);
            parts.push(' ');
        } else {
            parts.push_str("  ");
        }

        // Matched/total
        parts.push_str(&format!("{}/{}", self.matched, self.total));

        // Matcher mode
        if !self.matcher_mode.is_empty() {
            parts.push_str(&format!("/{}", &self.matcher_mode));
        }

        // Progress percentage
        if self.show_spinner && self.total > 0 && self.processed != self.total {
            let pct = self.processed.saturating_mul(100) / self.total;
            parts.push_str(&format!(" ({}%)", pct));
        }

        // Selection count
        if self.multi_selection && self.selected > 0 {
            parts.push_str(&format!(" [{}]", self.selected));
        }

        parts
    }

    /// Get the inline separator character: spinner when active, '<' otherwise
    /// Used for Inline info display mode
    pub fn inline_separator(&self) -> char {
        if self.show_spinner
            && let Some(start) = self.start
        {
            let spinner_elapsed_ms = start.elapsed().as_millis();
            let index = ((spinner_elapsed_ms / (SPINNER_DURATION as u128)) % (SPINNERS_UNICODE.len() as u128)) as usize;
            SPINNERS_UNICODE[index]
        } else {
            '<'
        }
    }

    /// Build the inline status string (matched/total, mode, progress, selection)
    /// Used for Inline info display mode - does NOT include spinner prefix
    pub fn inline_status(&self) -> String {
        let mut parts = String::new();

        // Matched/total
        parts.push_str(&format!("{}/{}", self.matched, self.total));

        // Matcher mode
        if !self.matcher_mode.is_empty() {
            parts.push_str(&format!("/{}", &self.matcher_mode));
        }

        // Progress percentage
        if self.show_spinner && self.total > 0 {
            let pct = self.processed.saturating_mul(100) / self.total;
            parts.push_str(&format!(" ({}%)", pct));
        }

        // Selection count
        if self.multi_selection && self.selected > 0 {
            parts.push_str(&format!(" [{}]", self.selected));
        }

        parts
    }

    /// Build the right-aligned title string (current index / hscroll)
    pub fn right_title(&self) -> String {
        format!("{}/{}", self.current_item_idx, self.hscroll_offset)
    }
}

pub struct Input {
    pub prompt: String,
    /// see alternate_value
    alternate_prompt: String,
    pub value: String,
    /// cmd query when in normal mode, query when in interactive mode
    alternate_value: String,
    pub cursor_pos: u16,
    pub alternate_cursor_pos: u16,
    pub theme: Arc<ColorTheme>,
    /// Border type, if borders are enabled
    pub border: Option<BorderType>,
    /// Status information to display as the input's title
    pub status_info: Option<StatusInfo>,
    /// How to display the info/status (default, inline, or hidden)
    pub info_display: InfoDisplay,
    /// Whether layout is reversed (status goes below input instead of above)
    pub reverse: bool,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            prompt: String::from(">"),
            alternate_prompt: String::from("c>"),
            value: String::default(),
            alternate_value: String::default(),
            cursor_pos: 0,
            alternate_cursor_pos: 0,
            theme: Arc::new(ColorTheme::default()),
            border: None,
            status_info: None,
            info_display: InfoDisplay::Default,
            reverse: false,
        }
    }
}

impl Input {
    pub fn insert(&mut self, c: char) {
        self.value.insert(self.cursor_pos.into(), c);
        // unwrap: len_utf8 < 4
        self.move_cursor(c.len_utf8().try_into().unwrap());
    }
    pub fn insert_str(&mut self, s: &str) {
        self.value.insert_str(self.cursor_pos as usize, s);
        self.move_cursor(
            s.chars()
                .count()
                .try_into()
                .expect("Failed to fit inserted str len into an i32"),
        );
    }
    fn nchars(&self) -> usize {
        self.value.chars().count()
    }
    pub fn delete(&mut self, offset: i32) -> Option<char> {
        if self.value.is_empty() {
            return None;
        }
        let new_pos = self.cursor_pos as i32 + offset;
        if new_pos < 0 || new_pos as usize >= self.value.len() {
            return None;
        }
        let pos = self.value.floor_char_boundary(new_pos as usize);
        let ch = self.value.remove(pos);
        // Only move cursor if deleting backwards
        if offset < 0 {
            self.move_cursor(-1);
        }
        Some(ch)
    }
    pub fn move_cursor(&mut self, offset: i32) {
        if offset == 0 {
            return;
        }
        if offset < 0 {
            self.move_cursor_to(
                self.value
                    .floor_char_boundary((self.cursor_pos as i32 + offset) as usize) as u16,
            );
        } else {
            self.move_cursor_to(
                self.value
                    .ceil_char_boundary((self.cursor_pos as i32 + offset) as usize) as u16,
            );
        }
    }
    pub fn move_cursor_to(&mut self, pos: u16) {
        if self.value.is_char_boundary(pos as usize) {
            self.cursor_pos = u16::clamp(pos, 0, self.value.len() as u16);
        } else {
            warn!("Invalid cursor pos");
        }
    }
    pub fn move_to_end(&mut self) {
        self.move_cursor_to(
            self.value
                .len()
                .try_into()
                .expect("Failed to fit input len into an u16"),
        )
    }

    /// Check if a character is a word character (alphanumeric only)
    fn is_word_char(ch: char) -> bool {
        ch.is_alphanumeric()
    }

    /// Find the position of the end of the next word (alphanumeric boundaries for deletion)
    fn find_next_word_end(&self, start_pos: usize) -> usize {
        let mut pos = start_pos;

        // Skip any non-word characters
        while pos < self.nchars() {
            let ch = self.value.chars().nth(pos).unwrap();
            if Self::is_word_char(ch) {
                break;
            }
            pos += 1;
        }

        // Skip to the end of the word
        while pos < self.nchars() {
            let ch = self.value.chars().nth(pos).unwrap();
            if !Self::is_word_char(ch) {
                break;
            }
            pos += 1;
        }

        pos
    }

    /// Find the end of compound word (whitespace boundaries for cursor movement)
    fn find_compound_word_end(&self, start_pos: usize) -> usize {
        let mut pos = start_pos;

        // Skip any whitespace
        while pos < self.nchars() {
            let ch = self.value.chars().nth(pos).unwrap();
            if !ch.is_whitespace() {
                break;
            }
            pos += 1;
        }

        // Skip to the end of the non-whitespace sequence (includes punctuation)
        while pos < self.nchars() {
            let ch = self.value.chars().nth(pos).unwrap();
            if ch.is_whitespace() {
                break;
            }
            pos += 1;
        }

        pos
    }

    /// Find the position of the start of the previous word (alphanumeric word boundaries)
    fn find_prev_word_start(&self, start_pos: usize) -> usize {
        if start_pos == 0 {
            return 0;
        }

        let mut pos = start_pos;

        // Move back at least one position
        pos = pos.saturating_sub(1);

        // Skip any non-word characters
        while pos > 0 && !Self::is_word_char(self.value.chars().nth(pos).unwrap()) {
            pos -= 1;
        }

        // Skip to the beginning of the word
        while pos > 0 && Self::is_word_char(self.value.chars().nth(pos - 1).unwrap()) {
            pos -= 1;
        }

        pos
    }

    /// Find the position to delete backward to (stops at non-word characters)
    fn find_delete_backward_pos(&self, start_pos: usize) -> usize {
        if start_pos == 0 {
            return 0;
        }

        let mut pos = start_pos;

        // Skip any non-word characters (whitespace, punctuation, etc.)
        while pos > 0 {
            let ch = self.value.chars().nth(pos - 1).unwrap();
            if Self::is_word_char(ch) {
                break;
            }
            pos -= 1;
        }

        // Skip back through word characters
        while pos > 0 {
            let ch = self.value.chars().nth(pos - 1).unwrap();
            if !Self::is_word_char(ch) {
                break;
            }
            pos -= 1;
        }

        pos
    }

    pub fn delete_backward_word(&mut self) -> String {
        if self.cursor_pos == 0 {
            return String::new();
        }
        // Delete back by alphanumeric word boundaries (for Alt+Backspace)
        let start_pos = self.find_delete_backward_pos(self.cursor_pos as usize);
        let deleted = self.value[start_pos..self.cursor_pos as usize].to_string();
        self.value = format!(
            "{}{}",
            &self.value[..start_pos],
            &self.value[self.cursor_pos as usize..]
        );
        self.cursor_pos = start_pos as u16;
        deleted
    }

    pub fn delete_backward_to_whitespace(&mut self) -> String {
        if self.cursor_pos == 0 {
            return String::new();
        }
        // Unix word rubout: delete back to whitespace (for Ctrl+W)
        let mut pos = self.cursor_pos as usize;

        // Skip any trailing whitespace
        while pos > 0 && self.value.chars().nth(pos - 1).unwrap_or_default().is_whitespace() {
            pos -= 1;
        }

        // Delete back to next whitespace or start
        while pos > 0 && !self.value.chars().nth(pos - 1).unwrap_or_default().is_whitespace() {
            pos -= 1;
        }

        let deleted = self.value[pos..self.cursor_pos as usize].to_string();
        self.value = format!("{}{}", &self.value[..pos], &self.value[self.cursor_pos as usize..]);
        self.cursor_pos = pos as u16;
        deleted
    }

    pub fn delete_forward_word(&mut self) -> String {
        if self.cursor_pos as usize >= self.value.len() {
            return String::new();
        }
        let end_pos = self.find_next_word_end(self.cursor_pos as usize);
        let deleted = self.value[self.cursor_pos as usize..end_pos].to_string();
        self.value = format!("{}{}", &self.value[..self.cursor_pos as usize], &self.value[end_pos..]);
        deleted
    }
    pub fn move_cursor_forward_word(&mut self) {
        let new_pos = self.find_compound_word_end(self.cursor_pos as usize);
        self.cursor_pos = new_pos as u16;
    }

    pub fn move_cursor_backward_word(&mut self) {
        let new_pos = self.find_prev_word_start(self.cursor_pos as usize);
        self.cursor_pos = new_pos as u16;
    }
    pub fn delete_to_beginning(&mut self) -> String {
        let deleted = self.value[..self.cursor_pos as usize].to_string();
        self.value = self.value[self.cursor_pos as usize..].to_string();
        self.cursor_pos = 0;
        deleted
    }
    pub fn cursor_pos(&self) -> u16 {
        (display_width(&self.value[..(self.cursor_pos as usize)]) + display_width(&self.prompt))
            .try_into()
            .expect("Failed to fit cursor char into an u16")
    }
    pub fn switch_mode(&mut self) {
        std::mem::swap(&mut self.prompt, &mut self.alternate_prompt);
        std::mem::swap(&mut self.value, &mut self.alternate_value);
        std::mem::swap(&mut self.cursor_pos, &mut self.alternate_cursor_pos);
    }
}

impl SkimWidget for Input {
    fn from_options(options: &SkimOptions, theme: Arc<ColorTheme>) -> Self {
        let mut res = Self {
            theme,
            border: options.border,
            info_display: options.info.clone(),
            reverse: options.layout == TuiLayout::Reverse,
            ..Default::default()
        };
        if options.interactive {
            res.prompt = options.cmd_prompt.clone();
            res.alternate_prompt = options.prompt.clone();
            res.value = options.cmd_query.clone().unwrap_or_default();
            res.alternate_value = options.query.clone().unwrap_or_default();
        } else {
            res.prompt = options.prompt.clone();
            res.alternate_prompt = options.cmd_prompt.clone();
            res.value = options.query.clone().unwrap_or_default();
            res.alternate_value = options.cmd_query.clone().unwrap_or_default();
        }
        res.cursor_pos = res.value.len() as u16;
        res.alternate_cursor_pos = res.alternate_value.len() as u16;
        res
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) -> SkimRender {
        use ratatui::layout::Alignment;
        use ratatui::text::{Line, Span};
        use ratatui::widgets::Paragraph;
        use ratatui::widgets::{Block, Borders};

        let prompt_span = Span::styled(&self.prompt, self.theme.prompt);
        let value_span = Span::styled(&self.value, self.theme.query);

        let mut block = Block::default();

        // Add borders if enabled
        if let Some(border_type) = self.border {
            block = block
                .borders(Borders::ALL)
                .border_type(border_type.into())
                .border_style(self.theme.border);
        }

        // Handle different info display modes
        match self.info_display {
            InfoDisplay::Inline => {
                // Inline mode: render status on the same line as input
                // Format: prompt + value + " " + separator_char + " " + status + padding + right_status
                // separator_char is spinner when active, '<' otherwise
                if let Some(ref status) = self.status_info {
                    let separator = status.inline_separator();
                    let inline_status = status.inline_status();
                    let right_status = status.right_title();

                    // Calculate available width for padding
                    // Format: " X " where X is separator (3 chars total)
                    let prompt_width = display_width(&self.prompt);
                    let value_width = display_width(&self.value);
                    let separator_width = 4; // "  X " (2xspace + separator + space)
                    let inline_status_width = display_width(&inline_status);
                    let right_status_width = display_width(&right_status);

                    let used_width =
                        prompt_width + value_width + separator_width + inline_status_width + right_status_width;
                    let available_width = area.width as u64;
                    let padding_width = available_width.saturating_sub(used_width);

                    let line = Line::from(vec![
                        prompt_span,
                        value_span,
                        Span::styled(format!("  {} ", separator), self.theme.info),
                        Span::styled(inline_status, self.theme.info),
                        Span::raw(" ".repeat(padding_width.try_into().unwrap())),
                        Span::styled(right_status, self.theme.info),
                    ]);

                    Paragraph::new(line)
                        .block(block)
                        .style(self.theme.normal)
                        .render(area, buf);
                } else {
                    // No status info, just render input
                    let line = Line::from(vec![prompt_span, value_span]);
                    Paragraph::new(line)
                        .block(block)
                        .style(self.theme.normal)
                        .render(area, buf);
                }
            }
            InfoDisplay::Default => {
                // Default mode: render status as block title (separate line)
                // In normal layout: status above input (title_top)
                // In reverse layout: status below input (title_bottom)
                if let Some(ref status) = self.status_info {
                    let left_title = status.left_title();
                    let right_title = status.right_title();

                    if self.reverse {
                        block = block
                            .title_bottom(Line::from(left_title).style(self.theme.info).alignment(Alignment::Left))
                            .title_bottom(
                                Line::from(right_title)
                                    .style(self.theme.info)
                                    .alignment(Alignment::Right),
                            );
                    } else {
                        block = block
                            .title_top(Line::from(left_title).style(self.theme.info).alignment(Alignment::Left))
                            .title_top(
                                Line::from(right_title)
                                    .style(self.theme.info)
                                    .alignment(Alignment::Right),
                            );
                    }
                }

                let line = Line::from(vec![prompt_span, value_span]);
                Paragraph::new(line)
                    .block(block)
                    .style(self.theme.normal)
                    .render(area, buf);
            }
            InfoDisplay::Hidden => {
                // Hidden mode: no status displayed
                let line = Line::from(vec![prompt_span, value_span]);
                Paragraph::new(line)
                    .block(block)
                    .style(self.theme.normal)
                    .render(area, buf);
            }
        }

        SkimRender::default()
    }
}

impl Deref for Input {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl DerefMut for Input {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}
