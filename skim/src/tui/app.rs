use std::cmp::max;
use std::rc::Rc;
use std::sync::Arc;

use crate::engine::fuzzy::{FuzzyEngine, FuzzyEngineBuilder};
use crate::helper::item_reader;
use crate::item::ItemPool;
use crate::matcher::{Matcher, MatcherControl};
use crate::prelude::ExactOrFuzzyEngineFactory;
use crate::{MatchEngine, SkimItem};

use super::header::Header;
use super::item_list::ItemList;
use super::statusline::StatusLine;
use super::Event;
use color_eyre::eyre::{bail, Result};
use crossterm::event::{KeyCode, KeyModifiers};
use defer_drop::DeferDrop;
use input::Input;
use preview::Preview;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode::Char;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::Widget;
use tokio::task::{self, JoinHandle};

use super::{input, preview, tui};

// App state
pub struct App<'a> {
    pub item_pool: Arc<DeferDrop<ItemPool>>,
    pub should_quit: bool,
    pub should_trigger_matcher: bool,
    pub cursor_pos: (u16, u16),
    pub matcher_control: MatcherControl,
    pub matcher: Matcher,

    pub input: Input,
    pub preview: Preview<'a>,
    pub header: Header,
    pub status: StatusLine,
    pub item_list: ItemList,
}

// App ui render function
impl Widget for &mut App<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
        ]);
        let [top, header, status, bottom] = layout.areas(area);
        let [top_left, top_right] = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(top);
        self.input.render(bottom, buf);
        self.header.render(header, buf);
        self.status.render(status, buf);
        self.preview.render(top_right, buf);
        self.item_list.render(top_left, buf);
        self.cursor_pos = (bottom.x + self.input.cursor_pos, bottom.y)
    }
}

impl Default for App<'_> {
    fn default() -> Self {
        Self {
            input: Input::default(),
            preview: Preview::default(),
            header: Header::default(),
            status: StatusLine::default(),
            item_pool: Arc::default(),
            item_list: ItemList::default(),
            should_quit: false,
            cursor_pos: (0, 0),
            matcher: Matcher::builder(Rc::new(ExactOrFuzzyEngineFactory::builder().build())).build(),
            should_trigger_matcher: false,
            matcher_control: MatcherControl::default(),
        }
    }
}

impl App<'_> {
    pub fn handle_event(&mut self, tui: &mut tui::Tui, event: &Event) -> Result<()> {
        match event {
            Event::Render => {
                tui.get_frame();
                tui.draw(|f| {
                    self.render(f.area(), f.buffer_mut());
                    f.set_cursor_position(self.cursor_pos);
                })?;
            }
            Event::Heartbeat => {
                self.should_trigger_matcher = true;
            }
            Event::Quit => {
                tui.exit(-1)?;
                self.should_quit = true;
            }
            Event::Close => {
                tui.exit(0)?;
                self.should_quit = true;
            }
            Event::PreviewReady(s) => {
                self.preview.content(s)?;
            }
            Event::Error(msg) => {
                tui.exit(1)?;
                bail!(msg.to_owned());
            }
            Event::NewItem(item) => {
                self.item_pool.append(vec![item.clone()]);
                if self.should_trigger_matcher {
                    self.should_trigger_matcher = false;
                    self.restart_matcher();
                }
                trace!("Got new item, len {}", self.item_pool.len());
            }
            Event::Key(key) => match key.modifiers {
                KeyModifiers::CONTROL => match key.code {
                    Char('c') => tui.event_tx.send(Event::Quit)?,
                    Char('w') => {
                        self.input.delete_word();
                        self.restart_matcher();
                    }
                    Char('g') => self.item_list.move_cursor_to(0),
                    Char('h') => self.item_list.move_cursor_to(max(1, self.item_list.items.len()) - 1),
                    _ => (),
                },
                KeyModifiers::NONE => match key.code {
                    Char(c) => {
                        self.input.insert(c);
                        self.restart_matcher();
                    }
                    KeyCode::Enter => {
                        self.item_list.select();
                        tui.event_tx.send(Event::Close)?;
                    }
                    KeyCode::Backspace => {
                        self.input.delete();
                        self.restart_matcher();
                    }
                    KeyCode::Left => self.input.move_cursor(-1),
                    KeyCode::Right => self.input.move_cursor(1),
                    KeyCode::Up => {
                        self.item_list.move_cursor_by(1);
                        self.preview.run(
                            tui,
                            &format!("bat --color=always {}", self.item_list.items[self.item_list.cursor].item.text()),
                        );
                    }
                    KeyCode::Down => self.item_list.move_cursor_by(-1),
                    KeyCode::Tab => self.item_list.toggle(),
                    _ => (),
                },
                KeyModifiers::SHIFT => match key.code {
                    Char(c) => {
                        self.input.insert(c);
                        self.restart_matcher();
                    }
                    _ => (),
                },
                _ => (),
            },
            _ => (),
        };
        Ok(())
    }
    fn restart_matcher(&mut self) {
        self.matcher_control.kill();
        let tx = self.item_list.tx.clone();
        self.item_pool.reset();
        self.matcher_control = self.matcher.run(&self.input, self.item_pool.clone(), move |matches| {
            debug!("Got results from matcher, sending to item list...");
            let _ = tx.send(matches.lock().clone());
        });
    }
    pub fn results(&self) -> Vec<Arc<dyn SkimItem>> {
        self.item_list
            .selection
            .iter()
            .map(|item| {
                debug!("res index: {}", item.item_idx);
                item.item.clone()
            })
            .collect()
    }
}
