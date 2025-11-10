use std::{
    cmp::max,
    ops::{Deref, DerefMut},
};

use ratatui::{
    prelude::*,
    widgets::{Block, Paragraph, Widget},
};

use crate::{SkimOptions, theme::ColorTheme};
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
    pub fn with_options(options: &SkimOptions, theme: Arc<ColorTheme>) -> Self {
        Self {
            prompt: options.prompt.clone(),
            value: options.query.clone().unwrap_or_default(),
            theme,
            border: options.border,
            cursor_pos: options.query.clone().map(|q| q.len() as u16).unwrap_or_default(),
            ..Default::default()
        }
    }
    pub fn insert(&mut self, c: char) {
        self.value.insert(self.cursor_pos.into(), c);
        self.move_cursor(1);
    }
    pub fn insert_str(&mut self, s: &str) {
        self.value.insert_str(self.cursor_pos as usize, s);
        self.move_cursor(s.len() as i32);
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
    
    /// Check if a character is a word character (alphanumeric only)
    fn is_word_char(ch: char) -> bool {
        ch.is_alphanumeric()
    }
    
    /// Find the position of the start of the next word going forward from current position
    fn find_next_word_end(&self, start_pos: usize) -> usize {
        let mut pos = start_pos;
        
        // If we're on a non-word char, skip to the next word
        if pos < self.value.len() {
            let ch = self.value.chars().nth(pos).unwrap();
            if !Self::is_word_char(ch) {
                while pos < self.value.len() {
                    let ch = self.value.chars().nth(pos).unwrap();
                    if Self::is_word_char(ch) {
                        break;
                    }
                    pos += 1;
                }
            }
        }
        
        // Now skip to the end of the word
        while pos < self.value.len() {
            let ch = self.value.chars().nth(pos).unwrap();
            if !Self::is_word_char(ch) {
                break;
            }
            pos += 1;
        }
        
        pos
    }
    
    /// Find the position of the start of the previous word going backward from current position
    fn find_prev_word_start(&self, start_pos: usize) -> usize {
        if start_pos == 0 {
            return 0;
        }
        
        let mut pos = start_pos - 1;
        
        // Skip any non-word characters
        while pos > 0 {
            let ch = self.value.chars().nth(pos).unwrap();
            if Self::is_word_char(ch) {
                break;
            }
            if pos == 0 {
                break;
            }
            pos -= 1;
        }
        
        // Skip to the beginning of the word
        while pos > 0 {
            let ch = self.value.chars().nth(pos - 1).unwrap();
            if !Self::is_word_char(ch) {
                break;
            }
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
        let start_pos = self.find_delete_backward_pos(self.cursor_pos as usize);
        let deleted = self.value[start_pos..self.cursor_pos as usize].to_string();
        self.value = format!("{}{}", 
            &self.value[..start_pos],
            &self.value[self.cursor_pos as usize..]
        );
        self.cursor_pos = start_pos as u16;
        deleted
    }
    
    pub fn delete_forward_word(&mut self) -> String {
        if self.cursor_pos as usize >= self.value.len() {
            return String::new();
        }
        let end_pos = self.find_next_word_end(self.cursor_pos as usize);
        let deleted = self.value[self.cursor_pos as usize..end_pos].to_string();
        self.value = format!("{}{}",
            &self.value[..self.cursor_pos as usize],
            &self.value[end_pos..]
        );
        deleted
    }
    pub fn move_cursor_forward_word(&mut self) {
        let new_pos = self.find_next_word_end(self.cursor_pos as usize);
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
