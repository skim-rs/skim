//! Term is a thread-safe "terminal".
//!
//! It allows you to:
//! - Listen to key stroke events
//! - Output contents to the terminal
//!
//! ```no_run
//! use tuikit::prelude::*;
//!
//! let term = Term::<()>::new().unwrap();
//!
//! while let Ok(ev) = term.poll_event() {
//!     if let Event::Key(Key::Char('q')) = ev {
//!         break;
//!     }
//!
//!     term.print(0, 0, format!("got event: {:?}", ev).as_str());
//!     term.present();
//! }
//! ```
//!
//! Term is modeled after [termbox](https://github.com/nsf/termbox). The main idea is viewing
//! terminals as a table of fixed-size cells and input being a stream of structured messages

use std::cmp::{max, min};
use std::io::{Stdout, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossterm::cursor::MoveTo;
use crossterm::event::{
    self, DisableMouseCapture, EnableBracketedPaste, EnableFocusChange, EnableMouseCapture, Event, MouseEvent,
};
use crossterm::style::{ContentStyle, PrintStyledContent, StyledContent};
use crossterm::{cursor, execute, queue, terminal};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr as _};

use crate::canvas::Canvas;
use crate::cell::Cell;
use crate::draw::Draw;
use crate::error::TuikitError;
use crate::screen::Screen;
use crate::spinlock::SpinLock;
use crate::Result;

const MIN_HEIGHT: u16 = 1;
const WAIT_TIMEOUT: Duration = Duration::from_millis(300);
const POLLING_TIMEOUT: Duration = Duration::from_millis(10);

#[derive(Debug, Copy, Clone)]
pub enum TermHeight {
    Fixed(u16),
    Percent(u16),
}

pub struct Term {
    components_to_stop: Arc<AtomicUsize>,
    term_lock: SpinLock<TermLock<Stdout>>,
    raw_mouse: bool, // to produce raw mouse event or the parsed event(e.g. DoubleClick)
}

pub struct TermOptions {
    max_height: TermHeight,
    min_height: TermHeight,
    height: TermHeight,
    clear_on_exit: bool,
    clear_on_start: bool,
    mouse_enabled: bool,
    raw_mouse: bool,
    hold: bool, // to start term or not on creation
    disable_alternate_screen: bool,
}

impl Default for TermOptions {
    fn default() -> Self {
        Self {
            max_height: TermHeight::Percent(100),
            min_height: TermHeight::Fixed(3),
            height: TermHeight::Percent(100),
            clear_on_exit: true,
            clear_on_start: true,
            mouse_enabled: false,
            raw_mouse: false,
            hold: false,
            disable_alternate_screen: false,
        }
    }
}

// Builder
impl TermOptions {
    pub fn max_height(mut self, max_height: TermHeight) -> Self {
        self.max_height = max_height;
        self
    }

    pub fn min_height(mut self, min_height: TermHeight) -> Self {
        self.min_height = min_height;
        self
    }
    pub fn height(mut self, height: TermHeight) -> Self {
        self.height = height;
        self
    }
    pub fn clear_on_exit(mut self, clear: bool) -> Self {
        self.clear_on_exit = clear;
        self
    }
    pub fn clear_on_start(mut self, clear: bool) -> Self {
        self.clear_on_start = clear;
        self
    }
    pub fn mouse_enabled(mut self, enabled: bool) -> Self {
        self.mouse_enabled = enabled;
        self
    }
    pub fn raw_mouse(mut self, enabled: bool) -> Self {
        self.raw_mouse = enabled;
        self
    }
    pub fn hold(mut self, hold: bool) -> Self {
        self.hold = hold;
        self
    }
    pub fn disable_alternate_screen(mut self, disable_alternate_screen: bool) -> Self {
        self.disable_alternate_screen = disable_alternate_screen;
        self
    }
}

impl Term {
    /// Create a Term with height specified.
    ///
    /// Internally if the calculated height would fill the whole screen, `Alternate Screen` will
    /// be enabled, otherwise only part of the screen will be used.
    ///
    /// If the preferred height is larger than the current screen, whole screen is used.
    ///
    /// ```no_run
    /// use tuikit::term::{Term, TermHeight};
    ///
    /// let term: Term<()> = Term::with_height(TermHeight::Percent(30)).unwrap(); // 30% of the terminal height
    /// let term: Term<()> = Term::with_height(TermHeight::Fixed(20)).unwrap(); // fixed 20 lines
    /// ```
    pub fn with_height(height: TermHeight) -> Result<Term> {
        Term::with_options(TermOptions::default().height(height))
    }

    /// Create a Term (with 100% height)
    ///
    /// ```no_run
    /// use tuikit::term::{Term, TermHeight};
    ///
    /// let term: Term<()> = Term::new().unwrap();
    /// let term: Term<()> = Term::with_height(TermHeight::Percent(100)).unwrap();
    /// ```
    pub fn new() -> Result<Term> {
        Term::with_options(TermOptions::default())
    }

    /// Create a Term with custom options
    ///
    /// ```no_run
    /// use tuikit::term::{Term, TermHeight, TermOptions};
    ///
    /// let term: Term<()> = Term::with_options(TermOptions::default().height(TermHeight::Percent(100))).unwrap();
    /// ```
    pub fn with_options(options: TermOptions) -> Result<Term> {
        let raw_mouse = options.raw_mouse;
        let ret = Term {
            components_to_stop: Arc::new(AtomicUsize::new(0)),
            term_lock: SpinLock::new(TermLock::with_options(&options)),
            raw_mouse,
        };
        ret.enter()?;
        if options.hold {
            Ok(ret)
        } else {
            ret.restart().map(|_| ret)
        }
    }

    pub fn enter(&self) -> Result<()> {
        let mut lock = self.term_lock.lock();
        if let Some(out) = lock.output.as_mut() {
            terminal::enable_raw_mode()?;
            Ok(execute!(
                out,
                EnableBracketedPaste,
                EnableFocusChange,
                EnableMouseCapture,
            )?)
        } else {
            Err(TuikitError::TermLocked)
        }
    }

    fn ensure_not_stopped(&self) -> Result<()> {
        if self.components_to_stop.load(Ordering::SeqCst) == 2 {
            Ok(())
        } else {
            Err(TuikitError::TerminalNotStarted)
        }
    }

    /// Get the cursor position
    ///
    /// Note: in its current implementation, this is a wrapper around crossterm's cursor::position()
    fn get_cursor_pos(&self) -> Result<(u16, u16)> {
        Ok(cursor::position()?)
    }

    /// restart the terminal if it had been stopped
    pub fn restart(&self) -> Result<()> {
        let mut termlock = self.term_lock.lock();
        if self.components_to_stop.load(Ordering::SeqCst) == 2 {
            return Ok(());
        }

        termlock.restart()?;

        // wait for components to start
        while self.components_to_stop.load(Ordering::SeqCst) < 2 {
            debug!(
                "restart: components: {}",
                self.components_to_stop.load(Ordering::SeqCst)
            );
            thread::sleep(POLLING_TIMEOUT);
        }

        Ok(())
    }

    /// Pause the Term
    ///
    /// This function will cause the Term to give away the control to the terminal(such as listening
    /// to the key strokes). After the Term was "paused", `poll_event` will block indefinitely and
    /// recover after the Term was `restart`ed.
    pub fn pause(&self) -> Result<()> {
        self.pause_internal(false)
    }

    fn pause_internal(&self, exiting: bool) -> Result<()> {
        debug!("pause");
        let mut termlock = self.term_lock.lock();

        if self.components_to_stop.load(Ordering::SeqCst) == 0 {
            return Ok(());
        }

        // wait for the components to stop
        // i.e. key_listener & size_change_listener

        termlock.pause(exiting)?;

        // wait for the components to stop
        while self.components_to_stop.load(Ordering::SeqCst) > 0 {
            debug!("pause: components: {}", self.components_to_stop.load(Ordering::SeqCst));
            thread::sleep(POLLING_TIMEOUT);
        }

        Ok(())
    }

    fn filter_event(&self, event: Event) -> Option<Event> {
        match event {
            Event::Resize { .. } => {
                {
                    let mut termlock = self.term_lock.lock();
                    let _ = termlock.on_resize();
                }
                let (width, height) = self.term_size().unwrap_or((0, 0));
                Some(Event::Resize(width, height))
            }
            Event::Mouse(mut e) => {
                // adjust mouse event position
                let cursor_row = self.term_lock.lock().get_term_start_row() as u16;
                if e.row < cursor_row {
                    None
                } else {
                    e.row -= cursor_row;
                    Some(Event::Mouse(e))
                }
            }
            ev => Some(ev),
        }
    }

    /// Wait an event up to `timeout` and return it
    pub fn peek_event(&self, timeout: Duration) -> Result<Option<Event>> {
        event::poll(timeout)?
            .then(|| self.poll_event())
            .ok_or(TuikitError::Timeout(timeout))?
    }

    /// Wait for an event indefinitely and return it
    pub fn poll_event(&self) -> Result<Option<Event>> {
        let ev = event::read()?;
        Ok(self.filter_event(ev))
    }

    /// Sync internal buffer with terminal
    pub fn present(&self) -> Result<()> {
        self.ensure_not_stopped()?;
        let mut termlock = self.term_lock.lock();
        termlock.present()
    }

    /// Return the printable size(width, height) of the term
    pub fn term_size(&self) -> Result<(u16, u16)> {
        self.ensure_not_stopped()?;
        let termlock = self.term_lock.lock();
        termlock.term_size()
    }

    /// Clear internal buffer
    pub fn clear(&self) -> Result<()> {
        self.ensure_not_stopped()?;
        let mut termlock = self.term_lock.lock();
        termlock.clear()
    }

    /// Change a cell of position `(row, col)` to `cell`
    pub fn put_cell(&self, row: u16, col: u16, cell: Cell) -> Result<usize> {
        self.ensure_not_stopped()?;
        let mut termlock = self.term_lock.lock();
        termlock.put_cell(row, col, cell)
    }

    /// Print `content` starting with position `(row, col)`
    pub fn print(&self, row: u16, col: u16, content: &str) -> Result<usize> {
        self.print_with_style(row, col, content, ContentStyle::default())
    }

    /// print `content` starting with position `(row, col)` with `style`
    pub fn print_with_style(&self, row: u16, col: u16, content: &str, style: impl Into<ContentStyle>) -> Result<usize> {
        self.ensure_not_stopped()?;
        let mut termlock = self.term_lock.lock();
        termlock.print_with_style(row, col, content, style)
    }

    /// Set cursor position to (row, col), and show the cursor
    pub fn set_cursor(&self, row: u16, col: u16) -> Result<()> {
        self.ensure_not_stopped()?;
        let mut termlock = self.term_lock.lock();
        termlock.set_cursor(row, col)
    }

    /// show/hide cursor, set `show` to `false` to hide the cursor
    pub fn show_cursor(&self, show: bool) -> Result<()> {
        self.ensure_not_stopped()?;
        let mut termlock = self.term_lock.lock();
        termlock.show_cursor(show)
    }

    /// Enable mouse support
    pub fn enable_mouse_support(&self) -> Result<()> {
        self.ensure_not_stopped()?;
        let mut termlock = self.term_lock.lock();
        termlock.enable_mouse_support()
    }

    /// Disable mouse support
    pub fn disable_mouse_support(&self) -> Result<()> {
        self.ensure_not_stopped()?;
        let mut termlock = self.term_lock.lock();
        termlock.disable_mouse_support()
    }

    /// Whether to clear the terminal upon exiting. Defaults to true.
    pub fn clear_on_exit(&self, clear: bool) -> Result<()> {
        self.ensure_not_stopped()?;
        let mut termlock = self.term_lock.lock();
        termlock.clear_on_exit(clear);
        Ok(())
    }

    pub fn draw(&self, draw: &dyn Draw) -> Result<()> {
        let mut canvas = TermCanvas { term: self };
        draw.draw(&mut canvas).map_err(TuikitError::DrawError)
    }

    pub fn draw_mut(&self, draw: &mut dyn Draw) -> Result<()> {
        let mut canvas = TermCanvas { term: self };
        draw.draw_mut(&mut canvas).map_err(TuikitError::DrawError)
    }
}

impl Drop for Term {
    fn drop(&mut self) {
        let _ = self.pause_internal(true);
    }
}

pub struct TermCanvas<'a> {
    term: &'a Term,
}

impl Canvas for TermCanvas<'_> {
    fn size(&self) -> Result<(u16, u16)> {
        self.term.term_size()
    }

    fn clear(&mut self) -> Result<()> {
        self.term.clear()
    }

    fn put_cell(&mut self, row: u16, col: u16, cell: Cell) -> Result<usize> {
        self.term.put_cell(row, col, cell)
    }

    fn print_with_style(&mut self, row: u16, col: u16, content: &str, style: ContentStyle) -> Result<usize> {
        self.term.print_with_style(row, col, content, style)
    }

    fn set_cursor(&mut self, row: u16, col: u16) -> Result<()> {
        self.term.set_cursor(row, col)
    }

    fn show_cursor(&mut self, show: bool) -> Result<()> {
        self.term.show_cursor(show)
    }
}

struct TermLock<Output: Write> {
    prefer_height: TermHeight,
    max_height: TermHeight,
    min_height: TermHeight,
    // keep bottom intact when resize?
    bottom_intact: bool,
    clear_on_exit: bool,
    clear_on_start: bool,
    mouse_enabled: bool,
    alternate_screen: bool,
    disable_alternate_screen: bool,
    cursor_row: u16,
    screen_height: u16,
    screen_width: u16,
    screen: Screen,
    output: Option<Output>,
}

impl<Output: Write> Default for TermLock<Output> {
    fn default() -> Self {
        Self {
            prefer_height: TermHeight::Percent(100),
            max_height: TermHeight::Percent(100),
            min_height: TermHeight::Fixed(3),
            bottom_intact: false,
            alternate_screen: false,
            disable_alternate_screen: false,
            cursor_row: 0,
            screen_height: 0,
            screen_width: 0,
            screen: Screen::new(0, 0),
            output: None,
            clear_on_exit: true,
            clear_on_start: true,
            mouse_enabled: false,
        }
    }
}

impl<Output: Write> TermLock<Output> {
    pub fn with_options(options: &TermOptions) -> Self {
        let mut term = TermLock::default();
        term.prefer_height = options.height;
        term.max_height = options.max_height;
        term.min_height = options.min_height;
        term.clear_on_exit = options.clear_on_exit;
        term.clear_on_start = options.clear_on_start;
        term.screen.clear_on_start(options.clear_on_start);
        term.disable_alternate_screen = options.disable_alternate_screen;
        term.mouse_enabled = options.mouse_enabled;
        term
    }

    /// Present the content to the terminal
    pub fn present(&mut self) -> Result<()> {
        let output = self.output.as_mut().ok_or(TuikitError::TerminalNotStarted)?;
        self.screen.present(output)?;
        output.flush()?;
        Ok(())
    }

    /// Resize the internal buffer to according to new terminal size
    pub fn on_resize(&mut self) -> Result<()> {
        let output = self.output.as_mut().ok_or(TuikitError::TerminalNotStarted)?;
        let (screen_width, screen_height) = terminal::size()?;
        self.screen_height = screen_height;
        self.screen_width = screen_width;

        let width = screen_width;
        let height =
            Self::calc_preferred_height(&self.min_height, &self.max_height, &self.prefer_height, screen_height);

        // update the cursor position
        if self.cursor_row + height >= screen_height {
            self.bottom_intact = true;
        }

        if self.bottom_intact {
            self.cursor_row = screen_height - height;
        }

        // clear the screen
        queue!(output, cursor::MoveTo(0, self.cursor_row))?;
        if self.clear_on_start {
            queue!(output, terminal::Clear(terminal::ClearType::FromCursorDown))?;
        }
        output.flush()?;

        // clear the screen buffer
        self.screen.resize(width, height);
        Ok(())
    }

    fn calc_height(height_spec: &TermHeight, actual_height: u16) -> u16 {
        match *height_spec {
            TermHeight::Fixed(h) => h,
            TermHeight::Percent(p) => actual_height * min(p, 100) / 100,
        }
    }

    fn calc_preferred_height(
        min_height: &TermHeight,
        max_height: &TermHeight,
        prefer_height: &TermHeight,
        height: u16,
    ) -> u16 {
        let max_height = Self::calc_height(max_height, height);
        let min_height = Self::calc_height(min_height, height);
        let prefer_height = Self::calc_height(prefer_height, height);

        // ensure the calculated height is in range (MIN_HEIGHT, height)
        let max_height = max(min(max_height, height), MIN_HEIGHT);
        let min_height = max(min(min_height, height), MIN_HEIGHT);
        max(min(prefer_height, max_height), min_height)
    }

    /// Pause the terminal
    fn pause(&mut self, exiting: bool) -> Result<()> {
        if let Some(mut output) = self.output.take() {
            queue!(output, DisableMouseCapture, cursor::Show)?;
            if self.clear_on_exit || !exiting {
                // clear drawn contents
                if !self.disable_alternate_screen {
                    queue!(output, terminal::LeaveAlternateScreen)?;
                } else {
                    queue!(
                        output,
                        cursor::MoveTo(0, self.cursor_row),
                        terminal::Clear(terminal::ClearType::FromCursorDown)
                    )?;
                }
            } else {
                queue!(output, cursor::MoveTo(0, self.cursor_row + self.screen_height))?;
                if self.bottom_intact {
                    output.write(b"\n");
                }
            }
            output.flush()?;
        }
        Ok(())
    }

    /// ensure the screen had enough height
    /// If the prefer height is full screen, it will enter alternate screen
    /// otherwise it will ensure there are enough lines at the bottom
    fn ensure_height(&mut self, cursor_pos: (u16, u16)) -> Result<()> {
        let output = self.output.as_mut().ok_or(TuikitError::TerminalNotStarted)?;

        // initialize

        let (screen_width, screen_height) = terminal::size()?;
        let height_to_be =
            Self::calc_preferred_height(&self.min_height, &self.max_height, &self.prefer_height, screen_height);

        self.alternate_screen = false;
        let (mut cursor_row, cursor_col) = cursor_pos;
        if height_to_be >= screen_height {
            // whole screen
            self.alternate_screen = true;
            self.bottom_intact = false;
            self.cursor_row = 0;
            if !self.disable_alternate_screen {
                queue!(output, terminal::EnterAlternateScreen)?;
            }
        } else {
            // only use part of the screen

            // go to a new line so that existing line won't be messed up
            if cursor_col > 0 {
                output.write(b"\n");
                cursor_row += 1;
            }

            if (cursor_row + height_to_be) <= screen_height {
                self.bottom_intact = false;
                self.cursor_row = cursor_row;
            } else {
                for _ in 0..(height_to_be - 1) {
                    output.write(b"\n");
                }
                self.bottom_intact = true;
                self.cursor_row = min(cursor_row, screen_height - height_to_be);
            }
        }

        queue!(output, MoveTo(0, self.cursor_row))?;
        output.flush()?;
        self.screen_height = screen_height;
        self.screen_width = screen_width;
        Ok(())
    }

    /// get the start row of the terminal
    pub fn get_term_start_row(&self) -> u16 {
        self.cursor_row
    }

    /// restart the terminal
    pub fn restart(&mut self) -> Result<()> {
        let cursor_pos = cursor::position()?;
        // ensure the output area had enough height
        self.ensure_height(cursor_pos)?;
        self.on_resize()?;
        if self.mouse_enabled {
            self.enable_mouse()?;
        }
        Ok(())
    }

    /// return the printable size(width, height) of the term
    pub fn term_size(&self) -> Result<(u16, u16)> {
        self.screen.size()
    }

    /// clear internal buffer
    pub fn clear(&mut self) -> Result<()> {
        let output = self.output.as_mut().ok_or(TuikitError::TerminalNotStarted)?;
        execute!(output, terminal::Clear(terminal::ClearType::All))?;
        Ok(())
    }

    /// change a cell of position `(row, col)` to `cell`
    pub fn put_cell(&mut self, row: u16, col: u16, cell: Cell) -> Result<usize> {
        let pos = cursor::position()?;
        let output = self.output.as_mut().ok_or(TuikitError::TerminalNotStarted)?;
        execute!(
            output,
            cursor::MoveTo(col, row),
            PrintStyledContent(cell),
            cursor::MoveTo(pos.0, pos.1)
        )?;
        Ok(cell.content().width().unwrap_or(0))
    }

    /// print `content` starting with position `(row, col)`
    pub fn print_with_style(
        &mut self,
        row: u16,
        col: u16,
        content: &str,
        style: impl Into<ContentStyle>,
    ) -> Result<usize> {
        let pos = cursor::position()?;
        let output = self.output.as_mut().ok_or(TuikitError::TerminalNotStarted)?;
        execute!(
            output,
            cursor::MoveTo(col, row),
            PrintStyledContent(StyledContent::new(style.into(), content)),
            cursor::MoveTo(pos.0, pos.1)
        )?;
        Ok(content.width())
    }

    /// set cursor position to (row, col)
    pub fn set_cursor(&mut self, row: u16, col: u16) -> Result<()> {
        let output = self.output.as_mut().ok_or(TuikitError::TerminalNotStarted)?;
        execute!(output, cursor::MoveTo(col, row))?;
        Ok(())
    }

    /// show/hide cursor, set `show` to `false` to hide the cursor
    pub fn show_cursor(&mut self, show: bool) -> Result<()> {
        let output = self.output.as_mut().ok_or(TuikitError::TerminalNotStarted)?;
        if show {
            execute!(output, cursor::Show)?;
        } else {
            execute!(output, cursor::Hide)?;
        }
        Ok(())
    }

    /// Enable mouse support
    pub fn enable_mouse_support(&mut self) -> Result<()> {
        self.mouse_enabled = true;
        self.enable_mouse()
    }

    /// Disable mouse support
    pub fn disable_mouse_support(&mut self) -> Result<()> {
        self.mouse_enabled = false;
        self.disable_mouse()
    }

    pub fn clear_on_exit(&mut self, clear: bool) {
        self.clear_on_exit = clear;
    }

    /// Enable mouse (send ANSI codes to enable mouse)
    fn enable_mouse(&mut self) -> Result<()> {
        let output = self.output.as_mut().ok_or(TuikitError::TerminalNotStarted)?;
        execute!(output, EnableMouseCapture)?;
        Ok(())
    }

    /// Disable mouse (send ANSI codes to disable mouse)
    fn disable_mouse(&mut self) -> Result<()> {
        let output = self.output.as_mut().ok_or(TuikitError::TerminalNotStarted)?;
        execute!(output, DisableMouseCapture)?;
        Ok(())
    }
}

impl<Output: Write> Drop for TermLock<Output> {
    fn drop(&mut self) {
        let _ = self.pause(true);
    }
}