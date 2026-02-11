//! Skim is a fuzzy finder library for Rust.
//!
//! It provides a fast and customizable way to filter and select items interactively,
//! similar to fzf. Skim can be used as a library or as a command-line tool.
//!
//! # Examples
//!
//! ```no_run
//! use skim::prelude::*;
//! use std::io::Cursor;
//!
//! let options = SkimOptionsBuilder::default()
//!     .height("50%")
//!     .multi(true)
//!     .build()
//!     .unwrap();
//!
//! let input = "awk\nbash\ncsh\ndash\nfish\nksh\nzsh";
//! let item_reader = SkimItemReader::default();
//! let items = item_reader.of_bufread(Cursor::new(input));
//!
//! let output = Skim::run_with(options, Some(items)).unwrap();
//! ```

#![warn(missing_docs)]
#![cfg_attr(coverage, feature(coverage_attribute))]

#[macro_use]
extern crate log;

use std::any::Any;
use std::borrow::Cow;
use std::env;
use std::fmt::Display;
use std::io::{BufWriter, Stderr};
use std::sync::Arc;
use std::time::Duration;

use color_eyre::eyre::Result;
use color_eyre::eyre::{self, OptionExt};
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use reader::Reader;
use tokio::select;
use tui::App;
use tui::Event;
use tui::Size;

pub use crate::engine::fuzzy::FuzzyAlgorithm;
pub use crate::item::RankCriteria;
pub use crate::options::SkimOptions;
pub use crate::output::SkimOutput;
use crate::reader::ReaderControl;
pub use crate::skim_item::SkimItem;
use crate::tui::Tui;
use crate::tui::event::Action;

pub mod binds;
mod engine;
pub mod field;
pub mod fuzzy_matcher;
pub mod helper;
pub mod item;
pub mod matcher;
pub mod options;
mod output;
pub mod prelude;
pub mod reader;
mod skim_item;
pub mod spinlock;
pub mod theme;
pub mod tmux;
pub mod tui;
mod util;

#[cfg(feature = "cli")]
pub mod completions;
#[cfg(feature = "cli")]
pub mod manpage;

//------------------------------------------------------------------------------
/// Trait for downcasting to concrete types from trait objects
pub trait AsAny {
    /// Returns a reference to the value as `Any`
    fn as_any(&self) -> &dyn Any;
    /// Returns a mutable reference to the value as `Any`
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Any> AsAny for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

//------------------------------------------------------------------------------
// Display Context
#[derive(Default, Debug, Clone)]
/// Represents how a query matches an item
pub enum Matches {
    /// No matches
    #[default]
    None,
    /// Matches at specific character indices
    CharIndices(Vec<usize>),
    /// Matches in a character range (start, end)
    CharRange(usize, usize),
    /// Matches in a byte range (start, end)
    ByteRange(usize, usize),
}

#[derive(Default, Clone)]
/// Context information for displaying an item
pub struct DisplayContext {
    /// The match score for this item
    pub score: i32,
    /// Where the query matched in the item
    pub matches: Matches,
    /// The width of the container to display in
    pub container_width: usize,
    /// The base style to apply to non-matched portions
    pub base_style: Style,
    /// The style to apply to matched portions
    pub matched_syle: Style,
}

impl DisplayContext {
    /// Converts the context and text into a styled `Line` with highlighted matches
    pub fn to_line(self, cow: Cow<str>) -> Line {
        let text: String = cow.into_owned();

        // Combine base_style with match style for highlighted text
        // Match style takes precedence for fg, but inherits bg from base if not set
        match &self.matches {
            Matches::CharIndices(indices) => {
                let mut res = Line::default();
                let mut chars = text.chars();
                let mut prev_index = 0;
                for &index in indices {
                    let span_content = chars.by_ref().take(index - prev_index);
                    res.push_span(Span::styled(span_content.collect::<String>(), self.base_style));
                    let highlighted_char = chars.next().unwrap_or_default().to_string();

                    res.push_span(Span::styled(highlighted_char, self.base_style.patch(self.matched_syle)));
                    prev_index = index + 1;
                }
                res.push_span(Span::styled(chars.collect::<String>(), self.base_style));
                res
            }
            // AnsiString::from((context.text, indices, context.highlight_attr)),
            #[allow(clippy::cast_possible_truncation)]
            Matches::CharRange(start, end) => {
                let mut chars = text.chars();
                let mut res = Line::default();
                res.push_span(Span::styled(
                    chars.by_ref().take(*start).collect::<String>(),
                    self.base_style,
                ));
                let highlighted_text = chars.by_ref().take(*end - *start).collect::<String>();

                res.push_span(Span::styled(highlighted_text, self.base_style.patch(self.matched_syle)));
                res.push_span(Span::styled(chars.collect::<String>(), self.base_style));
                res
            }
            Matches::ByteRange(start, end) => {
                let mut bytes = text.bytes();
                let mut res = Line::default();
                res.push_span(Span::styled(
                    String::from_utf8(bytes.by_ref().take(*start).collect()).unwrap(),
                    self.base_style,
                ));
                let highlighted_bytes = bytes.by_ref().take(*end - *start).collect();
                let highlighted_text = String::from_utf8(highlighted_bytes).unwrap();

                res.push_span(Span::styled(highlighted_text, self.base_style.patch(self.matched_syle)));
                res.push_span(Span::styled(
                    String::from_utf8(bytes.collect()).unwrap(),
                    self.base_style,
                ));
                res
            }
            Matches::None => Line::from(vec![Span::styled(text, self.base_style)]),
        }
    }
}

//------------------------------------------------------------------------------
// Preview Context

/// Context information for generating item previews
pub struct PreviewContext<'a> {
    /// The current search query
    pub query: &'a str,
    /// The current command query (for interactive mode)
    pub cmd_query: &'a str,
    /// Width of the preview window
    pub width: usize,
    /// Height of the preview window
    pub height: usize,
    /// Index of the current item
    pub current_index: usize,
    /// Text of the current selection
    pub current_selection: &'a str,
    /// selected item indices (may or may not include current item)
    pub selected_indices: &'a [usize],
    /// selected item texts (may or may not include current item)
    pub selections: &'a [&'a str],
}

//------------------------------------------------------------------------------
// Preview
#[derive(Default, Copy, Clone, Debug)]
/// Position and scroll information for preview display
pub struct PreviewPosition {
    /// Horizontal scroll position
    pub h_scroll: Size,
    /// Horizontal offset
    pub h_offset: Size,
    /// Vertical scroll position
    pub v_scroll: Size,
    /// Vertical offset
    pub v_offset: Size,
}

/// Defines how an item should be previewed
pub enum ItemPreview {
    /// execute the command and print the command's output
    Command(String),
    /// Display the prepared text(lines)
    Text(String),
    /// Display the colored text(lines)
    AnsiText(String),
    /// Execute a command and display output with position
    CommandWithPos(String, PreviewPosition),
    /// Display text with position
    TextWithPos(String, PreviewPosition),
    /// Display ANSI-colored text with position
    AnsiWithPos(String, PreviewPosition),
    /// Use global command settings to preview the item
    Global,
}

//==============================================================================
// A match engine will execute the matching algorithm

#[derive(Eq, PartialEq, Debug, Copy, Clone, Default)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[cfg_attr(feature = "cli", clap(rename_all = "snake_case"))]
/// Case sensitivity mode for matching
pub enum CaseMatching {
    /// Case-sensitive matching
    Respect,
    /// Case-insensitive matching
    Ignore,
    /// Smart case: case-insensitive unless query contains uppercase
    #[default]
    Smart,
}

#[derive(PartialEq, Eq, Clone, Debug)]
#[allow(dead_code)]
/// Represents the range of a match in an item
pub enum MatchRange {
    /// Range of bytes (start, end)
    ByteRange(usize, usize),
    /// Individual character indices that matched
    Chars(Vec<usize>),
}

/// Rank tuple used for sorting match results
/// The field will be ordered based on the `tiebreak` parameter
pub type Rank = [i32; 5];

#[derive(Clone)]
/// Result of matching a query against an item
pub struct MatchResult {
    /// The rank/score of this match
    pub rank: Rank,
    /// The range where the match occurred
    pub matched_range: MatchRange,
}

impl MatchResult {
    #[must_use]
    /// Converts the match range to character indices
    pub fn range_char_indices(&self, text: &str) -> Vec<usize> {
        match &self.matched_range {
            &MatchRange::ByteRange(start, end) => {
                let first = text[..start].chars().count();
                let last = first + text[start..end].chars().count();
                (first..last).collect()
            }
            MatchRange::Chars(vec) => vec.clone(),
        }
    }
}

/// A matching engine that can match queries against items
pub trait MatchEngine: Sync + Send + Display {
    /// Matches an item against the query, returning a result if matched
    fn match_item(&self, item: &dyn SkimItem) -> Option<MatchResult>;
}

/// Factory for creating match engines
pub trait MatchEngineFactory {
    /// Creates a match engine with explicit case sensitivity
    fn create_engine_with_case(&self, query: &str, case: CaseMatching) -> Box<dyn MatchEngine>;
    /// Creates a match engine with default case sensitivity
    fn create_engine(&self, query: &str) -> Box<dyn MatchEngine> {
        self.create_engine_with_case(query, CaseMatching::default())
    }
}

//------------------------------------------------------------------------------
// Preselection

/// A selector that determines whether an item should be "pre-selected" in multi-selection mode
pub trait Selector {
    /// Returns true if the item at the given index should be pre-selected
    fn should_select(&self, index: usize, item: &dyn SkimItem) -> bool;
}

//------------------------------------------------------------------------------
/// Sender for streaming items to skim
pub type SkimItemSender = kanal::Sender<Vec<Arc<dyn SkimItem>>>;
/// Receiver for streaming items to skim
pub type SkimItemReceiver = kanal::Receiver<Vec<Arc<dyn SkimItem>>>;

/// Main entry point for running skim
pub struct Skim<Backend = ratatui::backend::CrosstermBackend<BufWriter<Stderr>>>
where
    Backend: ratatui::backend::Backend,
    Backend::Error: Send + Sync + 'static,
{
    app: App,
    tui: Option<Tui<Backend>>,
    height: Size,
    reader: Reader,
    reader_done: bool,
    initial_cmd: String,
    reader_control: Option<ReaderControl>,
    matcher_interval: Option<tokio::time::Interval>,
    listener: Option<interprocess::local_socket::tokio::Listener>,
    final_event: Event,
    final_key: KeyEvent,
}

impl Skim {
    /// Run skim, collecting items from the source and using options
    ///
    /// # Params
    ///
    /// - options: the "complex" options that control how skim behaves
    /// - source: a stream of items to be passed to skim for filtering.
    ///   If None is given, skim will invoke the command given to fetch the items.
    ///
    /// # Returns
    ///
    /// - None: on internal errors.
    /// - `SkimOutput`: the collected key, event, query, selected items, etc.
    ///
    /// # Panics
    ///
    /// Panics if the tui fails to initilize
    pub fn run_with(options: SkimOptions, source: Option<SkimItemReceiver>) -> Result<SkimOutput> {
        trace!("running skim");
        let mut skim = Self::init(options, source)?;

        skim.start();

        if skim.should_enter() {
            let rt = tokio::runtime::Runtime::new()?;

            skim.init_tui()?;
            rt.block_on(async {
                skim.enter().await?;
                skim.run().await?;
                eyre::Ok(())
            })?;
        } else {
            // We didn't enter
            skim.final_event = Event::Action(Action::Accept(None));
        }
        let output = skim.output();
        debug!("output: {output:?}");

        Ok(output)
    }
    /// Initialize skim, without starting anything yet
    pub fn init(options: SkimOptions, source: Option<SkimItemReceiver>) -> Result<Self> {
        let height = Size::try_from(options.height.as_str())?;

        // application state
        // Initialize theme from options
        let theme = Arc::new(crate::theme::ColorTheme::init_from_options(&options));
        let reader = Reader::from_options(&options).source(source);
        const SKIM_DEFAULT_COMMAND: &str = "find .";
        let default_command = String::from(match env::var("SKIM_DEFAULT_COMMAND").as_deref() {
            Err(_) | Ok("") => SKIM_DEFAULT_COMMAND,
            Ok(v) => v,
        });
        let cmd = options.cmd.clone().unwrap_or(default_command);

        let app = App::from_options(options, theme.clone(), cmd.clone());

        //------------------------------------------------------------------------------
        // reader
        // In interactive mode, expand all placeholders ({}, {q}, etc) with initial query (empty or from --query)
        let initial_cmd = if app.options.interactive && app.options.cmd.is_some() {
            let expanded = app.expand_cmd(&cmd, true);
            log::debug!(
                "Interactive mode: initial_cmd = {:?} (from template {:?})",
                expanded,
                cmd
            );
            expanded
        } else {
            cmd.clone()
        };
        Ok(Self {
            app,
            height,
            reader,
            reader_done: false,
            initial_cmd,
            tui: None,
            reader_control: None,
            matcher_interval: None,
            listener: None,
            final_event: Event::Quit,
            final_key: KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        })
    }

    /// Start the reader and matcher, but do not enter the TUI yet
    pub fn start(&mut self) {
        debug!("Starting reader with initial_cmd: {:?}", self.initial_cmd);
        self.reader_control = Some(self.reader.collect(self.app.item_pool.clone(), &self.initial_cmd));
        self.app.restart_matcher(true);
    }

    /// Initialize the TUI, but do not enter it yet
    pub fn init_tui(&mut self) -> Result<()> {
        self.tui = Some(tui::Tui::new_with_height(self.height)?);
        Ok(())
    }

    /// Returns a clone of the TUI event sender.
    ///
    /// Use this to send events (e.g. [`Event::Render`], [`Event::Action`])
    /// to the running skim instance from outside the event loop. The sender
    /// is cheap to clone and can be moved into async blocks or other tasks.
    ///
    /// Must be called after [`init_tui()`](Self::init_tui).
    pub fn event_sender(&self) -> tokio::sync::mpsc::Sender<Event> {
        self.tui
            .as_ref()
            .expect("TUI needs to be initialized using Skim::init_tui before getting the event sender")
            .event_tx
            .clone()
    }

    /// Enter the TUI
    pub async fn enter(&mut self) -> Result<()> {
        debug!("Entering TUI");
        self.init_listener().await?;
        self.tui
            .as_mut()
            .expect("TUI needs to be initialized using Skim::init_tui before entering")
            .enter()
    }

    /// Checks read-0 select-1, and sync to wait and returns whether or not we should enter
    fn should_enter(&mut self) -> bool {
        let reader_control = self
            .reader_control
            .as_ref()
            .expect("reader_control needs to be initilized using Skim::start");
        let app = &mut self.app;
        // Deal with read-0 / select-1
        let min_items_before_enter = if app.options.exit_0 {
            1
        } else if app.options.select_1 {
            2
        } else if app.options.sync {
            usize::MAX
        } else {
            0
        };
        if min_items_before_enter > 0 || app.options.sync {
            trace!(
                "checking matcher, stopped: {}, processed: {}, matched: {}/{}, pool: {}, query: {}, reader_control_done: {}",
                app.matcher_control.stopped(),
                app.matcher_control.get_num_processed(),
                app.matcher_control.get_num_matched(),
                min_items_before_enter,
                app.item_pool.num_not_taken(),
                app.input.value,
                reader_control.is_done()
            );
            while app.matcher_control.get_num_matched() < min_items_before_enter
                && (!app.matcher_control.stopped() || !reader_control.is_done())
            {
                trace!("still waiting");
                std::thread::sleep(Duration::from_millis(10));
                app.restart_matcher(false);
            }
            trace!(
                "checked matcher, stopped: {}, processed: {}, pool: {}, query: {}, reader_control_done: {}",
                app.matcher_control.stopped(),
                app.matcher_control.get_num_processed(),
                app.item_pool.num_not_taken(),
                app.input.value,
                reader_control.is_done()
            );
            trace!(
                "checking for matched item count before entering: {}/{min_items_before_enter}",
                app.matcher_control.get_num_matched()
            );
            if app.matcher_control.get_num_matched() == min_items_before_enter - 1 {
                app.item_list.items = app.item_list.processed_items.lock().take().unwrap_or_default().items;
                debug!("early exit, result: {:?}", app.results());
                return false;
            };
        }
        true
    }

    /// Initialize the IPC socket listener
    /// This needs to be called from an async context despite being sync
    async fn init_listener(&mut self) -> Result<()> {
        if let Some(socket_name) = &self.app.options.listen {
            self.listener = Some(
                interprocess::local_socket::ListenerOptions::new()
                    .name(interprocess::local_socket::ToNsName::to_ns_name::<
                        interprocess::local_socket::GenericNamespaced,
                    >(socket_name.to_owned())?)
                    .create_tokio()?,
            )
        }
        Ok(())
    }

    /// Capture `self` and extract the output
    /// This will perform cleanup
    pub fn output(mut self) -> SkimOutput {
        if let Some(mut rc) = self.reader_control.take() {
            rc.kill()
        }

        // Extract final_key and is_abort from final_event
        let is_abort = !matches!(&self.final_event, Event::Action(Action::Accept(_)));

        SkimOutput {
            cmd: if self.app.options.interactive {
                // In interactive mode, cmd is what the user typed
                self.app.input.to_string()
            } else if self.app.options.cmd_query.is_some() {
                // If cmd_query was provided, use that for output
                self.app.options.cmd_query.clone().unwrap()
            } else {
                // Otherwise use the execution command
                self.initial_cmd
            },
            final_event: self.final_event,
            final_key: self.final_key,
            query: self.app.input.to_string(),
            is_abort,
            selected_items: self.app.results(),
            header: self.app.header.header.clone(),
        }
    }

    /// Returns true if skim has finished (the user accepted or aborted)
    pub fn should_quit(&self) -> bool {
        self.app.should_quit
    }

    /// Process a single event loop iteration.
    ///
    /// This awaits the next event from the TUI, matcher, or IPC listener,
    /// processes it, and returns. Use this in your own event loop when you
    /// need fine-grained control over the application lifecycle.
    ///
    /// Returns `Ok(true)` if skim should quit, `Ok(false)` to continue.
    ///
    /// # Example
    ///
    /// ```ignore
    /// while !skim.tick().await? {
    ///     // do your own work between ticks
    /// }
    /// ```
    pub async fn tick(&mut self) -> Result<bool> {
        let matcher_interval = &mut self.matcher_interval;
        select! {
            event = self.tui.as_mut().expect("TUI should be initialized before the event loop can start").next() => {
                let evt = event.ok_or_eyre("Could not acquire next event")?;

                if let Event::Key(k) = &evt {
                  self.final_key = k.to_owned();
                } else {
                  self.final_event = evt.to_owned();
                }


                // Handle reload event separately
                if let Event::Reload(new_cmd) = &evt {
                    debug!("reloading with cmd {new_cmd}");
                    // Kill the current reader
                    if let Some(rc) = self.reader_control.as_mut() { rc.kill() }
                    // Clear items
                    self.app.item_pool.clear();
                    // Clear displayed items unless no_clear_if_empty is set
                    // (in which case the item_list will handle keeping stale items)
                    if !self.app.options.no_clear_if_empty {
                        self.app.item_list.clear();
                    }
                    self.app.restart_matcher(true);
                    // Start a new reader with the new command (no source, using cmd)
                    self.reader_control = Some(self.reader.collect(self.app.item_pool.clone(), new_cmd));
                    self.reader_done = false;
                } else {
                    self.app.handle_event(self.tui.as_mut().expect("TUI should be initialized before handling events"), &evt)?;
                }

                // Check reader status and update
                if self.reader_control.as_ref().is_some_and(|rc| rc.is_done()) && !self.reader_done {
                    self.app.restart_matcher(false);
                }
            }
            _ = async {
                match matcher_interval {
                    Some(interval) => { interval.tick().await; },
                    None => std::future::pending::<()>().await,
                }
            } => {
              self.app.restart_matcher(false);
            }
            Ok(stream) = async {
                match &self.listener {
                    Some(l) => interprocess::local_socket::traits::tokio::Listener::accept(l).await,
                    None => std::future::pending().await,
                }
            } => {
                debug!("Listener accepted a connection");
                let event_tx_clone_ipc = self.tui.as_ref().expect("TUI should be initialized before listening").event_tx.clone();
                tokio::spawn(async move {
                    use tokio::io::AsyncBufReadExt;
                    let reader = tokio::io::BufReader::new(stream);
                    let mut lines = reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        debug!("listener: got {line}");
                        if let Ok(act) = ron::from_str::<Action>(&line) {
                            debug!("listener: parsed into action {act:?}");
                            _ = event_tx_clone_ipc.try_send(Event::Action(act));
                            _ = event_tx_clone_ipc.try_send(Event::Render);
                        }
                    }
                });
            }
        }

        Ok(self.app.should_quit)
    }

    /// Run the event loop on the current task until skim quits.
    ///
    /// This is a convenience wrapper around [`tick()`](Self::tick) that loops
    /// until the user accepts or aborts. Use `tick()` directly if you need
    /// to interleave your own logic between iterations.
    pub async fn run(&mut self) -> Result<()> {
        self.matcher_interval = Some(tokio::time::interval(Duration::from_millis(100)));
        trace!("Starting event loop");
        loop {
            if self.tick().await? {
                break Ok(());
            }
        }
    }

    /// Spawn the event loop and run a user-provided future concurrently.
    ///
    /// This consumes `self`, spawns the event loop as a local task, and runs
    /// `user_task` alongside it. When the user accepts or aborts in the TUI,
    /// the event loop completes and the [`SkimOutput`] is returned — regardless
    /// of whether `user_task` has finished.
    ///
    /// Use this when you need to send items or do other work concurrently
    /// while the TUI is running.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let output = skim.run_until(async {
    ///     for i in 1..=10 {
    ///         tx.send(vec![Arc::new(format!("item {i}"))]);
    ///         tokio::time::sleep(Duration::from_millis(100)).await;
    ///     }
    /// }).await?;
    /// ```
    pub async fn run_until<F: Future + 'static>(mut self, user_task: F) -> Result<SkimOutput> {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let handle = tokio::task::spawn_local(async move {
                    self.run().await?;
                    Ok(self.output())
                });
                tokio::task::spawn_local(user_task);
                handle.await?
            })
            .await
    }
}
