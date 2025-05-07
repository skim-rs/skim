//! `Cell` is a cell of the terminal.
//! It has a display character and an attribute (fg and bg color, effects).
use std::sync::LazyLock;

use crossterm::style::{StyledContent, Stylize as _};

const EMPTY_CHAR: char = '\0';

pub type Cell = StyledContent<char>;

pub static BLANK_CELL: LazyLock<Cell> = LazyLock::new(|| BLANK_CELL.stylize());
pub static EMPTY_CELL: LazyLock<Cell> = LazyLock::new(|| EMPTY_CHAR.stylize());

//#[derive(Debug, Clone, Copy, PartialEq)]
//pub struct Cell {
//    pub ch: char,
//    pub style: ContentStyle,
//}
//
//impl Default for Cell {
//    fn default() -> Self {
//        Self {
//            ch: ' ',
//            style: ContentStyle::new(),
//        }
//    }
//}
//
//impl Cell {
//    pub fn empty() -> Self {
//        Self::default().ch(EMPTY_CHAR)
//    }
//
//    pub fn ch(mut self, ch: char) -> Self {
//        self.ch = ch;
//        self
//    }
//
//    pub fn fg(mut self, fg: Color) -> Self {
//        self.style.foreground_color = Some(fg);
//        self
//    }
//
//    pub fn bg(mut self, bg: Color) -> Self {
//        self.style.background_color = Some(bg);
//        self
//    }
//
//    pub fn attr(mut self, effect: Attributes) -> Self {
//        self.style.attributes = effect;
//        self
//    }
//
//    pub fn style(mut self, style: ContentStyle) -> Self {
//        self.style = style;
//        self
//    }
//
//    /// check if a cell is empty
//    pub fn is_empty(self) -> bool {
//        self.ch == EMPTY_CHAR && self.style == ContentStyle::new()
//    }
//}
//
//impl From<char> for Cell {
//    fn from(ch: char) -> Self {
//        Cell {
//            ch,
//            style: Default::default(),
//        }
//    }
//}
