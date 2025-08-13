use std::borrow::Cow;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::Arc;

use crate::item::ItemPool;
use crate::matcher::{Matcher, MatcherControl};
use crate::prelude::ExactOrFuzzyEngineFactory;
use crate::util::printf;
use crate::{ItemPreview, PreviewContext, SkimItem};

use super::Event;
use super::event::Action;
use super::header::Header;
use super::item_list::ItemList;
use super::options::TuiOptions;
use super::statusline::StatusLine;
use color_eyre::eyre::{Result, bail};
use color_eyre::owo_colors::OwoColorize;
use crossbeam::channel::{Receiver, Sender, unbounded};
use crossterm::event::{KeyEvent, KeyModifiers};
use defer_drop::DeferDrop;
use input::Input;
use preview::Preview;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode::Char;
use ratatui::layout::{Constraint, Direction, Layout, Rect, Size};
use ratatui::widgets::Widget;
use rayon::str::ParallelString;

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
    pub item_rx: Receiver<Arc<dyn SkimItem>>,
    pub item_tx: Sender<Arc<dyn SkimItem>>,

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
        let [mut list_area, header, status, bottom] = layout.areas(area);
        self.input.render(bottom, buf);
        self.header.render(header, buf);
        self.status.render(status, buf);
        if self.options.preview.is_some() && !self.options.preview_window.hidden {
            let direction = match self.options.preview_window.direction {
                super::Direction::Up | super::Direction::Down => Direction::Vertical,
                super::Direction::Left | super::Direction::Right => Direction::Horizontal,
            };
            let size = match self.options.preview_window.size {
                super::Size::Fixed(n) => Constraint::Length(n),
                super::Size::Percent(n) => Constraint::Percentage(n),
            };
            let preview_area = match self.options.preview_window.direction {
                super::Direction::Down | super::Direction::Left => {
                    let areas: [_; 2] = Layout::new(direction, [size, Constraint::Fill(1)]).areas(list_area);
                    list_area = areas[1];
                    areas[0]
                }
                super::Direction::Up | super::Direction::Right => {
                    let areas: [_; 2] = Layout::new(direction, [Constraint::Fill(1), size]).areas(list_area);
                    list_area = areas[0];
                    areas[1]
                }
            };
            self.preview.render(preview_area, buf);
        }
        self.item_list.render(list_area, buf);

        self.cursor_pos = (bottom.x + self.input.cursor_pos(), bottom.y)
    }
}

impl Default for App<'_> {
    fn default() -> Self {
        let (item_tx, item_rx) = unbounded();
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
        let prev_item = self.item_list.selected();
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
                if let Some(preview_opt) = &self.options.preview {
                    if let Some(item) = self.item_list.selected() {
                        let selection: Vec<_> =
                            self.item_list.selection.iter().map(|i| i.text().into_owned()).collect();
                        let selection_str: Vec<_> = selection.iter().map(|s| s.as_str()).collect();
                        let ctx = PreviewContext {
                            query: &self.input.value,
                            cmd_query: &self.input.value, // TODO handle mode
                            width: self.preview.cols as usize,
                            height: self.preview.rows as usize,
                            current_index: self
                                .item_list
                                .selected()
                                .and_then(|i| Some(i.get_index()))
                                .unwrap_or_default(),
                            current_selection: &self
                                .item_list
                                .selected()
                                .and_then(|i| Some(i.text().into_owned()))
                                .unwrap_or_default(),
                            selected_indices: &self
                                .item_list
                                .selection
                                .iter()
                                .map(|v| v.get_index())
                                .collect::<Vec<_>>(),
                            selections: &selection_str,
                        };
                        let preview = item.preview(ctx);
                        match preview {
                            ItemPreview::Command(cmd) => self.preview.run(
                                tui,
                                &printf(
                                    cmd,
                                    &self.options.delimiter,
                                    self.item_list.selection.iter().map(|m| m.item.clone()),
                                    self.item_list.selected(),
                                    &self.input,
                                    &self.input,
                                ),
                            ),
                            ItemPreview::Text(t) | ItemPreview::AnsiText(t) => {
                                self.preview.content(&t.bytes().collect())?
                            }
                            ItemPreview::CommandWithPos(_, preview_position) => todo!(),
                            ItemPreview::TextWithPos(_, preview_position) => todo!(),
                            ItemPreview::AnsiWithPos(_, preview_position) => todo!(),
                            ItemPreview::Global => {
                                self.preview.run(
                                    tui,
                                    &printf(
                                      preview_opt.to_string(),
                                        &self.options.delimiter,
                                        self.item_list.selection.iter().map(|m| m.item.clone()),
                                        self.item_list.selected(),
                                        &self.input,
                                        &self.input,
                                    ),
                                );
                            }
                        };
                    }
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
        let new_item = self.item_list.selected();
        if let Some(new) = new_item {
          if let Some(prev) = prev_item {
            if prev.text() != new.text() {
              self.on_item_changed(tui)?;
            }
          } else {
            self.on_item_changed(tui)?;
          }
        }
        Ok(())
    }
    pub fn handle_items(&mut self, items: Vec<Arc<dyn SkimItem>>) {
        self.item_pool.append(items);
        self.restart_matcher(false);
        trace!("Got new items, len {}", self.item_pool.len());
    }
    pub fn on_item_changed(&mut self, tui: &mut crate::tui::Tui) -> Result<()>{
      tui.event_tx.send(Event::RunPreview)?;

      Ok(())
    }
    fn handle_key(&mut self, key: &KeyEvent) -> Vec<Event> {
        let act = self.options.keymap.get(key);
        if act.is_some() {
            return act.unwrap().iter().map(|a| Event::Action(a.clone())).collect();
        }
        match key.modifiers {
            KeyModifiers::CONTROL => {
                if let Char('c') = key.code {
                    return vec![Event::Quit];
                }
            }
            KeyModifiers::NONE => {
                if let Char(c) = key.code {
                    return vec![Event::Action(Action::AddChar(c))];
                }
            }
            KeyModifiers::SHIFT => {
                if let Char(c) = key.code {
                    return vec![Event::Action(Action::AddChar(c.to_uppercase().next().unwrap()))];
                }
            }
            _ => (),
        };
        vec![]
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

                if let Some(th) = &self.preview.thread_handle {
                  th.abort();
                }
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
            Down(n) => {
                use ratatui::widgets::ListDirection::*;
                match self.item_list.direction {
                    TopToBottom => self.item_list.scroll_down_by(*n),
                    BottomToTop => self.item_list.scroll_up_by(*n),
                }
                return Ok(vec![Event::RunPreview]);
            }
            EndOfLine => {
                self.input.move_cursor_to(self.input.len() as u16);
            }
            Execute(cmd) => {
                let mut command = Command::new("sh");
                command.args(["-c", cmd]);
                let _ = command.spawn();
            }
            ExecuteSilent(cmd) => {
                let mut command = Command::new("sh");
                command.args(["-c", cmd]);
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
                //let offset = self.item_list.view_range.1.abs_diff(self.item_list.view_range.0) as i32;
                //self.item_list.move_cursor_by(offset * n / 2);
                return Ok(vec![Event::RunPreview]);
            }
            HalfPageUp(n) => {
                //let offset = self.item_list.view_range.1.abs_diff(self.item_list.view_range.0) as i32;
                //self.item_list.move_cursor_by(-offset * n / 2);
                return Ok(vec![Event::RunPreview]);
            }
            PageDown(n) => {
                //let offset = self.item_list.view_range.1.abs_diff(self.item_list.view_range.0) as i32;
                //self.item_list.move_cursor_by(offset * n);
                return Ok(vec![Event::RunPreview]);
            }
            PageUp(n) => {
                //let offset = self.item_list.view_range.1.abs_diff(self.item_list.view_range.0) as i32;
                //self.item_list.move_cursor_by(-offset * n);
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
            RestartMatcher => {
                self.restart_matcher(true);
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
                use ratatui::widgets::ListDirection::*;
                match self.item_list.direction {
                    TopToBottom => self.item_list.select_next(),
                    BottomToTop => self.item_list.select_previous(),
                }
                return Ok(vec![Event::RunPreview]);
            }
            ToggleInteractive => todo!(),
            ToggleOut => {
                self.item_list.toggle();
                use ratatui::widgets::ListDirection::*;
                match self.item_list.direction {
                    TopToBottom => self.item_list.select_previous(),
                    BottomToTop => self.item_list.select_next(),
                }
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
                use ratatui::widgets::ListDirection::*;
                match self.item_list.direction {
                    TopToBottom => self.item_list.scroll_up_by(*n),
                    BottomToTop => self.item_list.scroll_down_by(*n),
                }
                return Ok(vec![Event::RunPreview]);
            }
            Yank => {
                let contents = Cow::Owned(self.input.clone());
                self.yank(contents);
            }
        }
        Ok(Vec::default())
    }

    pub fn results(&self) -> Vec<Arc<dyn SkimItem>> {
        self.item_list
            .selection
            .iter()
            .map(|item| {
                debug!("res index: {}", item.get_index());
                item.item.clone()
            })
            .collect()
    }

    pub(crate) fn restart_matcher(&mut self, force: bool) {
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
