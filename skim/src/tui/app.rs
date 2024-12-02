use std::sync::Arc;

use crate::SkimItem;

use super::header::Header;
use super::statusline::StatusLine;
use super::Event;
use color_eyre::eyre::{bail, Result};
use crossterm::event::{KeyCode, KeyModifiers};
use input::Input;
use preview::{run_preview, Preview};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode::Char;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::Widget;
use tokio::task::JoinHandle;

use super::{input, preview, tui};

// App state
pub struct App {
    pub input: Input,
    pub preview: Preview,
    pub header: Header,
    pub status: StatusLine,
    pub should_quit: bool,
    pub preview_handle: JoinHandle<()>,
    pub cursor_pos: (u16, u16),
    pub results: Vec<Arc<dyn SkimItem>>
}

// App ui render function
impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::vertical([Constraint::Fill(1), Constraint::Length(2), Constraint::Length(2), Constraint::Length(2)]);
        let [top, header, status, bottom] = layout.areas(area);
        self.input.render(bottom, buf);
        self.header.render(header, buf);
        self.status.render(status, buf);
        self.preview.render(top, buf);
        self.cursor_pos = (bottom.x + self.input.cursor_pos, bottom.y)
    }
}

impl Default for App {
    fn default() -> Self {
        Self {
            input: Input::default(),
            preview: Preview::default(),
            header: Header::default(),
            status: StatusLine::default(),
            preview_handle: tokio::spawn(async {}),
            should_quit: false,
            cursor_pos: (0, 0),
            results: Vec::default()
        }
    }
}

impl App {
    pub fn handle_event(&mut self, tui: &mut tui::Tui, event: &Event) -> Result<()> {
        match event {
            Event::Render => {
                tui.get_frame();
                tui.draw(|f| {
                    self.render(f.area(), f.buffer_mut());
                    f.set_cursor_position(self.cursor_pos);
                })?;
            }
            Event::Quit => {
                self.preview_handle.abort();
                tui.exit(-1)?
            }
            Event::Close => {
                self.preview_handle.abort();
                tui.exit(0)?
            }
            Event::PreviewReady(s) => {
                self.preview.content = s.clone();
            }
            Event::Error(msg) => {
                self.preview_handle.abort();
                tui.exit(1)?;
                bail!(msg.to_owned());
            }
            Event::Key(key) => match key.modifiers {
                KeyModifiers::CONTROL => match key.code {
                    Char('c') => tui.event_tx.send(Event::Quit)?,
                    Char('w') => {
                        self.input.delete_word();
                        run_preview(self, tui)?;
                    }
                    _ => (),
                },
                KeyModifiers::NONE => match key.code {
                    Char(c) => {
                        self.input.insert(c);
                        run_preview(self, tui)?;
                    }
                    KeyCode::Enter => tui.event_tx.send(Event::Close)?,
                    KeyCode::Backspace => {
                        self.input.delete();
                        run_preview(self, tui)?;
                    }
                    KeyCode::Left => self.input.move_cursor(-1),
                    KeyCode::Right => self.input.move_cursor(1),
                    _ => (),
                },
                KeyModifiers::SHIFT => match key.code {
                    Char(c) => {
                        self.input.insert(c);
                        run_preview(self, tui)?;
                    }
                    _ => (),
                },
                _ => {}
            },
            _ => (),
        }
        Ok(())
    }
}
