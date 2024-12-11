use std::io::stderr;
use std::ops::{Deref, DerefMut};
use std::process;

use color_eyre::eyre::Result;
use crossterm::cursor;
use crossterm::event::KeyEventKind;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use futures::{FutureExt as _, StreamExt as _};
use ratatui::backend::CrosstermBackend as Backend;
use ratatui::{TerminalOptions, Viewport};
use tokio;
use tokio::sync::mpsc::unbounded_channel;
use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use super::{Event, Size};

pub struct Tui {
    pub terminal: ratatui::Terminal<Backend<std::io::Stderr>>,
    pub task: JoinHandle<()>,
    pub event_rx: UnboundedReceiver<Event>,
    pub event_tx: UnboundedSender<Event>,
    pub frame_rate: f64,
    pub tick_rate: f64,
    pub cancellation_token: CancellationToken,
    pub is_fullscreen: bool,
}

impl Tui {
    pub fn new_with_height(height: Size) -> Result<Self> {
        let backend = Backend::new(stderr());
        let event_channel = unbounded_channel();
        let (is_fullscreen, viewport) = match height {
            Size::Percent(100) => (true, Viewport::Fullscreen),
            Size::Fixed(lines) => (false, Viewport::Inline(lines)),
            Size::Percent(p) => todo!(),
        };
        set_panic_hook();
        Ok(Self {
            terminal: ratatui::Terminal::with_options(backend, TerminalOptions { viewport })?,
            task: tokio::spawn(async {}),
            event_rx: event_channel.1,
            event_tx: event_channel.0,
            frame_rate: 30.0,
            tick_rate: 10.0,
            cancellation_token: CancellationToken::default(),
            is_fullscreen,
        })
    }
    pub fn enter(&mut self) -> Result<()> {
        crossterm::terminal::enable_raw_mode()?;
        if self.is_fullscreen {
            crossterm::execute!(std::io::stderr(), EnterAlternateScreen, cursor::Hide)?;
        }
        self.start();
        Ok(())
    }

    pub fn exit(&mut self, status_code: i32) -> Result<()> {
        self.stop()?;
        if crossterm::terminal::is_raw_mode_enabled()? {
            self.flush()?;
            crossterm::execute!(std::io::stderr(), LeaveAlternateScreen, cursor::Show)?;
            crossterm::terminal::disable_raw_mode()?;
        }
        Ok(())
    }
    pub fn stop(&self) -> Result<()> {
        self.cancel();
        Ok(())
    }
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }
    pub fn start(&mut self) {
        let tick_delay = std::time::Duration::from_secs_f64(1.0 / self.tick_rate);
        let render_delay = std::time::Duration::from_secs_f64(1.0 / self.frame_rate);
        let _event_tx = self.event_tx.clone();
        let spawn = tokio::spawn(async move {
            let mut reader = crossterm::event::EventStream::new();
            let mut tick_interval = tokio::time::interval(tick_delay);
            let mut render_interval = tokio::time::interval(render_delay);
            loop {
                let tick_delay = tick_interval.tick();
                let render_delay = render_interval.tick();
                let crossterm_event = reader.next().fuse();
                tokio::select! {
                    maybe_event = crossterm_event => {
                      match maybe_event {
                        Some(Ok(evt)) => {
                          match evt {
                            crossterm::event::Event::Key(key) => {
                              if key.kind == KeyEventKind::Press {
                                _event_tx.send(Event::Key(key)).unwrap();
                              }
                            },
                              _ => ()
                          }
                        }
                        Some(Err(e)) => {
                          _event_tx.send(Event::Error(e.to_string())).unwrap();
                        }
                        None => {},
                      }
                    },
                    _ = tick_delay => {
                        _event_tx.send(Event::Heartbeat).unwrap();
                    },
                    _ = render_delay => {
                        _event_tx.send(Event::Render).unwrap();
                    },
                }
            }
        });
        self.task = spawn;
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.event_rx.recv().await
    }
}

impl Deref for Tui {
    type Target = ratatui::Terminal<Backend<std::io::Stderr>>;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

impl DerefMut for Tui {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terminal
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        self.exit(0).unwrap();
    }
}

fn set_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = ratatui::restore(); // ignore any errors as we are already failing
        hook(panic_info);
    }));
}