use std::ops::{Deref, DerefMut};

use ratatui::{prelude::*, widgets::Widget};

use crate::tui::widget::{SkimRender, SkimWidget};
use crate::{SkimOptions, theme::ColorTheme};
use std::sync::Arc;

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
    pub border: bool,
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
            border: false,
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
        (self.value[..(self.cursor_pos as usize)].chars().count() + self.prompt.chars().count())
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
        use ratatui::text::{Line, Span};
        use ratatui::widgets::Paragraph;

        let prompt_span = Span::styled(&self.prompt, self.theme.prompt);
        let value_span = Span::styled(&self.value, self.theme.query);
        let line = Line::from(vec![prompt_span, value_span]);
        use ratatui::widgets::{Block, Borders};
        let block = if self.border {
            Block::default().borders(Borders::ALL).border_style(self.theme.border)
        } else {
            Block::default()
        };
        Paragraph::new(line)
            .block(block)
            .style(self.theme.normal)
            .render(area, buf);

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
