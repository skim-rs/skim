use std::{
    cmp::max,
    ops::{Deref, DerefMut},
};

use ratatui::{
    prelude::*,
    widgets::{Block, Paragraph, Widget},
};

use crate::theme::ColorTheme;
use std::sync::Arc;

pub struct Input {
    pub prompt: String,
    pub value: String,
    pub cursor_pos: u16,
    pub theme: Arc<ColorTheme>,
    pub border: bool,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            prompt: String::from(">"),
            value: String::default(),
            cursor_pos: 0,
            theme: Arc::new(ColorTheme::default()),
            border: false,
        }
    }
}

impl Input {
    pub fn insert(&mut self, c: char) {
        self.value.insert(self.cursor_pos.into(), c);
        self.move_cursor(1);
    }
    pub fn delete(&mut self, offset: i32) -> Option<char> {
        self.delete_at(self.cursor_pos() as i32 + offset)
    }
    pub fn delete_at(&mut self, pos: i32) -> Option<char> {
        if self.value.is_empty() || pos < 0 {
            return None;
        }
        let clamped = usize::clamp(pos as usize, 0, max(self.value.len(), 1) - 1);
        self.move_cursor(-1);
        Some(self.value.remove(clamped))
    }
    pub fn move_cursor(&mut self, offset: i32) {
        self.move_cursor_to((self.cursor_pos as i32 + offset) as u16)
    }
    pub fn move_cursor_to(&mut self, pos: u16) {
        self.cursor_pos = u16::clamp(pos, 0, self.value.len() as u16);
    }
    /// Delete a word. Direction is -1 for backwards, +1 for forward
    fn delete_word_dir(&mut self, direction: i32) -> String {
        let prev_char = self.delete(direction); // Remove first non-alphanumeric char if there is one
        let mut res = match prev_char {
            Some(c) => String::from(c),
            None => String::default(),
        };
        while !self.value.is_empty() {
            let prev_char = self.value.remove((self.cursor_pos as i32 + direction) as usize);
            if prev_char.is_alphabetic() {
                self.move_cursor(direction);
                res.push(prev_char);
            } else {
                self.value.push(prev_char);
                break;
            }
        }
        res
    }
    pub fn delete_backward_word(&mut self) -> String {
        self.delete_word_dir(-1)
    }
    pub fn delete_forward_word(&mut self) -> String {
        self.delete_word_dir(1)
    }
    pub fn cursor_pos(&self) -> u16 {
        self.cursor_pos + self.prompt.chars().count() as u16
    }
}

impl Widget for &Input {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        use ratatui::text::{Line, Span};
        use ratatui::widgets::Paragraph;

        let prompt_span = Span::styled(&self.prompt, self.theme.prompt());
        let value_span = Span::styled(&self.value, self.theme.query());
        let line = Line::from(vec![prompt_span, value_span]);
        use ratatui::widgets::{Block, Borders};
        let block = if self.border {
            Block::default().borders(Borders::ALL).border_style(self.theme.border())
        } else {
            Block::default()
        };
        Paragraph::new(line).block(block).render(area, buf);
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
