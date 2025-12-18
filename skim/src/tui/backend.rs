use std::ops::{Deref, DerefMut};

use color_eyre::eyre::{Context, Result};
use crossterm::cursor;
use crossterm::event::KeyEventKind;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use futures::{FutureExt as _, StreamExt as _};
use ratatui::prelude::Backend;
use ratatui::{TerminalOptions, Viewport};
use tokio;
use tokio::sync::mpsc::unbounded_channel;
use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use super::{Event, Size};

/// Terminal user interface handler for skim
pub struct Tui<B: Backend = ratatui::backend::CrosstermBackend<std::io::Stderr>> {
    /// The ratatui terminal instance
    pub terminal: ratatui::Terminal<B>,
    /// Background task handle for event polling
    pub task: Option<JoinHandle<()>>,
    /// Receiver for TUI events
    pub event_rx: UnboundedReceiver<Event>,
    /// Sender for TUI events
    pub event_tx: UnboundedSender<Event>,
    /// Frame rate for rendering (frames per second)
    pub frame_rate: f64,
    /// Tick rate for updates (ticks per second)
    pub tick_rate: f64,
    /// Token for cancelling background tasks
    pub cancellation_token: CancellationToken,
    /// Whether running in fullscreen mode
    pub is_fullscreen: bool,
}

impl<B: Backend> Tui<B> {
    /// Creates a new TUI with the specified backend and height
    pub fn new_with_height(backend: B, height: Size) -> Result<Self> {
        let event_channel = unbounded_channel();
        let (is_fullscreen, viewport) = match height {
            Size::Percent(100) => (true, Viewport::Fullscreen),
            Size::Fixed(lines) => (false, Viewport::Inline(lines)),
            Size::Percent(p) => {
                let term_height = backend.size().context("Failed to get terminal size")?.height;
                (false, Viewport::Inline(term_height * p / 100))
            }
        };
        set_panic_hook();
        Ok(Self {
            terminal: ratatui::Terminal::with_options(backend, TerminalOptions { viewport })?,
            task: None,
            event_rx: event_channel.1,
            event_tx: event_channel.0,
            frame_rate: 12.0,
            tick_rate: 12.0,
            cancellation_token: CancellationToken::default(),
            is_fullscreen,
        })
    }
    /// Enters the TUI by enabling raw mode and starting event handling
    pub fn enter(&mut self) -> Result<()> {
        crossterm::terminal::enable_raw_mode()?;
        if self.is_fullscreen {
            crossterm::execute!(std::io::stderr(), EnterAlternateScreen, cursor::Hide)?;
        }
        self.start();
        Ok(())
    }

    /// Exits the TUI by stopping event handling and disabling raw mode
    pub fn exit(&mut self) -> Result<()> {
        self.stop()?;
        if crossterm::terminal::is_raw_mode_enabled()? {
            self.flush()?;
            crossterm::execute!(std::io::stderr(), LeaveAlternateScreen, cursor::Show)?;
            crossterm::terminal::disable_raw_mode()?;
        }
        // When using the inline layout, we want to remove all previous output
        //  -> reset cursor at the top of the drawing area
        if !self.is_fullscreen {
            let area = self.get_frame().area();
            let orig = ratatui::layout::Position { x: area.x, y: area.y };
            self.set_cursor_position(orig)?;
        };
        Ok(())
    }
    /// Stops the TUI event loop
    pub fn stop(&self) -> Result<()> {
        self.cancel();
        Ok(())
    }
    /// Cancels all background tasks
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }
    /// Starts the event loop for handling keyboard and timer events
    pub fn start(&mut self) {
        let tick_delay = std::time::Duration::from_secs_f64(1.0 / self.tick_rate);
        let render_delay = std::time::Duration::from_secs_f64(1.0 / self.frame_rate);
        let _event_tx = self.event_tx.clone();
        let _cancellation_token = self.cancellation_token.clone();
        self.task = Some(tokio::spawn(async move {
            let mut reader = crossterm::event::EventStream::new();
            let mut tick_interval = tokio::time::interval(tick_delay);
            let mut render_interval = tokio::time::interval(render_delay);
            loop {
                let tick_delay = tick_interval.tick();
                let render_delay = render_interval.tick();
                let crossterm_event = reader.next().fuse();
                tokio::select! {
                    _ = _cancellation_token.cancelled() => {
                        break;
                    }
                    maybe_event = crossterm_event => {
                      match maybe_event {
                        Some(Ok(crossterm::event::Event::Key(key))) => {
                          if key.kind == KeyEventKind::Press {
                            _ = _event_tx.send(Event::Key(key));
                          }
                        }
                        Some(Err(e)) => {
                          _ = _event_tx.send(Event::Error(e.to_string()));
                        }
                        None | Some(Ok(_)) => {},
                      }
                    },
                    _ = tick_delay => {
                        _ = _event_tx.send(Event::Heartbeat);
                    },
                    _ = render_delay => {
                        _ = _event_tx.send(Event::Render);
                    },
                }
            }
        }));
    }

    /// Gets the next event from the event queue
    pub async fn next(&mut self) -> Option<Event> {
        self.event_rx.recv().await
    }
}

impl<B: Backend> Deref for Tui<B> {
    type Target = ratatui::Terminal<B>;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

impl<B: Backend> DerefMut for Tui<B> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terminal
    }
}

impl<B: Backend> Drop for Tui<B> {
    fn drop(&mut self) {
        let _ = self.exit();
    }
}

fn set_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        ratatui::restore(); // ignore any errors as we are already failing
        hook(panic_info);
    }));
}
