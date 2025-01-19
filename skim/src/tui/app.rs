use std::borrow::Cow;
use std::cmp::max;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::Arc;

use crate::binds::KeyMap;
use crate::item::ItemPool;
use crate::matcher::{Matcher, MatcherControl};
use crate::prelude::ExactOrFuzzyEngineFactory;
use crate::util::printf;
use crate::SkimItem;

use super::event::Action;
use super::header::Header;
use super::item_list::ItemList;
use super::options::TuiOptions;
use super::statusline::StatusLine;
use super::Event;
use color_eyre::eyre::{bail, Result};
use crossbeam::epoch::Pointable;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use defer_drop::DeferDrop;
use input::Input;
use preview::Preview;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode::Char;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::Widget;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use super::{input, preview, tui};

// App state
pub struct App<'a> {
    pub item_pool: Arc<DeferDrop<ItemPool>>,
    pub should_quit: bool,
    pub should_trigger_matcher: bool,
    pub cursor_pos: (u16, u16),
    pub matcher_control: MatcherControl,
    pub matcher: Matcher,
    pub yank_register: Cow<'a, str>,
    pub item_rx: UnboundedReceiver<Arc<dyn SkimItem>>,
    pub item_tx: UnboundedSender<Arc<dyn SkimItem>>,

    pub input: Input,
    pub preview: Preview<'a>,
    pub header: Header,
    pub status: StatusLine,
    pub item_list: ItemList,

    pub options: TuiOptions,
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
        let (item_tx, item_rx) = unbounded_channel();
        Self {
            input: Input::default(),
            preview: Preview::default(),
            header: Header::default(),
            status: StatusLine::default(),
            item_pool: Arc::default(),
            item_list: ItemList::default(),
            item_rx,
            item_tx,
            should_quit: false,
            cursor_pos: (0, 0),
            matcher: Matcher::builder(Rc::new(ExactOrFuzzyEngineFactory::builder().build())).build(),
            yank_register: Cow::default(),
            should_trigger_matcher: false,
            matcher_control: MatcherControl::default(),
            options: TuiOptions::default(),
        }
    }
}

impl<'a> App<'a> {
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
                if self.options.preview.is_some() {
                    self.preview.run(
                        tui,
                        &printf(
                            self.options.preview.clone().unwrap(),
                            &self.options.delimiter,
                            self.item_list.selection.iter().map(|m| m.item.clone()),
                            self.item_list.selected(),
                            &self.input,
                            &self.input,
                        ),
                    );
                }
            }
            Event::Clear => {
                tui.clear()?;
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
                for evt in self.handle_action(act)? {
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
    pub fn handle_items(&mut self, items: Vec<Arc<dyn SkimItem>>) {
        self.item_pool.append(items);
        // self.restart_matcher(false);
        trace!("Got new items, len {}", self.item_pool.len());
    }
    fn handle_key(&mut self, key: &KeyEvent) -> Vec<Event> {
        let act = self.options.keymap.get(key);
        if act.is_some() {
            return act.unwrap().iter().map(|a| Event::Action(a.clone())).collect();
        }
        match key.modifiers {
            KeyModifiers::CONTROL => match key.code {
                Char('c') => return vec![Event::Quit],
                _ => (),
            },
            KeyModifiers::NONE => match key.code {
                Char(c) => return vec![Event::Action(Action::AddChar(c))],
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

    fn handle_action(&mut self, act: &Action) -> Result<Vec<Event>> {
        use Action::*;
        match act {
            Abort => {
                return Ok(vec![Event::Quit]);
            }
            Accept(_) => {
                return Ok(vec![Event::Close]);
            }
            AddChar(c) => {
                self.input.insert(*c);
                self.restart_matcher(true);
                return Ok(vec![Event::RunPreview]);
            }
            AppendAndSelect => {
                let value = self.input.clone();
                let item: Arc<dyn SkimItem> = Arc::new(value);
                self.item_pool.append(vec![item]);
                self.restart_matcher(true);
                return Ok(vec![Event::RunPreview]);
            }
            BackwardChar => {
                self.input.move_cursor(-1);
            }
            BackwardDeleteChar => {
                self.input.delete(-1);
                self.restart_matcher(true);
                return Ok(vec![Event::RunPreview]);
            }
            BackwardKillWord => {
                let deleted = Cow::Owned(self.input.delete_backward_word());
                self.yank(deleted);
                self.restart_matcher(true);
                return Ok(vec![Event::RunPreview]);
            }
            BackwardWord => {
                self.input.delete_backward_word();
                self.restart_matcher(true);
                return Ok(vec![Event::RunPreview]);
            }
            BeginningOfLine => {
                self.input.move_cursor_to(0);
            }
            Cancel => {
                self.matcher_control.kill();
                self.preview.thread_handle.abort();
            }
            ClearScreen => {
                return Ok(vec![Event::Clear]);
            }
            DeleteChar => {
                self.input.delete(1);
                self.restart_matcher(true);
                return Ok(vec![Event::RunPreview]);
            }
            DeleteCharEOF => {
                self.input.delete(1);
                self.restart_matcher(true);
                return Ok(vec![Event::RunPreview]);
            }
            DeselectAll => {
                self.item_list.selection = Default::default();
                return Ok(vec![Event::RunPreview]);
            }
            Down(offset) => {
                self.item_list.move_cursor_by(-*offset);
                return Ok(vec![Event::RunPreview]);
            }
            EndOfLine => {
                self.input.move_cursor_to(self.input.len() as u16);
            }
            Execute(cmd) => {
                let mut command = Command::new("sh");
                command.args(&["-c", cmd]);
                let _ = command.spawn();
            }
            ExecuteSilent(cmd) => {
                let mut command = Command::new("sh");
                command.args(&["-c", cmd]);
                command.stdout(Stdio::null());
                command.stderr(Stdio::null());
                let _ = command.spawn();
            }
            ForwardChar => {
                self.input.move_cursor(1);
            }
            ForwardWord => {
                todo!()
            }
            IfQueryEmpty(act) => {
                let inner = crate::binds::parse_action_chain(act)?;
                if self.input.is_empty() {
                    return Ok(inner.iter().map(|e| Event::Action(e.to_owned())).collect());
                }
            }
            IfQueryNotEmpty(act) => {
                let inner = crate::binds::parse_action_chain(act)?;
                if !self.input.is_empty() {
                    return Ok(inner.iter().map(|e| Event::Action(e.to_owned())).collect());
                }
            }
            IfNonMatched(act) => {
                let inner = crate::binds::parse_action_chain(act)?;
                if self.item_list.items.is_empty() {
                    return Ok(inner.iter().map(|e| Event::Action(e.to_owned())).collect());
                }
            }
            Ignore => (),
            KillLine => {
                let cursor = self.input.cursor_pos as usize;
                let deleted = Cow::Owned(self.input.split_off(cursor));
                self.yank(deleted);
                return Ok(vec![Event::RunPreview]);
            }
            KillWord => {
                let deleted = Cow::Owned(self.input.delete_backward_word());
                self.yank(deleted);
                return Ok(vec![Event::RunPreview]);
            }
            NextHistory => todo!(),
            HalfPageDown(n) => {
                let offset = self.item_list.view_range.1.abs_diff(self.item_list.view_range.0) as i32;
                self.item_list.move_cursor_by(offset * n / 2);
                return Ok(vec![Event::RunPreview]);
            }
            HalfPageUp(n) => {
                let offset = self.item_list.view_range.1.abs_diff(self.item_list.view_range.0) as i32;
                self.item_list.move_cursor_by(-offset * n / 2);
                return Ok(vec![Event::RunPreview]);
            }
            PageDown(n) => {
                let offset = self.item_list.view_range.1.abs_diff(self.item_list.view_range.0) as i32;
                self.item_list.move_cursor_by(offset * n);
                return Ok(vec![Event::RunPreview]);
            }
            PageUp(n) => {
                let offset = self.item_list.view_range.1.abs_diff(self.item_list.view_range.0) as i32;
                self.item_list.move_cursor_by(-offset * n);
                return Ok(vec![Event::RunPreview]);
            }
            PreviewUp(n) => todo!(),
            PreviewDown(n) => todo!(),
            PreviewLeft(n) => todo!(),
            PreviewRight(n) => todo!(),
            PreviewPageUp(n) => todo!(),
            PreviewPageDown(n) => todo!(),
            PreviousHistory => todo!(),
            Redraw => return Ok(vec![Event::Clear]),
            Reload(Some(s)) => todo!(),
            Reload(None) => todo!(),
            RefreshCmd => todo!(),
            RefreshPreview => {
                return Ok(vec![Event::RunPreview]);
            }
            RotateMode => todo!(),
            ScrollLeft(n) => todo!(),
            ScrollRight(n) => todo!(),
            SelectAll => self.item_list.select_all(),
            SelectRow(row) => self.item_list.select_row(*row),
            Toggle => self.item_list.toggle(),
            ToggleAll => self.item_list.toggle_all(),
            ToggleIn => {
                self.item_list.toggle();
                self.item_list.move_cursor_by(1);
                return Ok(vec![Event::RunPreview]);
            }
            ToggleInteractive => todo!(),
            ToggleOut => {
                self.item_list.toggle();
                self.item_list.move_cursor_by(-1);
                return Ok(vec![Event::RunPreview]);
            }
            TogglePreview => todo!(),
            TogglePreviewWrap => todo!(),
            ToggleSort => todo!(),
            UnixLineDiscard => todo!(),
            UnixWordRubout => {
                self.input.delete_backward_word();
                return Ok(vec![Event::RunPreview]);
            }
            Up(n) => {
                self.item_list.move_cursor_by(*n);
                return Ok(vec![Event::RunPreview]);
            }
            Yank => {
                let contents = Cow::Owned(self.input.clone());
                self.yank(contents);
            }
        }
        return Ok(Vec::default());
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

    fn restart_matcher(&mut self, force: bool) {
        let matcher_stopped = self.matcher_control.stopped();
        if force || (matcher_stopped && self.item_pool.num_not_taken() == 0) {
            self.matcher_control.kill();
            let tx = self.item_list.tx.clone();
            self.item_pool.reset();
            self.matcher_control = self.matcher.run(&self.input, self.item_pool.clone(), move |matches| {
                debug!("Got results from matcher, sending to item list...");
                let _ = tx.send(matches.lock().clone());
            });
        }
        if self.should_trigger_matcher {
            self.should_trigger_matcher = false;
        }
    }

    fn yank(&mut self, contents: Cow<'a, str>) {
        self.yank_register = contents;
    }
}
