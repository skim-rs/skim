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
//!     .height(Some("50%"))
//!     .multi(true)
//!     .build()
//!     .unwrap();
//!
//! let input = "awk\nbash\ncsh\ndash\nfish\nksh\nzsh";
//! let item_reader = SkimItemReader::default();
//! let items = item_reader.of_bufread(Cursor::new(input));
//!
//! let output = Skim::run_with(&options, Some(items)).unwrap();
//! ```

#![warn(missing_docs)]

#[macro_use]
extern crate log;

use std::any::Any;
use std::borrow::Cow;
use std::env;
use std::fmt::Display;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use color_eyre::eyre::Result;
use color_eyre::eyre::{self, OptionExt};
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::prelude::CrosstermBackend;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use reader::Reader;
use tokio::select;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tui::App;
use tui::Event;
use tui::Size;

pub use crate::engine::fuzzy::FuzzyAlgorithm;
pub use crate::item::RankCriteria;
pub use crate::options::SkimOptions;
pub use crate::output::SkimOutput;
pub use crate::skim_item::SkimItem;
use crate::tui::event::Action;

pub mod binds;
mod engine;
pub mod field;
pub mod fuzzy_matcher;
pub mod helper;
pub mod item;
mod matcher;
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
#[derive(Default, Debug)]
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

#[derive(Default)]
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
    fn match_item(&self, item: Arc<dyn SkimItem>) -> Option<MatchResult>;
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
pub type SkimItemSender = UnboundedSender<Arc<dyn SkimItem>>;
/// Receiver for streaming items to skim
pub type SkimItemReceiver = UnboundedReceiver<Arc<dyn SkimItem>>;

/// Main entry point for running skim
pub struct Skim {}

impl Skim {
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
        let height = Size::try_from(options.height.as_str())?;
        let backend = CrosstermBackend::new(std::io::stderr());
        let mut tui = tui::Tui::new_with_height(backend, height)?;

        // application state
        // Initialize theme from options
        let theme = Arc::new(crate::theme::ColorTheme::init_from_options(&options));
        let mut reader = Reader::from_options(&options).source(source);
        const SKIM_DEFAULT_COMMAND: &str = "find .";
        let default_command = String::from(match env::var("SKIM_DEFAULT_COMMAND").as_deref() {
            Err(_) | Ok("") => SKIM_DEFAULT_COMMAND,
            Ok(v) => v,
        });
        let cmd = options.cmd.clone().unwrap_or(default_command);
        let listen_socket = options.listen.clone();

        let mut app = App::from_options(options, theme.clone(), cmd.clone());

        let rt = tokio::runtime::Runtime::new()?;
        let mut final_event: Event = Event::Quit;
        let mut final_key: KeyEvent = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
        rt.block_on(async {
            tui.enter()?;

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
            log::debug!("Starting reader with initial_cmd: {:?}", initial_cmd);
            let (item_tx, mut item_rx) = unbounded_channel();
            let mut reader_control = reader.run(item_tx.clone(), &initial_cmd);

            //------------------------------------------------------------------------------
            // model + previewer

            let mut matcher_interval = tokio::time::interval(Duration::from_millis(100));
            let reader_done = Arc::new(AtomicBool::new(false));
            let reader_done_clone = reader_done.clone();

            let item_pool = app.item_pool.clone();
            tokio::spawn(async move {
                const BATCH: usize = 4096; // Smaller batches for more responsive updates
                loop {
                    let mut buf = Vec::with_capacity(BATCH);
                    if item_rx.recv_many(&mut buf, BATCH).await > 0 {
                        item_pool.append(buf);
                        trace!("Got new items, len {}", item_pool.len());
                    } else {
                        reader_done_clone.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            });

            // Start matcher initially
            app.restart_matcher(true);

            let listener = if let Some(socket_name) = &listen_socket {
                debug!("starting listener on socket at {socket_name}");
                Some(
                    interprocess::local_socket::ListenerOptions::new()
                        .name(
                            interprocess::local_socket::ToNsName::to_ns_name::<
                                interprocess::local_socket::GenericNamespaced,
                            >(socket_name.as_str())
                            .unwrap_or_else(|e| {
                                panic!("Failed to build full name for IPC listener from {socket_name}: {e}")
                            }),
                        )
                        .create_tokio()
                        .unwrap_or_else(|e| panic!("Failed to create tokio IPC listener at {socket_name}: {e}")),
                )
            } else {
                None
            };
            let event_tx_clone = tui.event_tx.clone();

            loop {
                select! {
                    event = tui.next() => {
                        let evt = event.ok_or_eyre("Could not acquire next event")?;

                        // Handle reload event
                        if let Event::Reload(new_cmd) = &evt {
                            debug!("reloading with cmd {new_cmd}");
                            // Kill the current reader
                            reader_control.kill();
                            // Clear items
                            app.item_pool.clear();
                            // Clear displayed items unless no_clear_if_empty is set
                            // (in which case the item_list will handle keeping stale items)
                            if !app.options.no_clear_if_empty {
                                app.item_list.clear();
                            }
                            app.restart_matcher(true);
                            // Start a new reader with the new command (no source, using cmd)
                            reader_control = reader.run(item_tx.clone(), new_cmd);
                            app.status.reading = true;
                            reader_done.store(false, std::sync::atomic::Ordering::Relaxed);
                        }
                        if let Event::Key(k) = &evt {
                          final_key = k.to_owned();
                        } else {
                          final_event = evt.to_owned();
                        }

                        // Check reader status and update
                        if !reader_control.is_done() {
                          app.reader_timer = Instant::now();
                        } else if ! reader_done.load(std::sync::atomic::Ordering::Relaxed) {
                            reader_done.store(true, std::sync::atomic::Ordering::Relaxed);
                            app.restart_matcher(true);
                            app.status.reading = false;
                        }
                        app.handle_event(&mut tui, &evt)?;
                    }
                    _ = matcher_interval.tick() => {
                      app.restart_matcher(false);
                    }
                    Ok(stream) = async {
                        match &listener {
                            Some(l) => interprocess::local_socket::traits::tokio::Listener::accept(l).await,
                            None => std::future::pending().await,
                        }
                    } => {
                        let event_tx_clone_ipc = event_tx_clone.clone();
                        tokio::spawn(async move {
                            use tokio::io::AsyncBufReadExt;
                            let reader = tokio::io::BufReader::new(stream);
                            let mut lines = reader.lines();
                            while let Ok(Some(line)) = lines.next_line().await {
                                debug!("listener: got {line}");
                                if let Ok(act) = ron::from_str::<Action>(&line) {
                                    debug!("listener: parsed into action {act:?}");
                                    _ = event_tx_clone_ipc.send(Event::Action(act));
                                    _ = event_tx_clone_ipc.send(Event::Render);
                                }
                            }
                        });
                    }
                }

                if app.should_quit {
                    break;
                }
            }
            reader_control.kill();
            eyre::Ok(())
        })?;

        // Extract final_key and is_abort from final_event
        let is_abort = !matches!(&final_event, Event::Action(Action::Accept(_)));

        Ok(SkimOutput {
            cmd: if app.options.interactive {
                // In interactive mode, cmd is what the user typed
                app.input.to_string()
            } else if app.options.cmd_query.is_some() {
                // If cmd_query was provided, use that for output
                app.options.cmd_query.clone().unwrap()
            } else {
                // Otherwise use the execution command
                cmd
            },
            final_event,
            final_key,
            query: app.input.to_string(),
            is_abort,
            selected_items: app.results(),
            header: app.header.header,
        })
    }
}
