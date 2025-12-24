use std::borrow::Cow;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::Arc;

use crate::item::{ItemPool, MatchedItem, RankBuilder};
use crate::matcher::{Matcher, MatcherControl};
use crate::prelude::{AndOrEngineFactory, ExactOrFuzzyEngineFactory, RegexEngineFactory};
use crate::tui::options::TuiLayout;
use crate::tui::statusline::InfoDisplay;
use crate::tui::widget::SkimWidget;
use crate::util::{self, printf};
use crate::{ItemPreview, MatchEngineFactory, PreviewContext, SkimItem, SkimOptions};

use super::Event;
use super::event::Action;
use super::header::Header;
use super::item_list::ItemList;
use super::statusline::StatusLine;
use color_eyre::eyre::{Result, bail};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use defer_drop::DeferDrop;
use input::Input;
use preview::Preview;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode::Char;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::Widget;

use super::{input, preview};

/// Application state for skim's TUI
pub struct App<'a> {
    /// Pool of items to be filtered
    pub item_pool: Arc<DeferDrop<ItemPool>>,
    /// Whether the application should quit
    pub should_quit: bool,

    /// Current cursor position (x, y)
    pub cursor_pos: (u16, u16),
    /// Control handle for the matcher thread
    pub matcher_control: MatcherControl,
    /// The matcher for filtering items
    pub matcher: Matcher,
    /// Register for yank/paste operations
    pub yank_register: Cow<'a, str>,
    /// Last time the matcher was restarted
    pub last_matcher_restart: std::time::Instant,
    /// Whether a matcher restart is pending
    pub pending_matcher_restart: bool,

    /// Input field widget
    pub input: Input,
    /// Preview pane widget
    pub preview: Preview<'a>,
    /// Header widget
    pub header: Header,
    /// Status line widget
    pub status: StatusLine,
    /// Item list widget
    pub item_list: ItemList,
    /// Color theme
    pub theme: Arc<crate::theme::ColorTheme>,

    /// Timer for tracking reader activity
    pub reader_timer: std::time::Instant,
    /// Timer for tracking matcher activity
    pub matcher_timer: std::time::Instant,

    /// Spinner visibility state for debounce/hysteresis
    pub spinner_visible: bool,
    /// Last time spinner visibility changed
    pub spinner_last_change: std::time::Instant,

    /// Track when items were just updated to avoid unnecessary status updates
    pub items_just_updated: bool,

    /// Query history navigation
    pub query_history: Vec<String>,
    /// Current position in query history
    pub history_index: Option<usize>,
    /// Saved input when navigating history
    pub saved_input: String,

    /// Command history navigation (for interactive mode)
    pub cmd_history: Vec<String>,
    /// Current position in command history
    pub cmd_history_index: Option<usize>,
    /// Saved command input when navigating history
    pub saved_cmd_input: String,

    /// Skim configuration options
    pub options: SkimOptions,
    /// Whether to show a border around the input field
    pub input_border: bool,
    /// The command being executed
    pub cmd: String,
    /// Preview area rectangle for mouse event handling
    pub preview_area: Option<Rect>,
}

impl Widget for &mut App<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let status_area;
        let input_area;
        let input_len = (self.input.len() + 2 + self.options.prompt.chars().count()) as u16;
        let remaining_height = 1
            + (self.options.header.as_ref().and(Some(1)).unwrap_or(0))
            + if self.options.info == InfoDisplay::Default {
                1
            } else {
                0
            };

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
            self.preview_area = Some(preview_area);
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
            self.preview_area = Some(preview_area);
            self.preview.render(preview_area, buf);
        } else {
            self.preview_area = None;
        }
        self.items_just_updated = self.item_list.render(list_area, buf).items_updated;

        self.cursor_pos = (input_area.x + self.input.cursor_pos(), input_area.y);
    }
}

impl Default for App<'_> {
    fn default() -> Self {
        let theme = Arc::new(crate::theme::ColorTheme::default());
        let opts = SkimOptions::default();
        Self {
            input: Input::from_options(&opts, theme.clone()),
            preview: Preview::from_options(&opts, theme.clone()),
            header: Header::from_options(&opts, theme.clone()),
            status: StatusLine::from_options(&opts, theme.clone()),
            item_list: ItemList::from_options(&opts, theme.clone()),
            item_pool: Arc::default(),
            theme,
            should_quit: false,
            cursor_pos: (0, 0),
            matcher: Matcher::builder(Rc::new(ExactOrFuzzyEngineFactory::builder().build()))
                .case(crate::CaseMatching::default())
                .build(),
            yank_register: Cow::default(),
            matcher_control: MatcherControl::default(),
            reader_timer: std::time::Instant::now(),
            matcher_timer: std::time::Instant::now(),
            last_matcher_restart: std::time::Instant::now(),
            pending_matcher_restart: false,
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
            preview_area: None,
        }
    }
}

impl<'a> App<'a> {
    /// Creates a new App from skim options
    pub fn from_options(options: SkimOptions, theme: Arc<crate::theme::ColorTheme>, cmd: String) -> Self {
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

        let engine_factory: Rc<dyn MatchEngineFactory> = if options.regex {
            Rc::new(RegexEngineFactory::builder())
        } else {
            let rank_builder = Arc::new(RankBuilder::new(options.tiebreak.clone()));
            log::debug!("Creating matcher for algo {:?}", options.algorithm);
            let fuzzy_engine_factory = ExactOrFuzzyEngineFactory::builder()
                .fuzzy_algorithm(options.algorithm)
                .exact_mode(options.exact)
                .rank_builder(rank_builder)
                .build();
            Rc::new(AndOrEngineFactory::new(fuzzy_engine_factory))
        };

        // Create RankBuilder from tiebreak options
        let matcher = Matcher::builder(engine_factory).case(options.case).build();

        Self {
            input,
            preview: Preview::from_options(&options, theme.clone()),
            header: Header::from_options(&options, theme.clone()),
            status: StatusLine::from_options(&options, theme.clone()),
            item_pool: Arc::new(DeferDrop::new(ItemPool::from_options(&options))),
            item_list: ItemList::from_options(&options, theme.clone()),
            theme,
            should_quit: false,
            cursor_pos: (0, 0),
            matcher,
            yank_register: Cow::default(),
            matcher_control: MatcherControl::default(),
            reader_timer: std::time::Instant::now(),
            matcher_timer: std::time::Instant::now(),
            last_matcher_restart: std::time::Instant::now(),
            pending_matcher_restart: false,
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
            preview_area: None,
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

    /// Call when query changes (e.g., AddChar, BackwardDeleteChar, etc.)
    fn on_query_changed(&mut self) -> Result<Vec<Event>> {
        // In interactive mode with --cmd, execute the command with {} substitution
        if self.options.interactive && self.options.cmd.is_some() {
            let expanded_cmd = self.expand_cmd(&self.cmd);
            return Ok(vec![Event::Reload(expanded_cmd)]);
        }
        self.restart_matcher_debounced();
        Ok(vec![
            Event::Key(KeyEvent::new(KeyCode::F(255), KeyModifiers::NONE)), // Send F255 which is the change bind
            Event::RunPreview,
        ])
    }

    /// Handles a TUI event and updates application state
    pub fn handle_event(&mut self, tui: &mut super::Tui, event: &Event) -> Result<()> {
        let prev_item = self.item_list.selected();
        match event {
            Event::Render => {
                // Always render to avoid freezing, but the render function itself can optimize
                tui.get_frame();
                tui.draw(|f| {
                    f.render_widget(&mut *self, f.area());
                    f.set_cursor_position(self.cursor_pos);
                })?;
            }
            Event::Heartbeat => {
                // Heartbeat is used for periodic UI updates
                self.on_matcher_state_changed();
                self.status.time_since_read = self.reader_timer.elapsed();
                self.status.time_since_match = self.matcher_timer.elapsed();
                self.status.reading = self.item_pool.num_not_taken() != 0;
                self.status.matcher_running = !self.matcher_control.stopped();
                self.on_items_updated();
            }
            Event::RunPreview => {
                if let Some(preview_opt) = &self.options.preview
                    && let Some(item) = self.item_list.selected()
                {
                    let selection: Vec<_> = self.item_list.selection.iter().map(|i| i.text().into_owned()).collect();
                    let selection_str: Vec<_> = selection.iter().map(|s| s.as_str()).collect();
                    let ctx = PreviewContext {
                        query: &self.input.value,
                        cmd_query: if self.options.interactive {
                            &self.input.value
                        } else {
                            self.options.cmd_query.as_deref().unwrap_or(&self.input.value)
                        },
                        width: self.preview.cols as usize,
                        height: self.preview.rows as usize,
                        current_index: self.item_list.selected().map(|i| i.get_index()).unwrap_or_default(),
                        current_selection: &self
                            .item_list
                            .selected()
                            .map(|i| i.text().into_owned())
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
                        ItemPreview::Text(t) | ItemPreview::AnsiText(t) => self.preview.content(t.bytes().collect())?,
                        ItemPreview::CommandWithPos(cmd, preview_position) => {
                            // Execute command and apply position after content is ready
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
                            );
                            // Apply position offsets
                            let v_scroll = match preview_position.v_scroll {
                                crate::tui::Size::Fixed(n) => n,
                                crate::tui::Size::Percent(p) => (self.preview.rows as u32 * p as u32 / 100) as u16,
                            };
                            let v_offset = match preview_position.v_offset {
                                crate::tui::Size::Fixed(n) => n,
                                crate::tui::Size::Percent(p) => (self.preview.rows as u32 * p as u32 / 100) as u16,
                            };
                            self.preview.scroll_y = v_scroll.saturating_add(v_offset);

                            let h_scroll = match preview_position.h_scroll {
                                crate::tui::Size::Fixed(n) => n,
                                crate::tui::Size::Percent(p) => (self.preview.cols as u32 * p as u32 / 100) as u16,
                            };
                            let h_offset = match preview_position.h_offset {
                                crate::tui::Size::Fixed(n) => n,
                                crate::tui::Size::Percent(p) => (self.preview.cols as u32 * p as u32 / 100) as u16,
                            };
                            self.preview.scroll_x = h_scroll.saturating_add(h_offset);
                        }
                        ItemPreview::TextWithPos(t, preview_position) => self
                            .preview
                            .content_with_position(t.bytes().collect(), preview_position)?,
                        ItemPreview::AnsiWithPos(t, preview_position) => self
                            .preview
                            .content_with_position(t.bytes().collect(), preview_position)?,
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
            Event::Clear => {
                tui.clear()?;
            }
            Event::Quit => {
                tui.exit()?;
                self.should_quit = true;
            }
            Event::Close => {
                tui.exit()?;
                self.should_quit = true;
            }
            Event::PreviewReady(s) => {
                self.preview.content(s.to_owned())?;
                // Apply preview offset if configured
                if let Some(offset_expr) = &self.options.preview_window.offset {
                    let offset = self.calculate_preview_offset(offset_expr);
                    self.preview.set_offset(offset);
                }
            }
            Event::Error(msg) => {
                tui.exit()?;
                bail!(msg.to_owned());
            }
            Event::Action(act) => {
                for evt in self.handle_action(act)? {
                    tui.event_tx.send(evt)?;
                }
                self.on_selection_changed();
                self.on_matcher_state_changed();
            }
            Event::Key(key) => {
                for evt in self.handle_key(key) {
                    tui.event_tx.send(evt)?;
                }
            }
            Event::Redraw => {
                tui.clear()?;
            }
            Event::Mouse(mouse_event) => {
                self.handle_mouse(mouse_event, tui)?;
            }
            _ => (),
        };

        // Check if item changed
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
    /// Handles new items received from the reader
    pub fn handle_items(&mut self, items: Vec<Arc<dyn SkimItem>>) {
        self.item_pool.append(items);
        // Don't restart matcher immediately - use debounced restart instead
        self.pending_matcher_restart = true;
        trace!("Got new items, len {}", self.item_pool.len());
        // mark reader activity and reset reader timer
        self.reader_timer = std::time::Instant::now();
        self.status.reading = true;
        self.items_just_updated = true;
        // Update status to reflect new pool state
        self.on_items_updated();
    }
    /// Called when the selected item changes
    pub fn on_item_changed(&mut self, tui: &mut crate::tui::Tui) -> Result<()> {
        tui.event_tx.send(Event::RunPreview)?;

        Ok(())
    }
    fn handle_key(&mut self, key: &KeyEvent) -> Vec<Event> {
        debug!("key event: {:?}", key);

        if let Some(act) = &self.options.keymap.get(key) {
            debug!("{act:?}");
            return act.iter().map(|a| Event::Action(a.clone())).collect();
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
                return self.on_query_changed();
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
                self.restart_matcher_debounced();
                return Ok(vec![Event::RunPreview]);
            }
            BackwardChar => {
                self.input.move_cursor(-1);
            }
            BackwardDeleteChar => {
                self.input.delete(-1);
                return self.on_query_changed();
            }
            BackwardKillWord => {
                let deleted = Cow::Owned(self.input.delete_backward_word());
                self.yank(deleted);
                return self.on_query_changed();
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
                return self.on_query_changed();
            }
            DeleteCharEOF => {
                self.input.delete(0);
                return self.on_query_changed();
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
                let expanded_cmd = self.expand_cmd(cmd);
                debug!("execute: {}", expanded_cmd);
                command.args(["-c", &expanded_cmd]);
                let in_raw_mode = crossterm::terminal::is_raw_mode_enabled()?;
                if in_raw_mode {
                    crossterm::terminal::disable_raw_mode()?;
                }
                crossterm::execute!(
                    std::io::stderr(),
                    crossterm::terminal::LeaveAlternateScreen,
                    crossterm::event::DisableMouseCapture
                )?;
                let _ = command.spawn().and_then(|mut c| c.wait());
                if in_raw_mode {
                    crossterm::terminal::enable_raw_mode()?;
                }
                crossterm::execute!(
                    std::io::stderr(),
                    crossterm::terminal::EnterAlternateScreen,
                    crossterm::event::EnableMouseCapture
                )?;
                return Ok(vec![Event::Redraw]);
            }
            ExecuteSilent(cmd) => {
                let mut command = Command::new("sh");
                let expanded_cmd = self.expand_cmd(cmd);
                command.args(["-c", &expanded_cmd]);
                command.stdout(Stdio::null());
                command.stderr(Stdio::null());
                let _ = command.spawn();
            }
            First | Top => {
                // Jump to first item (considering reserved items)
                self.item_list.jump_to_first();
                return Ok(vec![Event::RunPreview]);
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
                } else if let Some(o) = otherwise {
                    return Ok(crate::binds::parse_action_chain(o)?
                        .iter()
                        .map(|e| Event::Action(e.to_owned()))
                        .collect());
                }
            }
            IfQueryNotEmpty(then, otherwise) => {
                let inner = crate::binds::parse_action_chain(then)?;
                if !self.input.is_empty() {
                    return Ok(inner.iter().map(|e| Event::Action(e.to_owned())).collect());
                } else if let Some(o) = otherwise {
                    return Ok(crate::binds::parse_action_chain(o)?
                        .iter()
                        .map(|e| Event::Action(e.to_owned()))
                        .collect());
                }
            }
            IfNonMatched(then, otherwise) => {
                let inner = crate::binds::parse_action_chain(then)?;
                if self.item_list.items.is_empty() {
                    return Ok(inner.iter().map(|e| Event::Action(e.to_owned())).collect());
                } else if let Some(o) = otherwise {
                    return Ok(crate::binds::parse_action_chain(o)?
                        .iter()
                        .map(|e| Event::Action(e.to_owned()))
                        .collect());
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
                return self.on_query_changed();
            }
            Last => {
                // Jump to last item
                self.item_list.jump_to_last();
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
                        } else {
                            // Move forward in history (toward more recent)
                            let new_idx = idx + 1;
                            self.input.value = history[new_idx].clone();
                            self.input.move_cursor_to(self.input.value.len() as u16);
                            *history_index = Some(new_idx);
                        }
                    }
                }

                return self.on_query_changed();
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
                    }
                    Some(idx) => {
                        if idx > 0 {
                            // Move backward in history (toward older entries)
                            let new_idx = idx - 1;
                            self.input.value = history[new_idx].clone();
                            self.input.move_cursor_to(self.input.value.len() as u16);
                            *history_index = Some(new_idx);
                        }
                        // else: already at oldest, do nothing
                    }
                }

                return self.on_query_changed();
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
                // Refresh the command (reload in interactive mode)
                if self.options.interactive {
                    let expanded_cmd = self.expand_cmd(&self.cmd);
                    return Ok(vec![Event::Reload(expanded_cmd)]);
                }
            }
            RefreshPreview => {
                return Ok(vec![Event::RunPreview]);
            }
            RestartMatcher => {
                self.restart_matcher(true);
            }
            RotateMode => {
                // Cycle through modes: fuzzy -> exact -> regex -> fuzzy
                if self.options.regex {
                    // regex -> fuzzy
                    self.options.regex = false;
                    self.options.exact = false;
                } else if self.options.exact {
                    // exact -> regex
                    self.options.exact = false;
                    self.options.regex = true;
                } else {
                    // fuzzy -> exact
                    self.options.exact = true;
                }
                self.restart_matcher(true);
            }
            ScrollLeft(n) => {
                self.item_list.manual_hscroll -= *n;
            }
            ScrollRight(n) => {
                self.item_list.manual_hscroll = self.item_list.manual_hscroll.saturating_add(*n);
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
                self.preview.wrap = !self.preview.wrap;
            }
            ToggleSort => {
                self.options.no_sort = !self.options.no_sort;
                self.restart_matcher(true);
            }
            UnixLineDiscard => {
                self.input.delete_to_beginning();
                return self.on_query_changed();
            }
            UnixWordRubout => {
                self.input.delete_backward_to_whitespace();
                return self.on_query_changed();
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
                return self.on_query_changed();
            }
        }
        Ok(Vec::default())
    }

    /// Returns the selected items as results
    pub fn results(&self) -> Vec<Arc<dyn SkimItem>> {
        if self.options.multi && !self.item_list.selection.is_empty() {
            self.item_list
                .selection
                .iter()
                .map(|item| {
                    debug!("res index: {}", item.get_index());
                    item.item.clone()
                })
                .collect()
        } else if let Some(sel) = self.item_list.selected() {
            vec![sel]
        } else {
            vec![]
        }
    }

    pub(crate) fn restart_matcher(&mut self, force: bool) {
        // Check if query meets minimum length requirement
        if let Some(min_length) = self.options.min_query_length {
            let query_to_check = &self.input.value;

            if query_to_check.chars().count() < min_length {
                // Query is too short, clear items and don't run matcher
                self.matcher_control.kill();
                self.item_list.items.clear();
                self.item_list.current = 0;
                self.item_list.offset = 0;
                self.status.matcher_running = false;
                return;
            }
        }

        let matcher_stopped = self.matcher_control.stopped();
        if force || self.pending_matcher_restart || (matcher_stopped && self.item_pool.num_not_taken() > 0) {
            // Reset debounce timer on any restart to prevent interference
            self.last_matcher_restart = std::time::Instant::now();
            self.pending_matcher_restart = false;
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
            let item_pool = self.item_pool.clone();
            self.matcher_control = self.matcher.run(query, item_pool.clone(), move |matches| {
                let m = matches.lock();
                debug!("Got {} results from matcher, sending to item list...", m.len());

                // Prepend reserved header items to the matched results
                let reserved_items = item_pool.reserved();
                let mut all_items = Vec::with_capacity(reserved_items.len() + m.len());

                // Add reserved items as MatchedItems with min rank (always at top, unmatched)
                for item in reserved_items {
                    all_items.push(MatchedItem {
                        item,
                        rank: [i32::MIN, 0, 0, 0, 0],
                        matched_range: None,
                    });
                }

                // Add matched items
                all_items.extend_from_slice(&m);

                let _ = tx.send(all_items);
            });
        }
    }

    fn yank(&mut self, contents: Cow<'a, str>) {
        self.yank_register = contents;
    }

    /// Expand placeholders in a command string with current app state.
    /// Replaces {}, {q}, {cq}, {n}, {+}, {+n}, and field patterns.
    pub fn expand_cmd(&self, cmd: &str) -> String {
        util::printf(
            cmd.to_string(),
            &self.options.delimiter,
            self.item_list.items.iter().map(|x| x.item.clone()),
            self.item_list.selected(),
            &self.input.value,
            &self.input.value,
        )
    }

    /// Restart matcher with debouncing to avoid excessive restarts during rapid typing
    fn restart_matcher_debounced(&mut self) {
        const DEBOUNCE_MS: u64 = 50;
        let now = std::time::Instant::now();

        // If enough time has passed since last restart, restart immediately
        if now.duration_since(self.last_matcher_restart).as_millis() > DEBOUNCE_MS as u128 {
            self.restart_matcher(true);
        } else {
            self.pending_matcher_restart = true;
        }
    }

    /// Handle mouse events
    fn handle_mouse(&mut self, mouse_event: &MouseEvent, tui: &mut super::Tui) -> Result<()> {
        let mouse_pos = ratatui::layout::Position {
            x: mouse_event.column,
            y: mouse_event.row,
        };

        match mouse_event.kind {
            MouseEventKind::ScrollUp => {
                // Check if mouse is over preview area
                if let Some(preview_area) = self.preview_area
                    && preview_area.contains(mouse_pos)
                {
                    // Scroll preview up
                    for evt in self.handle_action(&Action::PreviewUp(3))? {
                        tui.event_tx.send(evt)?;
                    }
                    return Ok(());
                }
                // Otherwise scroll item list up
                for evt in self.handle_action(&Action::Up(1))? {
                    tui.event_tx.send(evt)?;
                }
            }
            MouseEventKind::ScrollDown => {
                // Check if mouse is over preview area
                if let Some(preview_area) = self.preview_area
                    && preview_area.contains(mouse_pos)
                {
                    // Scroll preview down
                    for evt in self.handle_action(&Action::PreviewDown(3))? {
                        tui.event_tx.send(evt)?;
                    }
                    return Ok(());
                }
                // Otherwise scroll item list down
                for evt in self.handle_action(&Action::Down(1))? {
                    tui.event_tx.send(evt)?;
                }
            }
            _ => {
                // Ignore other mouse events for now
            }
        }
        Ok(())
    }
}
