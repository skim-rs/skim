use std::cmp::max;
use std::rc::Rc;
use std::sync::Arc;

use crate::binds::KeyMap;
use crate::item::ItemPool;
use crate::matcher::{Matcher, MatcherControl};
use crate::prelude::ExactOrFuzzyEngineFactory;
use crate::SkimItem;

use super::event::Action;
use super::header::Header;
use super::item_list::ItemList;
use super::statusline::StatusLine;
use super::Event;
use color_eyre::eyre::{bail, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use defer_drop::DeferDrop;
use input::Input;
use preview::Preview;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode::Char;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::Widget;

use super::{input, preview, tui};

// App state
pub struct App<'a> {
    pub item_pool: Arc<DeferDrop<ItemPool>>,
    pub should_quit: bool,
    pub should_trigger_matcher: bool,
    pub cursor_pos: (u16, u16),
    pub matcher_control: MatcherControl,
    pub matcher: Matcher,
    pub keymap: KeyMap,

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
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ]);
        let [top, header, status, bottom] = layout.areas(area);
        let [top_left, top_right] = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(top);
        self.input.render(bottom, buf);
        self.header.render(header, buf);
        self.status.render(status, buf);
        self.preview.render(top_right, buf);
        self.item_list.render(top_left, buf);
        self.cursor_pos = (bottom.x + self.input.cursor_pos(), bottom.y)
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
            keymap: crate::binds::get_default_key_map(),
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
            Event::RunPreview => {
                self.preview.run(
                    tui,
                    &format!(
                        "bat --color=always {}",
                        self.item_list.items[self.item_list.cursor].item.text()
                    ),
                );
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
            Event::Action(act) => {
                if let Some(evt) = self.handle_action(act) {
                    tui.event_tx.send(evt)?;
                }
            }
            Event::NewItem(item) => {
                self.item_pool.append(vec![item.clone()]);
                self.restart_matcher(false);
                trace!("Got new item, len {}", self.item_pool.len());
            }
            Event::Key(key) => {
                for evt in self.handle_key(key) {
                    tui.event_tx.send(evt)?;
                }
            }
            _ => (),
        };
        Ok(())
    }
    fn handle_key(&mut self, key: &KeyEvent) -> Vec<Event> {
        let act = self.keymap.get(key);
        if act.is_some() {
            return act.unwrap().iter().map(|a| Event::Action(a.clone())).collect();
        }
        match key.modifiers {
            KeyModifiers::CONTROL => match key.code {
                Char('c') => return vec![Event::Quit],
                Char('w') => {
                    self.input.delete_word();
                    self.restart_matcher(true);
                }
                Char('g') => {
                    self.item_list.move_cursor_to(0);
                }
                Char('h') => {
                    self.item_list.move_cursor_to(max(1, self.item_list.items.len()) - 1);
                }
                _ => (),
            },
            KeyModifiers::NONE => match key.code {
                Char(c) => return vec![Event::Action(Action::AddChar(c))],
                KeyCode::Enter => {
                    self.item_list.select();
                    return vec![Event::Close];
                }
                KeyCode::Backspace => {
                    self.input.delete(-1);
                    self.restart_matcher(true);
                }
                KeyCode::Left => {
                    self.input.move_cursor(-1);
                }
                KeyCode::Right => {
                    self.input.move_cursor(1);
                }
                KeyCode::Up => {
                    self.item_list.move_cursor_by(1);
                    return vec![Event::RunPreview];
                }
                KeyCode::Down => {
                    self.item_list.move_cursor_by(-1);
                    return vec![Event::RunPreview];
                }
                KeyCode::Tab => {
                    self.item_list.toggle();
                }
                _ => (),
            },
            KeyModifiers::SHIFT => match key.code {
                Char(c) => return vec![Event::Action(Action::AddChar(c.to_uppercase().next().unwrap()))],
                _ => (),
            },
            _ => (),
        };
        return vec![];
    }

    fn handle_action(&mut self, act: &Action) -> Option<Event> {
        use Action::*;
        match act {
            Abort => {
                return Some(Event::Quit);
            }
            Accept(_) => {
                return Some(Event::Close);
            }
            AddChar(c) => {
                self.input.insert(*c);
                self.restart_matcher(true);
            }
            AppendAndSelect => {
                let value = self.input.clone();
                let item: Arc<dyn SkimItem> = Arc::new(value);
                self.item_pool.append(vec![item]);
                self.restart_matcher(false);
            }
            BackwardChar => {
                self.input.move_cursor(-1);
            }
            BackwardDeleteChar => {
                self.input.delete(-1);
                self.restart_matcher(true);
            }
            BackwardKillWord => {
                self.input.delete_word();
                self.restart_matcher(true);
            }
            BackwardWord => todo!(),
            BeginningOfLine => {
                self.input.move_cursor_to(0);
            }
            Cancel => {
                todo!();
            }
            ClearScreen => {
                todo!();
            }
            DeleteChar => {
                self.input.delete(1);
                self.restart_matcher(true);
            }
            DeleteCharEOF => {
                self.input.delete(1);
                self.restart_matcher(true);
            }
            DeselectAll => {
                self.item_list.selection = Default::default();
            }
            Down(offset) => {
                self.item_list.move_cursor_by(*offset);
            }
            EndOfLine => {
                self.input.move_cursor_to(self.input.len() as u16);
            }
            Execute(cmd) => todo!(),
            ExecuteSilent(cmd) => todo!(),
            ForwardChar => {
                self.input.move_cursor(1);
            }
            ForwardWord => {
                todo!();
            }
            IfQueryEmpty(act) => todo!(),
            IfQueryNotEmpty(act) => todo!(),
            IfNonMatched(act) => todo!(),
            Ignore => (),
            KillLine => todo!(),
            KillWord => todo!(),
            NextHistory => todo!(),
            HalfPageDown(n) => todo!(),
            HalfPageUp(n) => todo!(),
            PageDown(n) => todo!(),
            PageUp(n) => todo!(),
            PreviewUp(n) => todo!(),
            PreviewDown(n) => todo!(),
            PreviewLeft(n) => todo!(),
            PreviewRight(n) => todo!(),
            PreviewPageUp(n) => todo!(),
            PreviewPageDown(n) => todo!(),
            PreviousHistory => todo!(),
            Redraw => todo!(),
            Reload(Some(s)) => todo!(),
            Reload(None) => todo!(),
            RefreshCmd => todo!(),
            RefreshPreview => {
                return Some(Event::RunPreview);
            }
            RotateMode => todo!(),
            ScrollLeft(n) => todo!(),
            ScrollRight(n) => todo!(),
            SelectAll => todo!(),
            SelectRow(usize) => todo!(),
            Toggle => self.item_list.toggle(),
            ToggleAll => todo!(),
            ToggleIn => todo!(),
            ToggleInteractive => todo!(),
            ToggleOut => todo!(),
            TogglePreview => todo!(),
            TogglePreviewWrap => todo!(),
            ToggleSort => todo!(),
            UnixLineDiscard => todo!(),
            UnixWordRubout => todo!(),
            Up(n) => todo!(),
            Yank => todo!(),
        }
        return None;
    }

    fn restart_matcher(&mut self, mut force: bool) {
        if self.should_trigger_matcher {
            self.should_trigger_matcher = false;
            force = true;
        }
        if force {
            self.matcher_control.kill();
            let tx = self.item_list.tx.clone();
            self.item_pool.reset();
            self.matcher_control = self.matcher.run(&self.input, self.item_pool.clone(), move |matches| {
                debug!("Got results from matcher, sending to item list...");
                let _ = tx.send(matches.lock().clone());
            });
        }
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
