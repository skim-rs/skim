use std::io::{BufWriter, stderr};
use std::ops::{Deref, DerefMut};
use std::process::Stdio;
use std::sync::Once;

use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, KeyEventKind,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{self, cursor};
use eyre::Result;
use futures::{FutureExt as _, StreamExt as _};
use ratatui::layout::Rect;
use ratatui::prelude::{Backend, CrosstermBackend};
use ratatui::{TerminalOptions, Viewport};
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::util::cursor_pos_from_tty;
use super::{Event, Size, TICK_RATE};

static PANIC_HOOK_SET: Once = Once::new();

/// Terminal user interface handler for skim
pub struct Tui<B: Backend = ratatui::backend::CrosstermBackend<BufWriter<std::io::Stderr>>>
where
    B::Error: Send + Sync + 'static,
{
    /// The ratatui terminal instance
    pub terminal: ratatui::Terminal<B>,
    /// Background task handle for event polling
    pub task: Option<JoinHandle<()>>,
    /// Receiver for TUI events
    pub event_rx: Receiver<Event>,
    /// Sender for TUI events
    pub event_tx: Sender<Event>,
    /// Tick rate for updates (ticks per second)
    pub tick_rate: f64,
    /// Token for cancelling background tasks
    pub cancellation_token: CancellationToken,
    /// Whether running in fullscreen mode
    pub is_fullscreen: bool,
    enable_mouse: bool,
}

impl Tui {
    /// Creates a TUI with the default backend (buffered stderr) and the specified height
    ///
    /// # Errors
    ///
    /// Returns an error if the TUI backend cannot be initialized.
    pub fn new_with_height(height: Size) -> Result<Self> {
        let backend = CrosstermBackend::new(std::io::BufWriter::new(stderr()));
        Self::new_with_height_and_backend(backend, height)
    }
    /// Disable mouse handling.
    /// Needs to be called before enter.
    pub fn disable_mouse(&mut self) -> &mut Self {
        self.enable_mouse = false;
        self
    }
}

impl<B: Backend> Tui<B>
where
    B::Error: Send + Sync + 'static,
{
    /// Creates a new TUI with the specified backend and height
    ///
    /// # Errors
    ///
    /// Returns an error if the terminal size cannot be determined or setup fails.
    ///
    /// # Panics
    ///
    /// Panics if the terminal size cannot be read from the backend.
    pub fn new_with_height_and_backend(backend: B, height: Size) -> Result<Self> {
        let event_channel = channel(1024 * 1024);

        let term_height = backend.size().expect("Failed to get terminal height").height;
        let lines = match height {
            Size::Percent(100) => None,
            Size::Fixed(lines) => Some(lines),
            Size::Percent(p) => Some(term_height * p / 100),
            Size::Neg(lines) => Some(term_height.saturating_sub(lines)),
        };

        let viewport = if let Some(mut height) = lines {
            // Until https://github.com/crossterm-rs/crossterm/issues/919 is fixed, we need to do it ourselves
            let cursor_pos = cursor_pos_from_tty()?;
            let mut y = cursor_pos.1 - 1;
            height = height.min(term_height);
            if term_height - cursor_pos.1 < height {
                let to_scroll = height - (term_height - cursor_pos.1) - 1;
                crossterm::execute!(stderr(), crossterm::terminal::ScrollUp(to_scroll))?;
                y = y.saturating_sub(to_scroll);
            }
            Viewport::Fixed(Rect::new(
                0,
                y,
                backend.size().expect("Failed to get terminal width").width - 1,
                height,
            ))
        } else {
            Viewport::Fullscreen
        };

        set_panic_hook();
        Ok(Self {
            terminal: ratatui::Terminal::with_options(backend, TerminalOptions { viewport })?,
            task: None,
            event_rx: event_channel.1,
            event_tx: event_channel.0,
            tick_rate: f64::from(TICK_RATE),
            cancellation_token: CancellationToken::default(),
            is_fullscreen: lines.is_none(),
            enable_mouse: true,
        })
    }

    /// Enters the TUI by enabling raw mode and starting event handling
    ///
    /// # Errors
    ///
    /// Returns an error if enabling raw mode or mouse capture fails.
    pub fn enter(&mut self) -> Result<()> {
        self.enter_terminal()?;
        self.start();
        Ok(())
    }

    /// Enables terminal modes and enters the alternate screen without starting event polling.
    ///
    /// This lets callers run terminal queries after alternate-screen entry but
    /// before the event stream starts reading terminal input.
    ///
    /// # Errors
    ///
    /// Returns an error if enabling raw mode or terminal features fails.
    pub fn enter_terminal(&mut self) -> Result<()> {
        crossterm::terminal::enable_raw_mode()?;
        // On Windows, install a console ctrl handler so that CTRL_C_EVENT
        // performs terminal cleanup instead of killing the process abruptly.
        #[cfg(windows)]
        super::windows::install_ctrl_c_handler()?;

        self.execute_enter()?;
        Ok(())
    }

    /// Exits the TUI by stopping event handling and disabling raw mode
    ///
    /// # Errors
    ///
    /// Returns an error if disabling raw mode or mouse capture fails.
    pub fn exit(&mut self) -> Result<()> {
        self.stop();
        cleanup_terminal()?;
        // Remove our console ctrl handler now that raw mode is off.
        #[cfg(windows)]
        super::windows::uninstall_ctrl_c_handler();
        // When using the inline layout, we want to remove all previous output
        //  -> reset cursor at the top of the drawing area
        if !self.is_fullscreen {
            let area = self.get_frame().area();
            let orig = ratatui::layout::Position { x: area.x, y: area.y };
            crossterm::execute!(
                stderr(),
                cursor::MoveTo(orig.x, orig.y),
                Clear(ClearType::FromCursorDown)
            )?;
            self.set_cursor_position(orig)?;
        }
        Ok(())
    }
    /// Stops the TUI event loop
    /// Equivalent to `self.cancel()`
    pub fn stop(&self) {
        self.cancel();
    }
    /// Forces the next [`draw`](ratatui::Terminal::draw) to repaint every cell.
    ///
    /// ratatui only writes cells that differ from the previously drawn buffer.
    /// After the display has been disturbed out from under it — e.g. an
    /// `execute` action that ran a child program and re-entered the alternate
    /// screen — that cached buffer is stale and a normal draw would leave the
    /// screen partially blank. Resetting *both* double buffers makes the next
    /// draw diff against an empty buffer and thus repaint everything.
    ///
    /// Unlike [`ratatui::Terminal::clear`], this performs no cursor-position
    /// query (which crossterm writes to stdout and which stalls when stdout is
    /// redirected), and it is viewport-agnostic (works for fullscreen and
    /// inline layouts alike).
    pub fn force_full_redraw(&mut self) {
        self.terminal.swap_buffers();
        self.terminal.swap_buffers();
    }
    /// Stops the input reader and waits for it to release the terminal.
    ///
    /// Unlike [`stop`](Self::stop), this blocks until the background task has
    /// observed the cancellation and dropped its `EventStream`, so crossterm's
    /// internal reader thread has stopped reading the terminal before this
    /// returns. Call this before handing the terminal to a foreground child
    /// process (e.g. an `execute` action): otherwise skim's reader competes
    /// with the child for keystrokes and interactive TUIs appear to freeze.
    ///
    /// Restart the reader afterwards with [`start`](Self::start).
    ///
    /// # Panics
    ///
    /// Panics if called from outside a multi-threaded Tokio runtime, since it
    /// uses `block_in_place` to await the reader task from synchronous code.
    pub fn stop_and_join(&mut self) {
        self.cancel();
        if let Some(task) = self.task.take() {
            // We are on a synchronous call stack nested inside the async event
            // loop. `block_in_place` moves this worker off the async pool so we
            // can block on the task's completion without starving the runtime.
            let _ = tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(task));
        }
    }
    /// Cancels all background tasks
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }
    /// Starts the event loop for handling keyboard and timer events
    pub fn start(&mut self) {
        let tick_delay = std::time::Duration::from_secs_f64(1.0 / self.tick_rate);
        let event_tx_clone = self.event_tx.clone();
        // Cancel any previously running reader before spawning a new one.
        if self.task.is_some() {
            self.cancel();
        }
        // Install a fresh cancellation token: a `CancellationToken` stays
        // cancelled once cancelled, so reusing the old one (after `stop`,
        // `stop_and_join`, or a prior `start`) would make the new task observe
        // the cancellation immediately and exit without reading any input.
        // This is what lets the reader resume after an `execute` action.
        self.cancellation_token = CancellationToken::new();
        let cancellation_token_clone = self.cancellation_token.clone();
        self.task = Some(tokio::spawn(async move {
            let mut reader = crossterm::event::EventStream::new();
            let mut tick_interval = tokio::time::interval(tick_delay);
            loop {
                let tick_delay = tick_interval.tick();
                let crossterm_event = reader.next().fuse();
                tokio::select! {
                    () = cancellation_token_clone.cancelled() => {
                        break;
                    }
                    maybe_event = crossterm_event => {
                      match maybe_event {
                        Some(Ok(crossterm::event::Event::Key(key))) => {
                          if key.kind == KeyEventKind::Press {
                            _ = event_tx_clone.try_send(Event::Key(key));
                          }
                        }
                        Some(Ok(crossterm::event::Event::Paste(text))) => {
                          _ = event_tx_clone.try_send(Event::Paste(text));
                        }
                        Some(Ok(crossterm::event::Event::Mouse(mouse))) => {
                          _ = event_tx_clone.try_send(Event::Mouse(mouse));
                        }
                        Some(Ok(crossterm::event::Event::Resize(cols, rows))) => {
                          _ = event_tx_clone.try_send(Event::Resize(cols, rows));
                          _ = event_tx_clone.try_send(Event::Render);
                        }
                        Some(Err(e)) => {
                          _ = event_tx_clone.try_send(Event::Error(e.to_string()));
                        }
                        None | Some(Ok(_)) => {},
                      }
                    },
                    _ = tick_delay => {
                        _ = event_tx_clone.try_send(Event::Heartbeat);
                    },
                }
            }
        }));
    }

    /// Gets the next event from the event queue
    pub async fn next(&mut self) -> Option<Event> {
        self.event_rx.recv().await
    }

    fn execute_enter(&self) -> Result<()> {
        crossterm::execute!(stderr(), EnableBracketedPaste)?;
        if self.enable_mouse {
            crossterm::execute!(stderr(), EnableMouseCapture)?;
        }
        if self.is_fullscreen {
            crossterm::execute!(stderr(), EnterAlternateScreen, cursor::Hide)?;
        }
        if let Err(e) = crossterm::execute!(
            stderr(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        ) {
            warn!("Failed to enable keyboard enhancement flags: {e}");
        }
        Ok(())
    }

    fn execute_leave(&self) -> Result<()> {
        crossterm::execute!(stderr(), DisableBracketedPaste)?;
        if self.enable_mouse {
            crossterm::execute!(stderr(), DisableMouseCapture)?;
        }
        if let Err(e) = crossterm::execute!(stderr(), PopKeyboardEnhancementFlags) {
            warn!("Failed to remove keyboard enhancement flags: {e}");
        }
        if self.is_fullscreen {
            crossterm::execute!(stderr(), LeaveAlternateScreen, cursor::Show)?;
        }
        Ok(())
    }

    /// Pauses the TUI by disabling raw mode, exiting alternate screen etc.
    /// Returns true if we were in raw mode, false otherwise. Used to restore to the same state later.
    ///
    /// # Errors
    ///
    /// This propagates the errors of the `disable_raw_mode` and `crossterm::execute` calls.
    pub fn pause(&mut self) -> Result<bool> {
        let in_raw_mode = crossterm::terminal::is_raw_mode_enabled().unwrap_or(false);
        if in_raw_mode {
            crossterm::terminal::disable_raw_mode()?;
        }
        self.execute_leave()?;

        Ok(in_raw_mode)
    }

    /// Resumes the TUI after a `pause()`
    /// Takes `in_raw_mode`, the boolean returned by `pause()`
    ///
    /// # Errors
    /// This propagates the errors of the `disable_raw_mode` and `crossterm::execute` calls.
    pub fn resume(&mut self, in_raw_mode: bool) -> Result<()> {
        if in_raw_mode {
            crossterm::terminal::enable_raw_mode()?;
        }
        self.execute_enter()?;
        Ok(())
    }

    /// Run a command in the foreground, temporarily handing it the terminal.
    ///
    /// This suspends skim's own input reader (via [`Tui::stop_and_join`]) so it
    /// does not compete with the child for terminal input — the cause of
    /// interactive TUIs freezing after a few keystrokes — leaves the alternate
    /// screen and raw mode, runs the command to completion, then restores skim's
    /// terminal state and restarts the reader. The child is given its own handle
    /// to the controlling terminal as stdin (see [`execute_child_stdin`]).
    pub(crate) fn run_execute(&mut self, cmd: &str) -> Result<()> {
        use std::io::IsTerminal as _;

        let has_tty = std::io::stderr().is_terminal();
        let mut in_raw_mode = false;

        // Stop skim's input reader and wait for it to release the terminal, so the
        // child is the only reader of keystrokes while it runs.
        self.stop_and_join();

        if has_tty {
            in_raw_mode = self.pause()?;
        }

        let mut command = crate::shell_cmd(cmd);
        command.stdin(execute_child_stdin());
        let _ = command.spawn().and_then(|mut c| c.wait());

        let mut restore_result = Ok(());
        if has_tty {
            restore_result = self.resume(in_raw_mode);
        }

        // Resume skim's input reader now that the terminal is ours again.
        self.start();
        restore_result
    }
}

impl<B: Backend> Deref for Tui<B>
where
    B::Error: Send + Sync + 'static,
{
    type Target = ratatui::Terminal<B>;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

impl<B: Backend> DerefMut for Tui<B>
where
    B::Error: Send + Sync + 'static,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terminal
    }
}

impl<B: Backend> Drop for Tui<B>
where
    B::Error: Send + Sync + 'static,
{
    fn drop(&mut self) {
        if let Some(t) = self.task.take() {
            t.abort();
        }
        let _ = self.exit();
    }
}

fn set_panic_hook() {
    PANIC_HOOK_SET.call_once(|| {
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            let _ = cleanup_terminal();
            #[cfg(windows)]
            super::windows::uninstall_ctrl_c_handler();
            hook(panic_info);
        }));
    });
}

/// Perform terminal cleanup: disable mouse capture, bracketed paste,
/// leave alternate screen, show cursor, and disable raw mode.
///
/// This is safe to call from any thread since:
/// - Escape sequences are written atomically to stderr
/// - `SetConsoleMode` (used by `disable_raw_mode`) is thread-safe on Windows
pub(crate) fn cleanup_terminal() -> std::io::Result<()> {
    if let Err(e) = crossterm::execute!(stderr(), PopKeyboardEnhancementFlags) {
        warn!("Failed to remove keyboard enhancement flags: {e}");
    }
    crossterm::execute!(
        stderr(),
        DisableMouseCapture,
        DisableBracketedPaste,
        LeaveAlternateScreen,
        cursor::Show
    )?;
    crossterm::terminal::disable_raw_mode()?;
    Ok(())
}

/// Build the stdin handle for an `execute` child process.
///
/// skim's own stdin (fd 0) is frequently a pipe carrying the item list
/// (e.g. `find | sk`), which is useless as a keyboard source for an
/// interactive child. Hand the child a fresh handle to the controlling
/// terminal instead, so programs like `ncdu` or other ncurses TUIs can read
/// the keyboard even when skim's stdin is a pipe. Falls back to inheriting
/// skim's stdin if the terminal cannot be opened.
fn execute_child_stdin() -> std::process::Stdio {
    #[cfg(unix)]
    let tty = std::fs::File::open("/dev/tty");
    #[cfg(windows)]
    let tty = std::fs::OpenOptions::new().read(true).write(true).open("CONIN$");
    tty.map_or_else(|_| Stdio::inherit(), Stdio::from)
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    fn fullscreen_tui() -> Tui<TestBackend> {
        // Percent(100) selects the fullscreen viewport, avoiding any TTY cursor query.
        Tui::new_with_height_and_backend(TestBackend::new(80, 24), Size::Percent(100))
            .expect("failed to build test TUI")
    }

    #[test]
    fn new_with_full_height_is_fullscreen() {
        let tui = fullscreen_tui();
        assert!(tui.is_fullscreen);
        assert!(tui.enable_mouse);
    }

    #[test]
    fn stop_cancels_token() {
        let tui = fullscreen_tui();
        assert!(!tui.cancellation_token.is_cancelled());
        tui.stop();
        assert!(tui.cancellation_token.is_cancelled());
    }

    #[test]
    fn cancel_is_idempotent() {
        let tui = fullscreen_tui();
        tui.cancel();
        tui.cancel();
        assert!(tui.cancellation_token.is_cancelled());
    }

    #[test]
    fn deref_exposes_terminal_frame() {
        let mut tui = fullscreen_tui();
        // Deref/DerefMut should expose the underlying ratatui terminal.
        let area = tui.get_frame().area();
        assert_eq!(area.width, 80);
        assert_eq!(area.height, 24);
    }
}
