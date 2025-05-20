//! A canvas is a trait defining the draw actions
use crate::cell::{Cell, EMPTY_CELL};
use crate::Result;
use crossterm::style::ContentStyle;
use unicode_width::UnicodeWidthChar;

pub trait Canvas {
    /// Get the canvas size (width, height)
    fn size(&self) -> Result<(u16, u16)>;

    /// clear the canvas
    fn clear(&mut self) -> Result<()>;

    /// change a cell of position `(row, col)` to `cell`
    /// if `(row, col)` is out of boundary, `Ok` is returned, but no operation is taken
    /// return the width of the character/cell
    fn put_cell(&mut self, row: u16, col: u16, cell: Cell) -> Result<usize>;

    /// just like put_cell, except it accept (char & style)
    /// return the width of the character/cell
    fn put_char_with_style(&mut self, row: u16, col: u16, ch: char, style: ContentStyle) -> Result<usize> {
        self.put_cell(row, col, Cell::new(style, ch))
    }

    /// print `content` starting with position `(row, col)` with `style`
    ///
    /// - canvas should NOT wrap to y+1 if the content is too long
    /// - canvas should handle wide characters
    ///
    /// returns the printed width of the content
    fn print_with_style(&mut self, row: u16, col: u16, content: &str, style: ContentStyle) -> Result<usize> {
        let mut width = 0;
        for ch in content.chars() {
            width += self.put_cell(row, col + width as u16, Cell::new(style, ch))?;
        }
        Ok(width)
    }

    /// print `content` starting with position `(row, col)` with default style
    fn print(&mut self, row: u16, col: u16, content: &str) -> Result<usize> {
        self.print_with_style(row, col, content, ContentStyle::default())
    }

    /// move cursor position (row, col) and show cursor
    fn set_cursor(&mut self, row: u16, col: u16) -> Result<()>;

    /// show/hide cursor, set `show` to `false` to hide the cursor
    fn show_cursor(&mut self, show: bool) -> Result<()>;
}

/// A sub-area of a canvas.
/// It will handle the adjustments of cursor movement, so that you could write
/// to for example (0, 0) and BoundedCanvas will adjust it to real position.
pub struct BoundedCanvas<'a> {
    canvas: &'a mut dyn Canvas,
    top: u16,
    left: u16,
    width: u16,
    height: u16,
}

impl<'a> BoundedCanvas<'a> {
    pub fn new(top: u16, left: u16, width: u16, height: u16, canvas: &'a mut dyn Canvas) -> Self {
        Self {
            canvas,
            top,
            left,
            width,
            height,
        }
    }
}

impl Canvas for BoundedCanvas<'_> {
    fn size(&self) -> Result<(u16, u16)> {
        Ok((self.width, self.height))
    }

    fn clear(&mut self) -> Result<()> {
        for row in self.top..(self.top + self.height) {
            for col in self.left..(self.left + self.width) {
                let _ = self.canvas.put_cell(row, col, *EMPTY_CELL);
            }
        }

        Ok(())
    }

    fn put_cell(&mut self, row: u16, col: u16, cell: Cell) -> Result<usize> {
        if row >= self.height || col >= self.width {
            // do nothing
            Ok(cell.content().width().unwrap_or(2))
        } else {
            self.canvas.put_cell(row + self.top, col + self.left, cell)
        }
    }

    fn set_cursor(&mut self, row: u16, col: u16) -> Result<()> {
        if row >= self.height || col >= self.width {
            // do nothing
            Ok(())
        } else {
            self.canvas.set_cursor(row + self.top, col + self.left)
        }
    }

    fn show_cursor(&mut self, show: bool) -> Result<()> {
        self.canvas.show_cursor(show)
    }
}