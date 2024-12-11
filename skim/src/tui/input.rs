use std::{
    cmp::max,
    ops::{Deref, DerefMut},
};

use ratatui::{
    prelude::*,
    widgets::{Block, Paragraph, Widget},
};

pub struct Input {
    pub prompt: String,
    pub value: String,
    pub cursor_pos: u16,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            prompt: String::from(">"),
            value: String::default(),
            cursor_pos: 0,
        }
    }
}

impl Input {
    pub fn insert(&mut self, c: char) {
        self.value.insert(self.cursor_pos.into(), c);
        self.move_cursor(1);
    }
    pub fn delete(&mut self, offset: i32) {
        self.delete_at(self.cursor_pos() as i32 + offset)
    }
    pub fn delete_at(&mut self, pos: i32) {
        if self.value.is_empty() || pos < 0 {
            return;
        }
        let clamped = usize::clamp(pos as usize, 0, max(self.value.len(), 1) - 1);
        self.value.remove(clamped);
        self.move_cursor(-1);
    }
    pub fn move_cursor(&mut self, offset: i32) {
        self.move_cursor_to((self.cursor_pos as i32 + offset) as u16)
    }
    pub fn move_cursor_to(&mut self, pos: u16) {
        self.cursor_pos = u16::clamp(pos, 0, self.value.len() as u16);
    }
    pub fn delete_word(&mut self) {
        self.delete(-1); // Remove first non-alphanumeric char if there is one
        while !self.value.is_empty() {
            let prev_char = self.value.remove((self.cursor_pos - 1) as usize);
            if prev_char.is_alphabetic() {
                self.move_cursor(-1);
            } else {
                self.value.push(prev_char);
                break;
            }
        }
    }
    pub fn cursor_pos(&self) -> u16 {
        self.cursor_pos + self.prompt.len() as u16 + 1
    }
}

impl Widget for &Input {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        Paragraph::new(format!("{} {}", &self.prompt, &self.value))
            .block(Block::default())
            .render(area, buf);
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
