use std::process::Stdio;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::item::{ItemPool, MatchedItem};
use crate::matcher::{Matcher, MatcherControl};
use crate::prelude::ExactOrFuzzyEngineFactory;
use crate::tui::SkimRender;
use crate::tui::input::StatusInfo;
use crate::tui::layout::{AppLayout, LayoutTemplate};
use crate::tui::options::TuiLayout;
use crate::tui::statusline::InfoDisplay;
use crate::tui::widget::SkimWidget;
use crate::{ItemPreview, PreviewContext, SkimItem, SkimOptions};
use crate::{Rank, util};

use super::Event;
use super::Tui;
use super::event::Action;
use super::header::Header;
use super::item_list::ItemList;
use super::{input, preview};
use crate::thread_pool::{self, ThreadPool};
use color_eyre::eyre::{Result, bail};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use input::Input;
use preview::Preview;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode::Char;
use ratatui::layout::Rect;
use ratatui::prelude::Backend;
use ratatui::widgets::Widget;
use std::sync::LazyLock;

static NUM_THREADS: LazyLock<usize> = LazyLock::new(|| {
    std::thread::available_parallelism()
        .ok()
        .map_or_else(|| 0, std::num::NonZero::get)
});

const FRAME_TIME_MS: u128 = 1000 / 30;
const MATCHER_DEBOUNCE_MS: u128 = 200;
const HIDE_GRACE_MS: u128 = 500;

/// Application state for skim's TUI
pub struct App {
    /// Pool of items to be filtered
    pub item_pool: Arc<ItemPool>,
    /// Thread pool used by the matcher (⌊2N/3⌋ threads).
    pub matcher_pool: Arc<ThreadPool>,
    /// Thread pool used by the reader pipeline (⌈N/3⌉ threads).
    pub reader_pool: Arc<ThreadPool>,
    /// Whether the application should quit
    pub should_quit: bool,

    /// Current cursor position (x, y)
    pub cursor_pos: (u16, u16),
    /// Control handle for the matcher thread
    pub matcher_control: MatcherControl,
    /// The matcher for filtering items
    pub matcher: Matcher,
    /// Register for yank/paste operations
    pub yank_register: String,
    /// Last time the matcher was restarted
    pub last_matcher_restart: std::time::Instant,
    /// Whether a matcher restart is pending
    pub pending_matcher_restart: bool,
    /// Whether or not we need a render on the next heartbeat
    pub needs_render: Arc<AtomicBool>,
    /// Time of the last render
    pub last_render_timer: std::time::Instant,

    /// Input field widget
    pub input: Input,
    /// Preview pane widget
    pub preview: Preview,
    /// Header widget
    pub header: Header,
    /// Item list widget
    pub item_list: ItemList,
    /// Color theme
    pub theme: Arc<crate::theme::ColorTheme>,

    /// Timer for tracking matcher activity
    pub matcher_timer: std::time::Instant,

    /// Last time spinner visibility changed
    pub spinner_last_change: std::time::Instant,
    /// Whether to show the spinner (controlled with debouncing)
    pub show_spinner: bool,
    /// Start time for spinner animation (set once at app creation, never reset)
    pub spinner_start: std::time::Instant,

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
    /// The command being executed
    pub cmd: String,
    /// Pre-computed layout template built from options; rebuilt when options change.
    pub layout_template: LayoutTemplate,
    /// The header height used when `layout_template` was last built.
    /// Used to detect when multiline header items arrive and the template needs
    /// to be rebuilt to allocate the correct number of rows.
    last_header_height: u16,
    /// Concrete widget areas for the last rendered frame; updated in `render()`.
    pub layout: AppLayout,
    /// Last time preview was spawned (for debouncing)
    pub last_preview_spawn: std::time::Instant,
    /// Whether a preview run was debounced and needs to be retried
    pub pending_preview_run: bool,
    reader_timer: std::time::Instant,
    items_just_updated: bool,
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut res = SkimRender::default();
        let has_border = self.options.border.is_some();

        // Update header with reserved items (from --header-lines).  When
        // --multiline is active a header-line item may occupy more rows than
        // the initial estimate (which was based purely on item count).  Detect
        // that change and rebuild the layout template so the header area gets
        // the right height before the frame is drawn.
        self.header.set_header_lines(self.item_pool.reserved());
        let current_header_height = self.header.height();
        if current_header_height != self.last_header_height {
            self.last_header_height = current_header_height;
            self.layout_template = LayoutTemplate::from_options(&self.options, current_header_height);
        }
        self.layout = self.layout_template.apply(area);

        if let Some(header_area) = self.layout.header_area {
            res |= self.header.render(header_area, buf);
        }

        if let Some(preview_area) = self.layout.preview_area {
            res |= self.preview.render(preview_area, buf);
        }

        res |= self.item_list.render(self.layout.list_area, buf);

        // Render the input after the item list so that the status shows correct information.
        self.input.status_info = if self.options.info == InfoDisplay::Hidden {
            None
        } else {
            Some(StatusInfo {
                total: self.item_pool.len(),
                matched: self.item_list.count(),
                processed: self.matcher_control.get_num_processed(),
                show_spinner: self.show_spinner,
                matcher_mode: if self.options.regex {
                    "RE".to_string()
                } else {
                    String::new()
                },
                multi_selection: self.options.multi,
                selected: self.item_list.selection.len(),
                current_item_idx: self.item_list.current,
                hscroll_offset: i64::from(self.item_list.manual_hscroll),
                start: Some(self.spinner_start),
            })
        };
        res |= self.input.render(self.layout.input_area, buf);

        // Cursor position needs to account for input border and title.
        self.cursor_pos = (
            self.layout.input_area.x + self.input.cursor_pos() + u16::from(has_border),
            self.layout.input_area.y
                + u16::from(!(self.options.layout == TuiLayout::Reverse && self.options.border.is_none())),
        );
        if res.run_preview {
            self.pending_preview_run = true;
        }
    }
}

impl Default for App {
    fn default() -> Self {
        let theme = Arc::new(crate::theme::ColorTheme::default());
        let opts = SkimOptions::default();
        let header = Header::from_options(&opts, theme.clone());
        let initial_header_height = header.height();
        let layout_template = LayoutTemplate::from_options(&opts, initial_header_height);
        let layout = layout_template.apply(Rect::default());
        let (reader_threads, matcher_threads) = thread_pool::partition_threads(*NUM_THREADS);
        Self {
            input: Input::from_options(&opts, theme.clone()),
            preview: Preview::from_options(&opts, theme.clone()),
            header,
            item_list: ItemList::from_options(&opts, theme.clone()),
            matcher_pool: Arc::new(ThreadPool::new(matcher_threads)),
            reader_pool: Arc::new(ThreadPool::new(reader_threads)),
            item_pool: Arc::default(),
            theme,
            should_quit: false,
            cursor_pos: (0, 0),
            matcher: Matcher::builder(Rc::new(ExactOrFuzzyEngineFactory::builder().build()))
                .case(crate::CaseMatching::default())
                .build(),
            yank_register: String::new(),
            matcher_control: MatcherControl::default(),
            matcher_timer: std::time::Instant::now(),
            last_matcher_restart: std::time::Instant::now(),
            pending_matcher_restart: false,
            needs_render: Arc::new(AtomicBool::new(true)),
            last_render_timer: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(1))
                .unwrap(),
            // spinner initial state
            spinner_last_change: std::time::Instant::now(),
            show_spinner: false,
            spinner_start: std::time::Instant::now(),
            query_history: Vec::new(),
            history_index: None,
            saved_input: String::new(),
            cmd_history: Vec::new(),
            cmd_history_index: None,
            saved_cmd_input: String::new(),
            options: opts,
            cmd: String::new(),
            last_header_height: initial_header_height,
            layout_template,
            layout,
            last_preview_spawn: std::time::Instant::now(),
            pending_preview_run: false,
            reader_timer: std::time::Instant::now(),
            items_just_updated: false,
        }
    }
}

impl App {
    /// Creates a new App from skim options.
    ///
    /// # Panics
    ///
    /// Panics if the thread pool cannot be built (system resource exhaustion).
    #[must_use]
    pub fn from_options(options: SkimOptions, theme: Arc<crate::theme::ColorTheme>, cmd: String) -> Self {
        let header = Header::from_options(&options, theme.clone());
        let initial_header_height = header.height();
        let layout_template = LayoutTemplate::from_options(&options, initial_header_height);
        let layout = layout_template.apply(Rect::default());
        let (reader_threads, matcher_threads) = thread_pool::partition_threads(*NUM_THREADS);
        Self {
            input: Input::from_options(&options, theme.clone()),
            preview: Preview::from_options(&options, theme.clone()),
            header,
            matcher_pool: Arc::new(ThreadPool::new(matcher_threads)),
            reader_pool: Arc::new(ThreadPool::new(reader_threads)),
            item_pool: Arc::new(ItemPool::from_options(&options)),
            item_list: ItemList::from_options(&options, theme.clone()),
            theme,
            should_quit: false,
            cursor_pos: (0, 0),
            matcher: Matcher::from_options(&options),
            yank_register: String::new(),
            matcher_control: MatcherControl::default(),
            reader_timer: std::time::Instant::now(),
            matcher_timer: std::time::Instant::now(),
            last_matcher_restart: std::time::Instant::now(),
            pending_matcher_restart: false,
            needs_render: Arc::new(AtomicBool::new(true)),
            last_render_timer: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(1))
                .unwrap(),
            // spinner initial state
            spinner_last_change: std::time::Instant::now(),
            show_spinner: false,
            spinner_start: std::time::Instant::now(),
            items_just_updated: false,
            query_history: options.query_history.clone(),
            history_index: None,
            saved_input: String::new(),
            cmd_history: options.cmd_history.clone(),
            cmd_history_index: None,
            saved_cmd_input: String::new(),
            options,
            cmd,
            last_header_height: initial_header_height,
            layout_template,
            layout,
            last_preview_spawn: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(1))
                .unwrap(),
            pending_preview_run: false,
        }
    }

    /// Rebuild the layout template for the given terminal dimensions.
    ///
    /// The template encodes all option-derived constraints; `apply` in render
    /// is then a cheap set of rect splits with no option inspection.
    /// Called on `Event::Resize(cols, rows)` and whenever a layout-affecting
    /// option changes (e.g. `TogglePreview`).
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.layout_template = LayoutTemplate::from_options(&self.options, self.header.height());
        self.layout = self.layout_template.apply(Rect::new(0, 0, cols, rows));
    }
}

impl App {
    /// Calculate preview offset from offset expression (e.g., "+123", "+{2}", "+{2}-2")
    fn calculate_preview_offset(&self, offset_expr: &str) -> u16 {
        // Remove the leading '+'
        let expr = offset_expr.trim_start_matches('+');

        // Substitute field placeholders using printf
        let substituted = self.expand_cmd(expr, true);
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

    fn needs_render(&mut self) {
        self.needs_render.store(true, Ordering::Relaxed);
    }

    /// Call after items are added or filtered (e.g., `Event::NewItem`, matcher completes)
    fn on_items_updated(&mut self) {
        self.pending_matcher_restart = true;
        trace!("Got new items, len {}", self.item_pool.len());
        // mark reader activity and reset reader timer
        self.reader_timer = std::time::Instant::now();
        self.items_just_updated = true;
    }

    /// Call after selection changes (e.g., selection actions, `Event::Key`)
    fn on_selection_changed() -> Vec<Event> {
        vec![Event::RunPreview]
    }

    /// Call when query changes (e.g., `AddChar`, `BackwardDeleteChar`, etc.)
    fn on_query_changed(&mut self) -> Vec<Event> {
        // In interactive mode with --cmd, execute the command with {} substitution
        if self.options.interactive && self.options.cmd.is_some() {
            let expanded_cmd = self.expand_cmd(&self.cmd, true);
            return vec![Event::Reload(expanded_cmd)];
        }
        self.restart_matcher_debounced();
        vec![
            Event::Key(KeyEvent::new(KeyCode::F(255), KeyModifiers::NONE)), // Send F255 which is the change bind
            Event::RunPreview,
        ]
    }

    fn update_spinner(&mut self) {
        let matcher_running = !self.matcher_control.stopped();
        let time_since_match = self.matcher_timer.elapsed();
        let reading = self.item_pool.num_not_taken() != 0;

        let should_show_spinner = reading || (matcher_running && time_since_match.as_millis() > MATCHER_DEBOUNCE_MS);

        if should_show_spinner && !self.show_spinner {
            self.toggle_spinner();
        } else if !should_show_spinner && self.show_spinner {
            // Hide spinner only after grace period to avoid flickering
            if self.spinner_last_change.elapsed().as_millis() >= HIDE_GRACE_MS {
                self.toggle_spinner();
            }
        }
        if self.show_spinner {
            self.needs_render.store(true, Ordering::Relaxed);
        }
    }

    fn run_preview<B: Backend>(&mut self, tui: &mut Tui<B>) -> Result<()>
    where
        B::Error: Send + Sync + 'static,
    {
        // Debounce preview spawning to prevent overwhelming the system during rapid scrolling
        const DEBOUNCE_MS: u64 = 50;
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_preview_spawn);

        if elapsed.as_millis() < u128::from(DEBOUNCE_MS) {
            // Mark that we have a pending preview to run after the debounce period
            self.pending_preview_run = true;
            return Ok(());
        }

        self.pending_preview_run = false;
        self.last_preview_spawn = now;

        if let Some(preview_opt) = &self.options.preview
            && let Some(item) = self.item_list.selected()
        {
            let selection: Vec<_> = self.item_list.selection.iter().map(|i| i.text().into_owned()).collect();
            let selection_str: Vec<_> = selection.iter().map(std::string::String::as_str).collect();
            let selected = self.item_list.selected();
            let ctx = PreviewContext {
                query: &self.input.value,
                cmd_query: if self.options.interactive {
                    &self.input.value
                } else {
                    self.options.cmd_query.as_deref().unwrap_or(&self.input.value)
                },
                width: self.preview.cols as usize,
                height: self.preview.rows as usize,
                current_index: selected
                    .as_ref()
                    .map(|i| i.rank.index.unsigned_abs() as usize)
                    .unwrap_or_default(),
                current_selection: &selected.map(|i| i.text().into_owned()).unwrap_or_default(),
                selected_indices: &self
                    .item_list
                    .selection
                    .iter()
                    .map(|v| v.rank.index.unsigned_abs() as usize)
                    .collect::<Vec<_>>(),
                selections: &selection_str,
            };
            let preview = item.preview(ctx);
            let preview_ready = !matches!(
                preview,
                ItemPreview::Global | ItemPreview::Command(_) | ItemPreview::CommandWithPos(_, _)
            );
            match preview {
                ItemPreview::Command(cmd) => self.preview.spawn(tui, &self.expand_cmd(&cmd, true))?,
                ItemPreview::Text(t) | ItemPreview::AnsiText(t) => {
                    self.preview.content(&t.bytes().collect::<Vec<_>>())?;
                }
                ItemPreview::CommandWithPos(cmd, preview_position) => {
                    // Execute command and apply position after content is ready
                    self.preview.spawn(tui, &self.expand_cmd(&cmd, true))?;
                    // Apply position offsets
                    let v_scroll = match preview_position.v_scroll {
                        crate::tui::Size::Fixed(n) => n,
                        crate::tui::Size::Percent(p) => {
                            u16::try_from(u32::from(self.preview.rows) * u32::from(p) / 100).unwrap_or(u16::MAX)
                        }
                    };
                    let v_offset = match preview_position.v_offset {
                        crate::tui::Size::Fixed(n) => n,
                        crate::tui::Size::Percent(p) => {
                            u16::try_from(u32::from(self.preview.rows) * u32::from(p) / 100).unwrap_or(u16::MAX)
                        }
                    };
                    self.preview.scroll_y = v_scroll;
                    self.preview.scroll_down(v_offset);

                    let h_scroll = match preview_position.h_scroll {
                        crate::tui::Size::Fixed(n) => n,
                        crate::tui::Size::Percent(p) => {
                            u16::try_from(u32::from(self.preview.cols) * u32::from(p) / 100).unwrap_or(u16::MAX)
                        }
                    };
                    let h_offset = match preview_position.h_offset {
                        crate::tui::Size::Fixed(n) => n,
                        crate::tui::Size::Percent(p) => {
                            u16::try_from(u32::from(self.preview.cols) * u32::from(p) / 100).unwrap_or(u16::MAX)
                        }
                    };
                    self.preview.scroll_x = h_scroll.saturating_add(h_offset);
                }
                ItemPreview::TextWithPos(t, preview_position) | ItemPreview::AnsiWithPos(t, preview_position) => self
                    .preview
                    .content_with_position(&t.bytes().collect::<Vec<_>>(), preview_position)?,
                ItemPreview::Global => self.preview.spawn(tui, &self.expand_cmd(preview_opt, true))?,
            }
            if preview_ready {
                let _ = tui.event_tx.try_send(Event::PreviewReady);
            }
        } else if let Some(cb) = &self.options.preview_fn {
            let selection: Vec<Arc<dyn SkimItem>>;
            if self.options.multi {
                selection = self.item_list.selection.iter().map(|i| i.item.clone()).collect();
            } else if let Some(sel) = self.item_list.selected() {
                selection = vec![sel.item];
            } else {
                selection = Vec::new();
            }
            self.preview.content(&cb(selection).join("\n").into_bytes())?;
        }
        Ok(())
    }

    /// Handles a TUI event and updates application state
    ///
    /// # Errors
    ///
    /// Returns an error if drawing, sending events, or spawning a preview fails.
    #[allow(clippy::too_many_lines)]
    pub fn handle_event<B: Backend>(&mut self, tui: &mut Tui<B>, event: &Event) -> Result<()>
    where
        B::Error: Send + Sync + 'static,
    {
        match event {
            Event::Render => {
                // Always render to avoid freezing, but the render function itself can optimize
                tui.get_frame();
                tui.draw(|f| {
                    f.render_widget(&mut *self, f.area());
                    f.set_cursor_position(self.cursor_pos);
                })?;
            }
            Event::Heartbeat | Event::Tick => {
                // Heartbeat is used for periodic UI updates
                self.update_spinner();

                if self.pending_matcher_restart {
                    self.restart_matcher(true);
                }
                if self.needs_render.load(Ordering::Relaxed)
                    && self.last_render_timer.elapsed().as_millis() > FRAME_TIME_MS
                {
                    debug!("Triggering render");
                    self.needs_render.store(false, Ordering::Relaxed);
                    self.last_render_timer = std::time::Instant::now();
                    tui.event_tx.try_send(Event::Render)?;
                }

                // Check if a debounced preview run needs to be executed
                if self.pending_preview_run
                    && let Err(e) = self.run_preview(tui)
                {
                    warn!("Heartbeat RunPreview: error {e:?}");
                }
            }
            Event::RunPreview => {
                if let Err(e) = self.run_preview(tui) {
                    warn!("RunPreview: error {e:?}");
                }
            }
            Event::Clear | Event::Redraw => {
                tui.clear()?;
            }
            Event::Quit | Event::Close => {
                tui.exit()?;
                self.should_quit = true;
            }
            Event::PreviewReady => {
                // Apply preview offset if configured
                if let Some(offset_expr) = &self.options.preview_window.offset {
                    let offset = self.calculate_preview_offset(offset_expr);
                    self.preview.set_offset(offset);
                }
                self.needs_render();
            }
            Event::Error(msg) => {
                tui.exit()?;
                bail!(msg.to_owned());
            }
            Event::Action(act) => {
                let events = self.handle_action(act)?;
                for evt in events {
                    tui.event_tx.try_send(evt)?;
                }
                tui.event_tx.try_send(Event::Render)?;
            }
            Event::Key(key) => {
                let events = self.handle_key(key);
                for evt in events {
                    tui.event_tx.try_send(evt)?;
                }
            }
            Event::Paste(text) => {
                // Strip newlines/carriage returns from pasted text so they don't
                // trigger Accept or get inserted as invisible characters.
                let cleaned: String = text.chars().filter(|c| *c != '\n' && *c != '\r').collect();
                if !cleaned.is_empty() {
                    self.input.insert_str(&cleaned);
                    let events = self.on_query_changed();
                    for evt in events {
                        tui.event_tx.try_send(evt)?;
                    }
                }
            }
            Event::Resize(cols, rows) => {
                self.resize(*cols, *rows);
                if let Err(e) = self.run_preview(tui) {
                    warn!("error while rerunnig preview after resize: {e}");
                }
            }
            Event::Mouse(mouse_event) => {
                self.handle_mouse(*mouse_event, tui)?;
            }
            Event::InvalidInput => {
                warn!("Received invalid input");
            }
            Event::ClearItems => {
                self.item_pool.clear();
                self.restart_matcher(true);
            }
            Event::AppendItems(items) => {
                self.item_pool.append(items.to_owned());
                self.restart_matcher(false);
            }
            Event::Reload(_) => {
                unreachable!("Reload is handled by the TUI event loop in lib.rs")
            }
        }

        Ok(())
    }
    /// Handles new items received from the reader
    pub fn handle_items(&mut self, items: Vec<Arc<dyn SkimItem>>) {
        self.item_pool.append(items);
        trace!("Got new items, len {}", self.item_pool.len());
        self.on_items_updated();
    }
    fn handle_key(&mut self, key: &KeyEvent) -> Vec<Event> {
        debug!("key event: {key:?}");

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
        }
        vec![]
    }

    #[allow(clippy::too_many_lines)]
    fn handle_action(&mut self, act: &Action) -> Result<Vec<Event>> {
        use Action::{
            Abort, Accept, AddChar, AppendAndSelect, BackwardChar, BackwardDeleteChar, BackwardDeleteCharEof,
            BackwardKillWord, BackwardWord, BeginningOfLine, Cancel, ClearScreen, Custom, DeleteChar, DeleteCharEof,
            DeselectAll, Down, EndOfLine, Execute, ExecuteSilent, First, ForwardChar, ForwardWord, HalfPageDown,
            HalfPageUp, IfNonMatched, IfQueryEmpty, IfQueryNotEmpty, Ignore, KillLine, KillWord, Last, NextHistory,
            PageDown, PageUp, PreviewDown, PreviewLeft, PreviewPageDown, PreviewPageUp, PreviewRight, PreviewUp,
            PreviousHistory, Redraw, RefreshCmd, RefreshPreview, Reload, RestartMatcher, RotateMode, ScrollLeft,
            ScrollRight, Select, SelectAll, SelectRow, SetHeader, SetPreviewCmd, SetQuery, Toggle, ToggleAll, ToggleIn,
            ToggleInteractive, ToggleOut, TogglePreview, TogglePreviewWrap, ToggleSort, Top, UnixLineDiscard,
            UnixWordRubout, Up, Yank,
        };
        use ratatui::widgets::ListDirection::{BottomToTop, TopToBottom};
        match act {
            Abort | Accept(_) => {
                self.should_quit = true;
            }
            AddChar(c) => {
                self.input.insert(*c);
                return Ok(self.on_query_changed());
            }
            AppendAndSelect => {
                let value = self.input.value.clone();
                let item: Arc<dyn SkimItem> = Arc::new(value);
                let rank = Rank {
                    index: i32::try_from(self.item_pool.len()).unwrap_or(i32::MAX),
                    ..Default::default()
                };
                self.item_pool.append(vec![item.clone()]);
                self.item_list.append(&mut vec![MatchedItem::new(
                    item,
                    rank,
                    None,
                    &self.matcher.rank_builder,
                )]);
                self.item_list.select_row(self.item_list.items.len() - 1);
                self.restart_matcher_debounced();
                return Ok(Self::on_selection_changed());
            }
            BackwardChar => {
                self.input.move_cursor(-1);
            }
            BackwardDeleteChar => {
                if self.input.delete(-1).is_some() {
                    return Ok(self.on_query_changed());
                }
            }
            BackwardDeleteCharEof => {
                if self.input.is_empty() {
                    self.should_quit = true;
                    return Ok(vec![]);
                }
                self.input.delete(-1);
                return Ok(self.on_query_changed());
            }
            BackwardKillWord => {
                let deleted = self.input.delete_backward_word();
                if !deleted.is_empty() {
                    self.yank(deleted);
                    return Ok(self.on_query_changed());
                }
            }
            BackwardWord => {
                self.input.move_cursor_backward_word();
            }
            BeginningOfLine => {
                self.input.move_cursor_to(0);
            }
            Cancel => {
                self.matcher_control.kill();
                self.preview.kill();
            }
            ClearScreen => {
                return Ok(vec![Event::Clear]);
            }
            DeleteChar => {
                if self.input.delete(0).is_some() {
                    return Ok(self.on_query_changed());
                }
            }
            DeleteCharEof => {
                if self.input.is_empty() {
                    self.should_quit = true;
                    return Ok(vec![]);
                } else if self.input.delete(0).is_some() {
                    return Ok(self.on_query_changed());
                }
            }
            DeselectAll => {
                if !self.item_list.selection.is_empty() {
                    self.item_list.selection = Default::default();
                    return Ok(Self::on_selection_changed());
                }
            }
            Down(n) => {
                match self.item_list.direction {
                    TopToBottom => self.item_list.scroll_by(i32::from(*n)),
                    BottomToTop => self.item_list.scroll_by(-i32::from(*n)),
                }
                return Ok(Self::on_selection_changed());
            }
            EndOfLine => {
                self.input.move_to_end();
            }
            Execute(cmd) => {
                let expanded_cmd = self.expand_cmd(cmd, true);
                debug!("execute: {expanded_cmd}");
                let mut command = crate::shell_cmd(&expanded_cmd);
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
                let expanded_cmd = self.expand_cmd(cmd, true);
                debug!("execute-silent: {expanded_cmd}");
                let mut command = crate::shell_cmd(&expanded_cmd);
                command.stdout(Stdio::null()).stderr(Stdio::null());
                let _ = command.spawn();
            }
            First | Top => {
                // Jump to first item (considering reserved items)
                self.item_list.jump_to_first();
                return Ok(Self::on_selection_changed());
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
                let deleted = self.input.split_off(cursor);
                self.yank(deleted);
                return Ok(self.on_query_changed());
            }
            KillWord => {
                let deleted = self.input.delete_forward_word();
                self.yank(deleted);
                return Ok(self.on_query_changed());
            }
            Last => {
                // Jump to last item
                self.item_list.jump_to_last();
                return Ok(Self::on_selection_changed());
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
                            self.input.move_to_end();
                            *history_index = None;
                        } else {
                            // Move forward in history (toward more recent)
                            let new_idx = idx + 1;
                            self.input.value = history[new_idx].clone();
                            self.input.move_to_end();
                            *history_index = Some(new_idx);
                        }
                    }
                }

                return Ok(self.on_query_changed());
            }
            HalfPageDown(n) => {
                let offset = i32::from(self.item_list.height) / 2;
                if self.options.layout == TuiLayout::Default {
                    self.item_list.scroll_by_rows(-offset * n);
                } else {
                    self.item_list.scroll_by_rows(offset * n);
                }
                return Ok(Self::on_selection_changed());
            }
            HalfPageUp(n) => {
                let offset = i32::from(self.item_list.height) / 2;
                if self.options.layout == TuiLayout::Default {
                    self.item_list.scroll_by_rows(offset * n);
                } else {
                    self.item_list.scroll_by_rows(-offset * n);
                }
                return Ok(Self::on_selection_changed());
            }
            PageDown(n) => {
                let offset = i32::from(self.item_list.height);
                if self.options.layout == TuiLayout::Default {
                    self.item_list.scroll_by_rows(-offset * n);
                } else {
                    self.item_list.scroll_by_rows(offset * n);
                }
                return Ok(Self::on_selection_changed());
            }
            PageUp(n) => {
                let offset = i32::from(self.item_list.height);
                if self.options.layout == TuiLayout::Default {
                    self.item_list.scroll_by_rows(offset * n);
                } else {
                    self.item_list.scroll_by_rows(-offset * n);
                }
                return Ok(Self::on_selection_changed());
            }
            PreviewUp(n) => {
                self.preview.scroll_up(u16::try_from(*n).unwrap_or(u16::MAX));
                self.needs_render();
            }
            PreviewDown(n) => {
                self.preview.scroll_down(u16::try_from(*n).unwrap_or(u16::MAX));
                self.needs_render();
            }
            PreviewLeft(n) => {
                self.preview.scroll_left(u16::try_from(*n).unwrap_or(u16::MAX));
                self.needs_render();
            }
            PreviewRight(n) => {
                self.preview.scroll_right(u16::try_from(*n).unwrap_or(u16::MAX));
                self.needs_render();
            }
            PreviewPageUp(_n) => {
                self.preview.page_up();
                self.needs_render();
            }
            PreviewPageDown(_n) => {
                self.preview.page_down();
                self.needs_render();
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
                        saved_input.clone_from(&self.input.value);
                        let new_idx = history.len() - 1;
                        self.input.value = history[new_idx].clone();
                        self.input.move_to_end();
                        *history_index = Some(new_idx);
                    }
                    Some(idx) => {
                        if idx > 0 {
                            // Move backward in history (toward older entries)
                            let new_idx = idx - 1;
                            self.input.value = history[new_idx].clone();
                            self.input.move_to_end();
                            *history_index = Some(new_idx);
                        }
                        // else: already at oldest, do nothing
                    }
                }

                return Ok(self.on_query_changed());
            }
            Redraw => return Ok(vec![Event::Clear]),
            Reload(Some(s)) => {
                self.item_list.clear_selection();
                return Ok(vec![Event::Reload(self.expand_cmd(s, true))]);
            }
            Reload(None) => {
                self.item_list.clear_selection();
                return Ok(vec![Event::Reload(self.cmd.clone())]);
            }
            RefreshCmd => {
                // Refresh the command (reload in interactive mode)
                if self.options.interactive {
                    let expanded_cmd = self.expand_cmd(&self.cmd, true);
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
                self.matcher = Matcher::from_options(&self.options);
                self.restart_matcher(true);
            }
            ScrollLeft(n) => {
                self.item_list.manual_hscroll = self.item_list.manual_hscroll.saturating_sub(*n);
            }
            ScrollRight(n) => {
                self.item_list.manual_hscroll = self.item_list.manual_hscroll.saturating_add(*n);
            }
            SelectAll => {
                self.item_list.select_all();
                return Ok(Self::on_selection_changed());
            }
            SelectRow(row) => {
                self.item_list.select_row(*row);
                return Ok(Self::on_selection_changed());
            }
            Select => {
                self.item_list.select();
                return Ok(Self::on_selection_changed());
            }
            SetHeader(opt_header) => {
                opt_header.clone_into(&mut self.options.header);
                self.header = Header::from_options(&self.options, self.theme.clone());
                // Rebuild the layout template so that the header area is
                // included (or excluded) on the next render.
                self.layout_template = LayoutTemplate::from_options(&self.options, self.header.height());
            }
            SetPreviewCmd(cmd) => {
                self.options.preview = Some(cmd.to_owned());
                return Ok(vec![Event::RunPreview]);
            }
            SetQuery(value) => {
                self.input.value = self.expand_cmd(value, false);
                self.input.move_to_end();
                return Ok(self.on_query_changed());
            }
            Toggle => {
                self.item_list.toggle();
                return Ok(Self::on_selection_changed());
            }
            ToggleAll => {
                self.item_list.toggle_all();
                return Ok(Self::on_selection_changed());
            }
            ToggleIn => {
                self.item_list.toggle();
                match self.item_list.direction {
                    TopToBottom => self.item_list.select_next(),
                    BottomToTop => self.item_list.select_previous(),
                }
                return Ok(Self::on_selection_changed());
            }
            ToggleInteractive => {
                self.options.interactive = !self.options.interactive;
                self.input.switch_mode();
                self.restart_matcher(true);
            }
            ToggleOut => {
                self.item_list.toggle();
                match self.item_list.direction {
                    TopToBottom => self.item_list.select_previous(),
                    BottomToTop => self.item_list.select_next(),
                }
                return Ok(Self::on_selection_changed());
            }
            TogglePreview => {
                self.options.preview_window.hidden = !self.options.preview_window.hidden;
                self.layout_template = LayoutTemplate::from_options(&self.options, self.header.height());
                self.needs_render();
            }
            TogglePreviewWrap => {
                self.preview.wrap = !self.preview.wrap;
                self.needs_render();
            }
            ToggleSort => {
                self.options.no_sort = !self.options.no_sort;
                self.restart_matcher(true);
            }
            UnixLineDiscard => {
                if !self.input.delete_to_beginning().is_empty() {
                    return Ok(self.on_query_changed());
                }
            }
            UnixWordRubout => {
                if !self.input.delete_backward_to_whitespace().is_empty() {
                    return Ok(self.on_query_changed());
                }
            }
            Up(n) => {
                match self.item_list.direction {
                    TopToBottom => self.item_list.scroll_by(-i32::from(*n)),
                    BottomToTop => self.item_list.scroll_by(i32::from(*n)),
                }
                return Ok(Self::on_selection_changed());
            }
            Yank => {
                // Insert from yank register at cursor position
                self.input.insert_str(&self.yank_register);
                return Ok(self.on_query_changed());
            }
            Custom(cb) => {
                return cb.call(self).map_err(|e| color_eyre::eyre::eyre!("{}", e));
            }
        }
        Ok(Vec::default())
    }

    /// Returns the selected items as results
    pub fn results(&mut self) -> Vec<MatchedItem> {
        if self.options.filter.is_some() {
            // In filter mode, drain items to avoid cloning
            self.item_list.items.drain(..).collect()
        } else if self.options.multi && !self.item_list.selection.is_empty() {
            self.item_list.selection.clone().into_iter().collect()
        } else if let Some(sel) = self.item_list.selected() {
            vec![sel]
        } else {
            vec![]
        }
    }

    /// Restart the matcher to process items in the item pool.
    ///
    /// If `force` is true, the matcher will be restarted even if it's currently running.
    /// If `force` is false, the matcher will only be restarted if there are new items
    /// to process or if the previous matcher has completed.
    pub fn restart_matcher(&mut self, force: bool) {
        use crate::tui::item_list::MergeStrategy;
        // Check if query meets minimum length requirement
        if let Some(min_length) = self.options.min_query_length
            && !self.options.disabled
        {
            let query_to_check = &self.input.value;

            if query_to_check.chars().count() < min_length {
                // Query is too short, clear items and don't run matcher
                self.matcher_control.kill();
                self.item_list.items.clear();
                self.item_list.current = 0;
                self.item_list.offset = 0;
                return;
            }
        }

        let matcher_stopped = self.matcher_control.stopped();
        if force || (matcher_stopped && self.item_pool.num_not_taken() > 0) {
            trace!("restarting matcher, force={force}");
            // Reset debounce timer on any restart to prevent interference
            self.last_matcher_restart = std::time::Instant::now();
            self.pending_matcher_restart = false;
            self.matcher_control.kill();
            // record matcher start time for statusline spinner/progress
            self.matcher_timer = std::time::Instant::now();
            // In interactive mode, use empty query so all items are shown
            // The input contains the command to execute, not a filter query
            let query = if self.options.disabled {
                ""
            } else if self.options.interactive {
                &input::Input::default()
            } else {
                &self.input
            };
            let item_pool = self.item_pool.clone();
            let thread_pool = &self.matcher_pool;
            let no_sort = self.options.no_sort;

            if force {
                self.item_pool.reset();
            }

            let merge_strategy = if force {
                MergeStrategy::Replace
            } else if no_sort {
                MergeStrategy::Append
            } else {
                MergeStrategy::SortedMerge
            };

            self.matcher_control = self.matcher.run(
                query,
                &item_pool,
                thread_pool,
                self.item_list.processed_items.clone(),
                merge_strategy,
                no_sort,
                self.needs_render.clone(),
            );
        }
    }

    fn yank(&mut self, contents: String) {
        self.yank_register = contents;
    }

    /// Expand placeholders in a command string with current app state.
    /// Replaces {}, {q}, {cq}, {n}, {+}, {+n}, and field patterns.
    ///
    /// Note: in command mode, the replstr is replaced by the current query
    #[must_use]
    pub fn expand_cmd(&self, cmd: &str, quote_args: bool) -> String {
        util::printf(
            cmd,
            &self.options.delimiter,
            &self.options.replstr,
            &self.item_list.selection.iter(),
            &self.item_list.selected(),
            &self.input.value,
            &self.input.value,
            quote_args,
        )
    }

    /// Restart matcher with debouncing to avoid excessive restarts during rapid typing
    fn restart_matcher_debounced(&mut self) {
        const DEBOUNCE_MS: u64 = 50;

        if self.options.disabled {
            return;
        }

        // If enough time has passed since last restart, restart immediately
        if self.last_matcher_restart.elapsed().as_millis() > u128::from(DEBOUNCE_MS) {
            debug!("restart_matcher_debounced: true");
            self.restart_matcher(true);
        } else {
            debug!("restart_matcher_debounced: false");
            self.pending_matcher_restart = true;
        }
    }

    /// Handle mouse events
    fn handle_mouse<B: Backend>(&mut self, mouse_event: MouseEvent, tui: &mut Tui<B>) -> Result<()>
    where
        B::Error: Send + Sync + 'static,
    {
        let mouse_pos = ratatui::layout::Position {
            x: mouse_event.column,
            y: mouse_event.row,
        };

        match mouse_event.kind {
            MouseEventKind::ScrollUp => {
                // Check if mouse is over preview area
                if let Some(preview_area) = self.layout.preview_area
                    && preview_area.contains(mouse_pos)
                {
                    // Scroll preview up
                    for evt in self.handle_action(&Action::PreviewUp(3))? {
                        tui.event_tx.try_send(evt)?;
                    }
                    return Ok(());
                }
                // Otherwise scroll item list up
                for evt in self.handle_action(&Action::Up(1))? {
                    tui.event_tx.try_send(evt)?;
                }
            }
            MouseEventKind::ScrollDown => {
                // Check if mouse is over preview area
                if let Some(preview_area) = self.layout.preview_area
                    && preview_area.contains(mouse_pos)
                {
                    // Scroll preview down
                    for evt in self.handle_action(&Action::PreviewDown(3))? {
                        tui.event_tx.try_send(evt)?;
                    }
                    return Ok(());
                }
                // Otherwise scroll item list down
                for evt in self.handle_action(&Action::Down(1))? {
                    tui.event_tx.try_send(evt)?;
                }
            }
            _ => {
                // Ignore other mouse events for now
            }
        }
        tui.event_tx.try_send(Event::Render)?;
        Ok(())
    }
    fn toggle_spinner(&mut self) {
        self.show_spinner = !self.show_spinner;
        self.spinner_last_change = std::time::Instant::now();
        self.needs_render.store(true, Ordering::Relaxed);
    }
}
