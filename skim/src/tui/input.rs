use std::ops::{Deref, DerefMut};

use ratatui::{
    prelude::*,
    widgets::{Block, Paragraph, Widget},
};

#[derive(Default)]
pub struct Input {
    pub value: String,
    pub cursor_pos: u16,
}
impl Input {
    pub fn insert(&mut self, c: char) {
        self.value.insert(self.cursor_pos.into(), c);
        self.move_cursor(1);
    }
    pub fn delete(&mut self) {
        if self.value.is_empty() {
            return;
        }
        self.move_cursor(-1);
        self.value.remove(self.cursor_pos.into());
    }
    pub fn move_cursor(&mut self, offset: i32) {
        self.cursor_pos = i32::clamp(self.cursor_pos as i32 + offset, 0, self.value.len() as i32)
            .try_into()
            .unwrap_or_default(); // TODO better overflow handling, even though it should never
                                  // happen until the input value gets longer than 2^(16-1)
    }
    pub fn delete_word(&mut self) {
        self.delete(); // Remove first non-alphanumeric char if there is one
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
}

impl Widget for &Input {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        Paragraph::new(self.value.as_str())
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
