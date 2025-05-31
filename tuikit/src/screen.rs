//! Buffering screen cells and try to optimize rendering contents
use crate::cell::{Cell, EMPTY_CELL};
use crate::error::TuikitError;
use crate::Result;
use crate::{canvas::Canvas, cell::BLANK_CELL};
use crossterm::{
    cursor, queue,
    style::{self, Print, PrintStyledContent},
    terminal,
};
use std::cmp::{max, min};
use std::io::Write;
use unicode_width::UnicodeWidthChar;

// much of the code comes from https://github.com/agatan/termfest/blob/master/src/screen.rs

/// A Screen is a table of cells to draw on.
/// It's a buffer holding the contents
#[derive(Debug)]
pub struct Screen {
    width: u16,
    height: u16,
    cursor: Cursor,
    cells: Vec<Cell>,
    painted_cells: Vec<Cell>,
    painted_cursor: Cursor,
    clear_on_start: bool,
}

impl Screen {
    /// create an empty screen with size: (width, height)
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: vec![*BLANK_CELL; (width * height) as usize],
            cursor: Cursor::default(),
            painted_cells: vec![*BLANK_CELL; (width * height) as usize],
            painted_cursor: Cursor::default(),
            clear_on_start: false,
        }
    }

    pub fn clear_on_start(&mut self, clear_on_start: bool) {
        self.clear_on_start = clear_on_start;
    }

    /// get the width of the screen
    #[inline]
    pub fn width(&self) -> u16 {
        self.width
    }

    /// get the height of the screen
    #[inline]
    pub fn height(&self) -> u16 {
        self.height
    }

    #[inline]
    fn index(&self, row: u16, col: u16) -> Result<usize> {
        if row >= self.height || col >= self.width {
            Err(TuikitError::IndexOutOfBound(row, col))
        } else {
            Ok((row * self.width + col) as usize)
        }
    }

    fn empty_canvas(&self, width: u16, height: u16) -> Vec<Cell> {
        vec![*EMPTY_CELL; (width * height) as usize]
    }

    fn copy_cells(&self, original: &[Cell], width: u16, height: u16) -> Vec<Cell> {
        let mut new_cells = self.empty_canvas(width, height);
        use std::cmp;
        let min_height = cmp::min(height, self.height) as usize;
        let min_width = cmp::min(width, self.width) as usize;
        for row in 0..min_height {
            let orig_start = row * self.width as usize;
            let orig_end = min_width + orig_start;
            let start = row * width as usize;
            let end = min_width + start;
            new_cells[start..end].copy_from_slice(&original[orig_start..orig_end]);
        }
        new_cells
    }

    /// to resize the screen to `(width, height)`
    pub fn resize(&mut self, width: u16, height: u16) {
        self.cells = self.copy_cells(&self.cells, width, height);
        self.painted_cells = self.empty_canvas(width, height);
        self.width = width;
        self.height = height;

        self.cursor.row = min(self.cursor.row, height);
        self.cursor.col = min(self.cursor.col, width);
    }

    /// sync internal buffer with the terminal
    pub fn present<Output: Write>(&mut self, output: &mut Output) -> Result<()> {
        // hide cursor && reset ContentStyleibutes
        queue!(output, cursor::Hide, cursor::MoveTo(0, 0), style::ResetColor)?;

        let mut last_cursor = Cursor::default();

        for row in 0..self.height {
            // calculate the last col that has contents
            let mut empty_col_index = 0;
            for col in (0..self.width).rev() {
                let index = self.index(row, col).unwrap();
                let cell = &self.cells[index];
                if *cell.content() == '\0' {
                    self.painted_cells[index] = *cell;
                } else {
                    empty_col_index = col + 1;
                    break;
                }
            }

            // compare cells and print necessary escape codes
            let mut last_ch_is_wide = false;
            for col in 0..empty_col_index {
                let index = self.index(row, col).unwrap();

                // advance if the last character is wide
                if last_ch_is_wide {
                    last_ch_is_wide = false;
                    self.painted_cells[index] = self.cells[index];
                    continue;
                }

                let cell_to_paint = self.cells[index];
                let cell_painted = self.painted_cells[index];

                // no need to paint if the content did not change
                if cell_to_paint == cell_painted {
                    continue;
                }

                // move cursor if necessary
                if last_cursor.row != row || last_cursor.col != col {
                    queue!(output, cursor::MoveTo(col as u16, row as u16))?;
                }

                // correctly draw the characters
                match *cell_to_paint.content() {
                    '\n' | '\r' | '\t' | '\0' => {
                        queue!(output, Print(' '))?;
                    }
                    _ => {
                        queue!(output, PrintStyledContent(cell_to_paint))?;
                    }
                }

                let display_width = cell_to_paint.content().width().unwrap_or(2);

                // wide character
                if display_width == 2 {
                    last_ch_is_wide = true;
                }

                last_cursor.row = row;
                last_cursor.col = col + display_width as u16;
                self.painted_cells[index] = cell_to_paint;
            }

            if empty_col_index != self.width {
                queue!(
                    output,
                    cursor::MoveTo(empty_col_index as u16, row as u16),
                    style::ResetColor,
                    style::SetAttribute(style::Attribute::Reset)
                )?;
                if self.clear_on_start {
                    queue!(output, terminal::Clear(terminal::ClearType::UntilNewLine))?;
                }
            }
        }

        // restore cursor
        queue!(output, cursor::MoveTo(self.cursor.col as u16, self.cursor.row as u16))?;
        if self.cursor.visible {
            queue!(output, cursor::Show)?;
        }

        self.painted_cursor = self.cursor;

        Ok(())
    }

    /// ```
    /// use tuikit::cell::Cell;
    /// use tuikit::canvas::Canvas;
    /// use tuikit::screen::Screen;
    ///
    ///
    /// let mut screen = Screen::new(1, 1);
    /// screen.put_cell(0, 0, Cell{ ch: 'a', ..Cell::default()});
    /// let mut iter = screen.iter_cell();
    /// assert_eq!(Some((0, 0, &Cell{ ch: 'a', ..Cell::default()})), iter.next());
    /// assert_eq!(None, iter.next());
    /// ```
    pub fn iter_cell(&self) -> CellIterator {
        CellIterator {
            width: self.width,
            index: 0,
            vec: &self.cells,
        }
    }
}

impl Canvas for Screen {
    /// Get the canvas size (width, height)
    fn size(&self) -> Result<(u16, u16)> {
        Ok((self.width(), self.height()))
    }

    /// clear the screen buffer
    fn clear(&mut self) -> Result<()> {
        for cell in self.cells.iter_mut() {
            *cell = *EMPTY_CELL;
        }
        Ok(())
    }

    /// change a cell of position `(row, col)` to `cell`
    fn put_cell(&mut self, row: u16, col: u16, cell: Cell) -> Result<usize> {
        let ch_width = cell.content().width().unwrap_or(2);
        if ch_width > 1 {
            let _ = self.index(row, col + 1).map(|index| {
                self.cells[index - 1] = cell;
                self.cells[index] = *BLANK_CELL;
            });
        } else {
            let _ = self.index(row, col).map(|index| {
                self.cells[index] = cell;
            });
        }
        Ok(ch_width)
    }

    /// move cursor position (row, col) and show cursor
    fn set_cursor(&mut self, row: u16, col: u16) -> Result<()> {
        self.cursor.row = min(row, max(self.height, 1) - 1);
        self.cursor.col = min(col, max(self.width, 1) - 1);
        self.cursor.visible = true;
        Ok(())
    }

    /// show/hide cursor, set `show` to `false` to hide the cursor
    fn show_cursor(&mut self, show: bool) -> Result<()> {
        self.cursor.visible = show;
        Ok(())
    }
}

pub struct CellIterator<'a> {
    width: u16,
    index: usize,
    vec: &'a Vec<Cell>,
}

impl<'a> Iterator for CellIterator<'a> {
    type Item = (u16, usize, &'a Cell);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.vec.len() {
            return None;
        }

        let (row, col) = (
            (self.index / self.width as usize) as u16,
            self.index % self.width as usize,
        );
        let ret = self.vec.get(self.index).map(|cell| (row, col, cell));
        self.index += 1;
        ret
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Cursor {
    pub row: u16,
    pub col: u16,
    visible: bool,
}

#[cfg(test)]
mod test {
    use crossterm::style::Stylize;

    use super::*;

    #[test]
    fn test_cell_iterator() {
        let mut screen = Screen::new(2, 2);
        let _ = screen.put_cell(0, 0, 'a'.stylize());
        let _ = screen.put_cell(0, 1, 'b'.stylize());
        let _ = screen.put_cell(1, 0, 'c'.stylize());
        let _ = screen.put_cell(1, 1, 'd'.stylize());

        let mut iter = screen.iter_cell();
        assert_eq!(Some((0, 0, &'a'.stylize())), iter.next());
        assert_eq!(Some((0, 1, &'b'.stylize())), iter.next());
        assert_eq!(Some((1, 0, &'c'.stylize())), iter.next());
        assert_eq!(Some((1, 1, &'c'.stylize())), iter.next());
        assert_eq!(None, iter.next());

        let empty_screen = Screen::new(0, 0);
        let mut empty_iter = empty_screen.iter_cell();
        assert_eq!(None, empty_iter.next());
    }
}
