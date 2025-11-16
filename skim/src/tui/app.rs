use std::borrow::Cow;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::Arc;

use crate::item::{ItemPool, MatchedItem, RankBuilder};
use crate::matcher::{Matcher, MatcherControl};
use crate::prelude::{AndOrEngineFactory, ExactOrFuzzyEngineFactory};
use crate::tui::options::TuiLayout;
use crate::tui::statusline::InfoDisplay;
use crate::tui::widget::SkimWidget;
use crate::util::{self, printf};
use crate::{ItemPreview, PreviewContext, SkimItem, SkimOptions};

use super::Event;
use super::event::Action;
use super::header::Header;
use super::item_list::ItemList;
use super::statusline::StatusLine;
use color_eyre::eyre::{Result, bail};
use crossbeam::channel::{Receiver, Sender, unbounded};
use crossterm::event::{KeyEvent, KeyModifiers};
use defer_drop::DeferDrop;
use input::Input;
use preview::Preview;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode::Char;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
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
    pub yank_register: Cow<'a, str>,
    pub item_rx: Receiver<Arc<dyn SkimItem>>,
    pub item_tx: Sender<Arc<dyn SkimItem>>,

    pub input: Input,
    pub preview: Preview<'a>,
    pub header: Header,
    pub status: StatusLine,
    pub item_list: ItemList,
    pub theme: Arc<crate::theme::ColorTheme>,

    pub reader_timer: std::time::Instant,
    pub matcher_timer: std::time::Instant,

    // spinner visibility state for debounce/hysteresis
    pub spinner_visible: bool,
    pub spinner_last_change: std::time::Instant,

    // track when items were just updated to avoid unnecessary status updates
    pub items_just_updated: bool,

    // query history navigation
    pub query_history: Vec<String>,
    pub history_index: Option<usize>,
    pub saved_input: String,

    // command history navigation (for interactive mode)
    pub cmd_history: Vec<String>,
    pub cmd_history_index: Option<usize>,
    pub saved_cmd_input: String,

    pub options: SkimOptions,
    pub input_border: bool,
    pub cmd: String,
}

impl Widget for &mut App<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let status_area;
        let input_area;
        let input_len = (self.input.len() + 2 + self.options.prompt.chars().count()) as u16;
        let remaining_height = 1
            + (self.options.header.as_ref().and(Some(1)).or(Some(0))).unwrap()
            + (self.options.info == InfoDisplay::Default)
                .then_some(1)
                .or(Some(0))
                .unwrap();

        // Determine if preview should be split from the root area (for left/right) or from list area (for up/down)
        let preview_visible = self.options.preview.is_some() && !self.options.preview_window.hidden;

        // Split preview from root area if it's on left/right
        let (work_area, preview_area_opt) = if preview_visible {
            let size = match self.options.preview_window.size {
                super::Size::Fixed(n) => Constraint::Length(n),
                super::Size::Percent(n) => Constraint::Percentage(n),
            };
            match self.options.preview_window.direction {
                super::Direction::Left => {
                    let areas: [_; 2] = Layout::new(Direction::Horizontal, [size, Constraint::Fill(1)]).areas(area);
                    (areas[1], Some(areas[0]))
                }
                super::Direction::Right => {
                    let areas: [_; 2] = Layout::new(Direction::Horizontal, [Constraint::Fill(1), size]).areas(area);
                    (areas[0], Some(areas[1]))
                }
                super::Direction::Up => {
                    let areas: [_; 2] = Layout::new(Direction::Vertical, [size, Constraint::Fill(1)]).areas(area);
                    (areas[1], Some(areas[0]))
                }
                super::Direction::Down => {
                    let areas: [_; 2] = Layout::new(Direction::Vertical, [Constraint::Fill(1), size]).areas(area);
                    (areas[0], Some(areas[1]))
                }
            }
        } else {
            (area, None)
        };

        let [mut list_area, mut remaining_area] = match self.options.layout {
            TuiLayout::Default | TuiLayout::ReverseList => {
                Layout::vertical([Constraint::Fill(1), Constraint::Length(remaining_height)]).areas(work_area)
            }
            TuiLayout::Reverse => {
                let mut layout =
                    Layout::vertical([Constraint::Length(remaining_height), Constraint::Fill(1)]).areas(work_area);
                layout.reverse();
                layout
            }
        };
        if self.options.header.is_some() {
            let header_area;
            [header_area, remaining_area] = match self.options.layout {
                TuiLayout::Default | TuiLayout::ReverseList => {
                    Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(remaining_area)
                }
                TuiLayout::Reverse => {
                    let mut a = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(remaining_area);
                    a.reverse();
                    a
                }
            };
            self.header.render(header_area, buf);
        }
        if self.options.info == InfoDisplay::Hidden {
            input_area = remaining_area;
        } else {
            match self.options.info {
                InfoDisplay::Default => {
                    let areas: [_; 2] =
                        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(remaining_area);
                    if self.options.layout == TuiLayout::Reverse {
                        input_area = areas[0];
                        status_area = areas[1];
                    } else {
                        status_area = areas[0];
                        input_area = areas[1];
                    }
                }
                InfoDisplay::Inline => {
                    [input_area, status_area] =
                        Layout::horizontal([Constraint::Length(input_len), Constraint::Fill(1)]).areas(remaining_area);
                }
                InfoDisplay::Hidden => {
                    unreachable!()
                }
            };
            // compute spinner debounce/hysteresis and set status.show_spinner before rendering it
            {
                // matcher debounce (ms) and hide grace (ms) — keep in sync with statusline settings
                // Increased values to reduce blinking when matcher toggles frequently.
                const MATCHER_DEBOUNCE_MS: u128 = 300;
                const HIDE_GRACE_MS: u128 = 600;
                let matcher_running = !self.matcher_control.stopped();
                let time_since_match = self.matcher_timer.elapsed();
                let time_since_read = self.reader_timer.elapsed();

                let matcher_ready = matcher_running && time_since_match.as_millis() > MATCHER_DEBOUNCE_MS;
                let reader_ready = self.item_pool.num_not_taken() != 0 && time_since_read.as_millis() > 50;

                let desired = matcher_ready || reader_ready;
                let now = std::time::Instant::now();

                if desired && !self.spinner_visible {
                    // turn on immediately
                    self.spinner_visible = true;
                    self.spinner_last_change = now;
                } else if !desired && self.spinner_visible {
                    // only hide after grace period
                    if now.duration_since(self.spinner_last_change).as_millis() >= HIDE_GRACE_MS {
                        self.spinner_visible = false;
                        self.spinner_last_change = now;
                    }
                }

                // propagate to status for rendering
                self.status.show_spinner = self.spinner_visible;
            }

            self.status.render(status_area, buf);
        }
        self.input.border = self.input_border;
        self.input.render(input_area, buf);

        // Render preview if enabled
        if let Some(preview_area) = preview_area_opt {
            // Preview was already split at the root level (left/right)
            self.preview.render(preview_area, buf);
        } else if self.options.preview.is_some() && !self.options.preview_window.hidden {
            // Preview needs to be split from list area (up/down)
            let direction = Direction::Vertical;
            let size = match self.options.preview_window.size {
                super::Size::Fixed(n) => Constraint::Length(n),
                super::Size::Percent(n) => Constraint::Percentage(n),
            };
            let preview_area = match self.options.preview_window.direction {
                super::Direction::Down => {
                    let areas: [_; 2] = Layout::new(direction, [size, Constraint::Fill(1)]).areas(list_area);
                    list_area = areas[1];
                    areas[0]
                }
                super::Direction::Up => {
                    let areas: [_; 2] = Layout::new(direction, [Constraint::Fill(1), size]).areas(list_area);
                    list_area = areas[0];
                    areas[1]
                }
                _ => unreachable!(),
            };
            self.preview.render(preview_area, buf);
        }
        self.items_just_updated = self.item_list.render(list_area, buf).items_updated;

        self.cursor_pos = (input_area.x + self.input.cursor_pos(), input_area.y);
    }
}

impl Default for App<'_> {
    fn default() -> Self {
        let (item_tx, item_rx) = unbounded();
        let theme = Arc::new(crate::theme::ColorTheme::default());
        Self {
            input: Input::default(),
            preview: {
                let mut preview = Preview::default();
                preview.theme = theme.clone();
                preview
            },
            header: Header::default().theme(theme.clone()),
            status: {
                let mut s = StatusLine::default();
                s.theme = theme.clone();
                s.multi_selection = SkimOptions::default().multi;
                s
            },
            item_pool: Arc::default(),
            item_list: {
                let mut il = ItemList::default();
                il.theme = theme.clone();
                il.multi_select = SkimOptions::default().multi;
                il
            },
            theme,
            item_rx,
            item_tx,
            should_quit: false,
            cursor_pos: (0, 0),
            matcher: Matcher::builder(Rc::new(ExactOrFuzzyEngineFactory::builder().build()))
                .case(crate::CaseMatching::default())
                .build(),
            yank_register: Cow::default(),
            should_trigger_matcher: false,
            matcher_control: MatcherControl::default(),
            reader_timer: std::time::Instant::now(),
            matcher_timer: std::time::Instant::now(),
            // spinner initial state
            spinner_visible: false,
            spinner_last_change: std::time::Instant::now(),
            items_just_updated: false,
            query_history: Vec::new(),
            history_index: None,
            saved_input: String::new(),
            cmd_history: Vec::new(),
            cmd_history_index: None,
            saved_cmd_input: String::new(),
            options: SkimOptions::default(),
            input_border: false,
            cmd: String::new(),
        }
    }
}

impl<'a> App<'a> {
    pub fn from_options(options: SkimOptions, theme: Arc<crate::theme::ColorTheme>, cmd: String) -> Self {
        let (item_tx, item_rx) = unbounded();
        let mut input = Input::from_options(&options, theme.clone());

        // In interactive mode, use cmd_prompt instead of regular prompt
        if options.interactive {
            input.prompt = options.cmd_prompt.clone();
            // In interactive mode, use cmd_query if provided
            if let Some(ref cmd_query) = options.cmd_query {
                input.value = cmd_query.clone();
                input.cursor_pos = cmd_query.len() as u16;
            }
        }

        // Create RankBuilder from tiebreak options
        let rank_builder = Arc::new(RankBuilder::new(options.tiebreak.clone()));

        Self {
            input,
            preview: Preview::from_options(&options, theme.clone()),
            header: Header::from_options(&options, theme.clone()),
            status: StatusLine::from_options(&options, theme.clone()),
            item_pool: Arc::default(),
            item_list: ItemList::from_options(&options, theme.clone()),
            theme,
            item_rx,
            item_tx,
            should_quit: false,
            cursor_pos: (0, 0),
            matcher: Matcher::builder(Rc::new(AndOrEngineFactory::new(
                ExactOrFuzzyEngineFactory::builder()
                    .rank_builder(rank_builder)
                    .exact_mode(options.exact)
                    .build(),
            )))
            .case(options.case)
            .build(),
            yank_register: Cow::default(),
            should_trigger_matcher: false,
            matcher_control: MatcherControl::default(),
            reader_timer: std::time::Instant::now(),
            matcher_timer: std::time::Instant::now(),
            // spinner initial state
            spinner_visible: false,
            spinner_last_change: std::time::Instant::now(),
            items_just_updated: false,
            query_history: options.query_history.clone(),
            history_index: None,
            saved_input: String::new(),
            cmd_history: options.cmd_history.clone(),
            cmd_history_index: None,
            saved_cmd_input: String::new(),
            options,
            input_border: false,
            cmd,
        }
    }
}

impl<'a> App<'a> {
    /// Calculate preview offset from offset expression (e.g., "+123", "+{2}", "+{2}-2")
    fn calculate_preview_offset(&self, offset_expr: &str) -> u16 {
        // Remove the leading '+'
        let expr = offset_expr.trim_start_matches('+');

        // Substitute field placeholders using printf
        let substituted = printf(
            expr.to_string(),
            &self.options.delimiter,
            self.item_list.selection.iter().map(|m| m.item.clone()),
            self.item_list.selected(),
            &self.input.value,
            &self.input.value,
        );

        // Evaluate the expression (handle simple arithmetic like "321-2")
        if let Some((left, right)) = substituted.split_once('-') {
            let left_val = left.trim_matches(|x: char| !x.is_numeric()).parse::<u16>().unwrap_or(0);
            let right_val = right
                .trim_matches(|x: char| !x.is_numeric())
                .parse::<u16>()
                .unwrap_or(0);
            left_val.saturating_sub(right_val)
        } else if let Some((left, right)) = substituted.split_once('+') {
            let left_val = left.trim_matches(|x: char| !x.is_numeric()).parse::<u16>().unwrap_or(0);
            let right_val = right
                .trim_matches(|x: char| !x.is_numeric())
                .parse::<u16>()
                .unwrap_or(0);
            left_val.saturating_add(right_val)
        } else {
            substituted
                .trim_matches(|x: char| !x.is_numeric())
                .parse::<u16>()
                .unwrap_or(0)
        }
    }

    /// Call after items are added or filtered (e.g., Event::NewItem, matcher completes)
    fn on_items_updated(&mut self) {
        self.status.total = self.item_pool.len();
        self.status.matched = self.item_list.count();
        self.status.processed = self.matcher_control.get_num_processed();
        // keep multi-selection flag synced with options
        self.status.multi_selection = self.options.multi;
        // reading/time updates should be performed by whoever owns reader timers
    }

    /// Call after selection changes (e.g., selection actions, Event::Key)
    fn on_selection_changed(&mut self) {
        self.status.selected = self.item_list.selection.len();
        self.status.current_item_idx = self.item_list.current;
        // ensure multi-selection display reflects current options
        self.status.multi_selection = self.options.multi;
    }

    /// Call when matcher state changes (start/stop)
    fn on_matcher_state_changed(&mut self) {
        self.status.matcher_running = !self.matcher_control.stopped();
        // set matcher mode string based on current options (e.g., regex mode)
        self.status.matcher_mode = if self.options.regex {
            "RE".to_string()
        } else {
            String::new()
        };
    }

    pub fn handle_event(&mut self, tui: &mut tui::Tui, event: &Event) -> Result<()> {
        let prev_item = self.item_list.selected();
        match event {
            Event::Render => {
                // update status timing fields before rendering so spinner/indicators are current
                self.status.time_since_read = self.reader_timer.elapsed();
                self.status.time_since_match = self.matcher_timer.elapsed();
                // reading state: true if there are items not yet taken by matcher
                self.status.reading = self.item_pool.num_not_taken() != 0;
                // matcher_running from control
                self.status.matcher_running = !self.matcher_control.stopped();
                tui.get_frame();
                tui.draw(|f| {
                    f.render_widget(&mut *self, f.area());
                    f.set_cursor_position(self.cursor_pos);
                })?;
                // Update status only if item list actually received new items
                if self.items_just_updated {
                    self.on_items_updated();
                    self.on_selection_changed();
                    self.items_just_updated = false;
                }
            }
            Event::Heartbeat => {
                self.should_trigger_matcher = true;
                self.on_matcher_state_changed();
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
                            ItemPreview::CommandWithPos(cmd, _preview_position) => {
                                // TODO: Implement preview with position
                                self.preview.run(
                                    tui,
                                    &printf(
                                        cmd.to_string(),
                                        &self.options.delimiter,
                                        self.item_list.selection.iter().map(|m| m.item.clone()),
                                        self.item_list.selected(),
                                        &self.input,
                                        &self.input,
                                    ),
                                )
                            }
                            ItemPreview::TextWithPos(t, _preview_position) => {
                                // TODO: Implement text preview with position
                                self.preview.content(&t.bytes().collect())?
                            }
                            ItemPreview::AnsiWithPos(t, _preview_position) => {
                                // TODO: Implement ANSI text preview with position
                                self.preview.content(&t.bytes().collect())?
                            }
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
                        self.on_items_updated();
                        self.on_selection_changed();
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
                // Apply preview offset if configured
                if let Some(offset_expr) = &self.options.preview_window.offset {
                    let offset = self.calculate_preview_offset(offset_expr);
                    self.preview.set_offset(offset);
                }
            }
            Event::Error(msg) => {
                tui.exit(1)?;
                bail!(msg.to_owned());
            }
            Event::Action(act) => {
                for evt in self.handle_action(act)? {
                    tui.event_tx.send(evt)?;
                }
                self.on_selection_changed();
                self.on_matcher_state_changed();
            }
            Event::NewItem(item) => {
                self.item_pool.append(vec![item.clone()]);
                self.restart_matcher(false);
                self.on_items_updated();
                self.on_selection_changed();
                self.on_matcher_state_changed();
                trace!("Got new item, len {}", self.item_pool.len());
            }
            Event::Key(key) => {
                for evt in self.handle_key(key) {
                    tui.event_tx.send(evt)?;
                }
                self.on_selection_changed();
            }
            Event::Redraw => {
                tui.clear()?;
            }
            _ => (),
        };
        let new_item = self.item_list.selected();
        if let Some(new) = new_item {
            if let Some(prev) = prev_item {
                if prev.text() != new.text() {
                    self.on_item_changed(tui)?;
                    self.on_selection_changed();
                }
            } else {
                self.on_item_changed(tui)?;
                self.on_selection_changed();
            }
        }
        Ok(())
    }
    pub fn handle_items(&mut self, items: Vec<Arc<dyn SkimItem>>) {
        self.item_pool.append(items);
        self.restart_matcher(false);
        trace!("Got new items, len {}", self.item_pool.len());
        // mark reader activity and reset reader timer
        self.reader_timer = std::time::Instant::now();
        self.status.reading = true;
        // update status so the statusline reflects the new pool state immediately
        self.on_items_updated();
        self.on_selection_changed();
        self.on_matcher_state_changed();
    }
    pub fn on_item_changed(&mut self, tui: &mut crate::tui::Tui) -> Result<()> {
        tui.event_tx.send(Event::RunPreview)?;

        Ok(())
    }
    fn handle_key(&mut self, key: &KeyEvent) -> Vec<Event> {
        debug!("key event: {:?}", key);
        let binds = &self.options.bind;

        let act = binds.get(key);
        if act.is_some() {
            debug!("{act:?}");
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
                self.should_quit = true;
            }
            Accept(_) => {
                self.should_quit = true;
            }
            AddChar(c) => {
                self.input.insert(*c);
                // In interactive mode with --cmd, execute the command with {} substitution
                if self.options.interactive && self.options.cmd.is_some() {
                    let expanded_cmd = self.expand_cmd(&self.cmd);
                    return Ok(vec![Event::Reload(expanded_cmd)]);
                }
                self.restart_matcher(true);
                return Ok(vec![Event::RunPreview]);
            }
            AppendAndSelect => {
                let value = self.input.value.clone();
                let item: Arc<dyn SkimItem> = Arc::new(value);
                self.item_pool.append(vec![item.clone()]);
                self.item_list.append(&mut vec![MatchedItem {
                    item,
                    rank: [0, 0, 0, 0, 0],
                    matched_range: None,
                }]);
                self.item_list.select_row(self.item_list.items.len() - 1);
                self.restart_matcher(true);
                return Ok(vec![Event::RunPreview]);
            }
            BackwardChar => {
                self.input.move_cursor(-1);
            }
            BackwardDeleteChar => {
                self.input.delete(-1);
                if self.options.interactive {
                    let expanded_cmd = self.expand_cmd(&self.cmd);
                    return Ok(vec![Event::Reload(expanded_cmd)]);
                }
                self.restart_matcher(true);
                return Ok(vec![Event::RunPreview]);
            }
            BackwardKillWord => {
                let deleted = Cow::Owned(self.input.delete_backward_word());
                self.yank(deleted);
                if self.options.interactive {
                    let expanded_cmd = self.expand_cmd(&self.cmd);
                    return Ok(vec![Event::Reload(expanded_cmd)]);
                }
                self.restart_matcher(true);
                return Ok(vec![Event::RunPreview]);
            }
            BackwardWord => {
                self.input.move_cursor_backward_word();
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
                self.input.delete(0);
                if self.options.interactive {
                    let expanded_cmd = self.expand_cmd(&self.cmd);
                    return Ok(vec![Event::Reload(expanded_cmd)]);
                }
                self.restart_matcher(true);
                return Ok(vec![Event::RunPreview]);
            }
            DeleteCharEOF => {
                self.input.delete(0);
                if self.options.interactive {
                    let expanded_cmd = self.expand_cmd(&self.cmd);
                    return Ok(vec![Event::Reload(expanded_cmd)]);
                }
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
                    TopToBottom => self.item_list.scroll_by(*n as i32),
                    BottomToTop => self.item_list.scroll_by(-(*n as i32)),
                }
                return Ok(vec![Event::RunPreview]);
            }
            EndOfLine => {
                self.input.move_cursor_to(self.input.len() as u16);
            }
            Execute(cmd) => {
                let mut command = Command::new("sh");
                let expanded_cmd = self.expand_cmd(&cmd);
                debug!("execute: {}", expanded_cmd);
                command.args(["-c", &expanded_cmd]);
                let in_raw_mode = crossterm::terminal::is_raw_mode_enabled()?;
                if in_raw_mode {
                    crossterm::terminal::disable_raw_mode()?;
                }
                crossterm::execute!(std::io::stderr(), crossterm::terminal::LeaveAlternateScreen)?;
                let _ = command.spawn().and_then(|mut c| c.wait());
                if in_raw_mode {
                    crossterm::terminal::enable_raw_mode()?;
                }
                crossterm::execute!(std::io::stderr(), crossterm::terminal::EnterAlternateScreen)?;
                return Ok(vec![Event::Redraw]);
            }
            ExecuteSilent(cmd) => {
                let mut command = Command::new("sh");
                let expanded_cmd = self.expand_cmd(&cmd);
                command.args(["-c", &expanded_cmd]);
                command.stdout(Stdio::null());
                command.stderr(Stdio::null());
                let _ = command.spawn();
            }
            ForwardChar => {
                self.input.move_cursor(1);
            }
            ForwardWord => {
                self.input.move_cursor_forward_word();
            }
            IfQueryEmpty(then, otherwise) => {
                let inner = crate::binds::parse_action_chain(then)?;
                if self.input.is_empty() {
                    return Ok(inner.iter().map(|e| Event::Action(e.to_owned())).collect());
                } else {
                    if let Some(o) = otherwise {
                        return Ok(crate::binds::parse_action_chain(&o)?
                            .iter()
                            .map(|e| Event::Action(e.to_owned()))
                            .collect());
                    }
                }
            }
            IfQueryNotEmpty(then, otherwise) => {
                let inner = crate::binds::parse_action_chain(then)?;
                if !self.input.is_empty() {
                    return Ok(inner.iter().map(|e| Event::Action(e.to_owned())).collect());
                } else {
                    if let Some(o) = otherwise {
                        return Ok(crate::binds::parse_action_chain(&o)?
                            .iter()
                            .map(|e| Event::Action(e.to_owned()))
                            .collect());
                    }
                }
            }
            IfNonMatched(then, otherwise) => {
                let inner = crate::binds::parse_action_chain(then)?;
                if self.item_list.items.is_empty() {
                    return Ok(inner.iter().map(|e| Event::Action(e.to_owned())).collect());
                } else {
                    if let Some(o) = otherwise {
                        return Ok(crate::binds::parse_action_chain(&o)?
                            .iter()
                            .map(|e| Event::Action(e.to_owned()))
                            .collect());
                    }
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
                let deleted = Cow::Owned(self.input.delete_forward_word());
                self.yank(deleted);
                if self.options.interactive {
                    let expanded_cmd = self.expand_cmd(&self.cmd);
                    return Ok(vec![Event::Reload(expanded_cmd)]);
                }
                self.restart_matcher(true);
                return Ok(vec![Event::RunPreview]);
            }
            NextHistory => {
                // Use cmd_history in interactive mode, query_history otherwise
                let (history, history_index, saved_input) = if self.options.interactive {
                    (
                        &self.cmd_history,
                        &mut self.cmd_history_index,
                        &mut self.saved_cmd_input,
                    )
                } else {
                    (&self.query_history, &mut self.history_index, &mut self.saved_input)
                };

                if history.is_empty() {
                    return Ok(vec![]);
                }

                match *history_index {
                    None => {
                        // Already at most recent (current input), do nothing
                    }
                    Some(idx) => {
                        if idx + 1 >= history.len() {
                            // Move to most recent (restore saved input)
                            self.input.value = saved_input.clone();
                            self.input.move_cursor_to(self.input.value.len() as u16);
                            *history_index = None;
                            if !self.options.interactive {
                                self.restart_matcher(true);
                            }
                        } else {
                            // Move forward in history (toward more recent)
                            let new_idx = idx + 1;
                            self.input.value = history[new_idx].clone();
                            self.input.move_cursor_to(self.input.value.len() as u16);
                            *history_index = Some(new_idx);
                            if !self.options.interactive {
                                self.restart_matcher(true);
                            }
                        }
                    }
                }

                // In interactive mode, execute the command to update results
                if self.options.interactive {
                    let cmd = self.input.value.clone();
                    return Ok(vec![Event::Reload(cmd)]);
                }
                return Ok(vec![Event::RunPreview]);
            }
            HalfPageDown(n) => {
                let offset = self.item_list.height as i32 / 2;
                self.item_list.scroll_by(offset * n);
                return Ok(vec![Event::RunPreview]);
            }
            HalfPageUp(n) => {
                let offset = self.item_list.height as i32 / 2;
                self.item_list.scroll_by(offset * n);
                return Ok(vec![Event::RunPreview]);
            }
            PageDown(n) => {
                let offset = self.item_list.height as i32;
                self.item_list.scroll_by(offset * n);
                return Ok(vec![Event::RunPreview]);
            }
            PageUp(n) => {
                let offset = self.item_list.height as i32;
                self.item_list.scroll_by(offset * n);
                return Ok(vec![Event::RunPreview]);
            }
            PreviewUp(n) => {
                self.preview.scroll_up(*n as u16);
            }
            PreviewDown(n) => {
                self.preview.scroll_down(*n as u16);
            }
            PreviewLeft(n) => {
                self.preview.scroll_left(*n as u16);
            }
            PreviewRight(n) => {
                self.preview.scroll_right(*n as u16);
            }
            PreviewPageUp(_n) => {
                self.preview.page_up();
            }
            PreviewPageDown(_n) => {
                self.preview.page_down();
            }
            PreviousHistory => {
                // Use cmd_history in interactive mode, query_history otherwise
                let (history, history_index, saved_input) = if self.options.interactive {
                    (
                        &self.cmd_history,
                        &mut self.cmd_history_index,
                        &mut self.saved_cmd_input,
                    )
                } else {
                    (&self.query_history, &mut self.history_index, &mut self.saved_input)
                };

                if history.is_empty() {
                    return Ok(vec![]);
                }

                match *history_index {
                    None => {
                        // Save current input and go to most recent history entry
                        *saved_input = self.input.value.clone();
                        let new_idx = history.len() - 1;
                        self.input.value = history[new_idx].clone();
                        self.input.move_cursor_to(self.input.value.len() as u16);
                        *history_index = Some(new_idx);
                        if !self.options.interactive {
                            self.restart_matcher(true);
                        }
                    }
                    Some(idx) => {
                        if idx > 0 {
                            // Move backward in history (toward older entries)
                            let new_idx = idx - 1;
                            self.input.value = history[new_idx].clone();
                            self.input.move_cursor_to(self.input.value.len() as u16);
                            *history_index = Some(new_idx);
                            if !self.options.interactive {
                                self.restart_matcher(true);
                            }
                        }
                        // else: already at oldest, do nothing
                    }
                }

                // In interactive mode, execute the command to update results
                if self.options.interactive {
                    let cmd = self.input.value.clone();
                    return Ok(vec![Event::Reload(cmd)]);
                }
                return Ok(vec![Event::RunPreview]);
            }
            Redraw => return Ok(vec![Event::Clear]),
            Reload(Some(s)) => {
                self.item_list.clear_selection();
                return Ok(vec![Event::Reload(s.clone())]);
            }
            Reload(None) => {
                self.item_list.clear_selection();
                return Ok(vec![Event::Reload(self.cmd.clone())]);
            }
            RefreshCmd => {
                // TODO: Implement command refresh
            }
            RefreshPreview => {
                return Ok(vec![Event::RunPreview]);
            }
            RestartMatcher => {
                self.restart_matcher(true);
            }
            RotateMode => {
                // TODO: Implement mode rotation
            }
            ScrollLeft(_n) => {
                // TODO: Implement horizontal scrolling left
            }
            ScrollRight(_n) => {
                // TODO: Implement horizontal scrolling right
            }
            SelectAll => self.item_list.select_all(),
            SelectRow(row) => self.item_list.select_row(*row),
            Select => self.item_list.select(),
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
            ToggleInteractive => {
                self.options.interactive = !self.options.interactive;
            }
            ToggleOut => {
                self.item_list.toggle();
                use ratatui::widgets::ListDirection::*;
                match self.item_list.direction {
                    TopToBottom => self.item_list.select_previous(),
                    BottomToTop => self.item_list.select_next(),
                }
                return Ok(vec![Event::RunPreview]);
            }
            TogglePreview => {
                self.options.preview_window.hidden = !self.options.preview_window.hidden;
            }
            TogglePreviewWrap => {
                // TODO: Implement preview wrap toggle
            }
            ToggleSort => {
                // TODO: Implement sort toggle
            }
            UnixLineDiscard => {
                self.input.delete_to_beginning();
                if self.options.interactive {
                    let expanded_cmd = self.expand_cmd(&self.cmd);
                    return Ok(vec![Event::Reload(expanded_cmd)]);
                }
                self.restart_matcher(true);
                return Ok(vec![Event::RunPreview]);
            }
            UnixWordRubout => {
                self.input.delete_backward_to_whitespace();
                if self.options.interactive {
                    let expanded_cmd = self.expand_cmd(&self.cmd);
                    return Ok(vec![Event::Reload(expanded_cmd)]);
                }
                self.restart_matcher(true);
                return Ok(vec![Event::RunPreview]);
            }
            Up(n) => {
                use ratatui::widgets::ListDirection::*;
                match self.item_list.direction {
                    TopToBottom => self.item_list.scroll_by(-(*n as i32)),
                    BottomToTop => self.item_list.scroll_by(*n as i32),
                }
                return Ok(vec![Event::RunPreview]);
            }
            Yank => {
                // Insert from yank register at cursor position
                self.input.insert_str(&self.yank_register);
                if self.options.interactive {
                    let expanded_cmd = self.expand_cmd(&self.cmd);
                    return Ok(vec![Event::Reload(expanded_cmd)]);
                }
                self.restart_matcher(true);
                return Ok(vec![Event::RunPreview]);
            }
        }
        Ok(Vec::default())
    }

    pub fn results(&self) -> Vec<Arc<dyn SkimItem>> {
        if self.options.multi {
            self.item_list
                .selection
                .iter()
                .map(|item| {
                    debug!("res index: {}", item.get_index());
                    item.item.clone()
                })
                .collect()
        } else {
            if let Some(sel) = self.item_list.selected() {
                vec![sel]
            } else {
                vec![]
            }
        }
    }

    pub(crate) fn restart_matcher(&mut self, force: bool) {
        // Check if query meets minimum length requirement
        if let Some(min_length) = self.options.min_query_length {
            let query_to_check = if self.options.interactive {
                &self.input.value
            } else {
                &self.input.value
            };

            if query_to_check.chars().count() < min_length {
                // Query is too short, clear items and don't run matcher
                self.matcher_control.kill();
                self.item_list.items.clear();
                self.item_list.current = 0;
                self.item_list.offset = 0;
                self.status.matcher_running = false;
                if self.should_trigger_matcher {
                    self.should_trigger_matcher = false;
                }
                return;
            }
        }

        let matcher_stopped = self.matcher_control.stopped();
        if force || (matcher_stopped && self.item_pool.num_not_taken() == 0) {
            self.matcher_control.kill();
            let tx = self.item_list.tx.clone();
            self.item_pool.reset();
            // record matcher start time for statusline spinner/progress
            self.matcher_timer = std::time::Instant::now();
            self.status.matcher_running = true;
            // In interactive mode, use empty query so all items are shown
            // The input contains the command to execute, not a filter query
            let query = if self.options.interactive {
                &input::Input::default()
            } else {
                &self.input
            };
            self.matcher_control = self.matcher.run(query, self.item_pool.clone(), move |matches| {
                let m = matches.lock();
                debug!("Got {} results from matcher, sending to item list...", m.len());
                let _ = tx.send(m.clone());
            });
        }
        if self.should_trigger_matcher {
            self.should_trigger_matcher = false;
        }
    }

    fn yank(&mut self, contents: Cow<'a, str>) {
        self.yank_register = contents;
    }

    fn expand_cmd(&self, cmd: &str) -> String {
        if self.options.interactive {
            // In interactive mode, only {} makes sense - expand it with typed input
            cmd.replace("{}", &self.input.value)
        } else {
            util::printf(
                cmd.to_string(),
                &self.options.delimiter,
                self.item_list.items.iter().map(|x| x.item.clone()),
                self.item_list.selected(),
                &self.input.value,
                &self.input.value,
            )
        }
    }
}
