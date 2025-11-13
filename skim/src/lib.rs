#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

use std::any::Any;
use std::borrow::Cow;
use std::env;
use std::fmt::Display;
use std::sync::Arc;
use std::time::Duration;

use color_eyre::eyre;
use color_eyre::eyre::OptionExt;
use color_eyre::eyre::Result;
use crossbeam::channel::{Receiver, Sender};
use crossterm::event::KeyCode;
use ratatui::prelude::CrosstermBackend;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use reader::Reader;
use tokio::select;
use tui::App;
use tui::Event;
use tui::Size;

pub use crate::engine::fuzzy::FuzzyAlgorithm;
pub use crate::item::RankCriteria;
pub use crate::options::SkimOptions;
pub use crate::output::SkimOutput;

pub mod binds;
mod engine;
pub mod field;
mod global;
mod helper;
pub mod item;
mod matcher;
pub mod options;
mod orderedvec;
mod output;
pub mod prelude;
pub mod reader;
mod spinlock;
mod theme;
pub mod tmux;
pub mod tui;
mod util;

//------------------------------------------------------------------------------
pub trait AsAny {
    fn as_any(&self) -> &dyn Any;
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

/// A `SkimItem` defines what's been processed(fetched, matched, previewed and returned) by skim
///
/// # Downcast Example
/// Skim will return the item back, but in `Arc<dyn SkimItem>` form. We might want a reference
/// to the concrete type instead of trait object. Skim provide a somehow "complicated" way to
/// `downcast` it back to the reference of the original concrete type.
///
/// ```rust
/// use skim::prelude::*;
///
/// struct MyItem {}
/// impl SkimItem for MyItem {
///     fn text(&self) -> Cow<str> {
///         unimplemented!()
///     }
/// }
///
/// impl MyItem {
///     pub fn mutable(&mut self) -> i32 {
///         1
///     }
///
///     pub fn immutable(&self) -> i32 {
///         0
///     }
/// }
///
/// let mut ret: Arc<dyn SkimItem> = Arc::new(MyItem{});
/// let mutable: &mut MyItem = Arc::get_mut(&mut ret)
///     .expect("item is referenced by others")
///     .as_any_mut() // cast to Any
///     .downcast_mut::<MyItem>() // downcast to (mut) concrete type
///     .expect("something wrong with downcast");
/// assert_eq!(mutable.mutable(), 1);
///
/// let immutable: &MyItem = (*ret).as_any() // cast to Any
///     .downcast_ref::<MyItem>() // downcast to concrete type
///     .expect("something wrong with downcast");
/// assert_eq!(immutable.immutable(), 0)
/// ```
pub trait SkimItem: AsAny + Send + Sync + 'static {
    /// The string to be used for matching (without color)
    fn text(&self) -> Cow<'_, str>;

    /// The content to be displayed on the item list, could contain ANSI properties
    fn display<'a>(&'a self, context: DisplayContext) -> Line<'a> {
        context.to_line(self.text())
    }

    /// Custom preview content, default to `ItemPreview::Global` which will use global preview
    /// setting(i.e. the command set by `preview` option)
    fn preview(&self, _context: PreviewContext) -> ItemPreview {
        ItemPreview::Global
    }

    /// Get output text(after accept), default to `text()`
    /// Note that this function is intended to be used by the caller of skim and will not be used by
    /// skim. And since skim will return the item back in `SkimOutput`, if string is not what you
    /// want, you could still use `downcast` to retain the pointer to the original struct.
    fn output(&self) -> Cow<'_, str> {
        self.text()
    }

    /// we could limit the matching ranges of the `get_text` of the item.
    /// providing (`start_byte`, `end_byte`) of the range
    fn get_matching_ranges(&self) -> Option<&[(usize, usize)]> {
        None
    }

    /// Get index, for matching purposes
    ///
    /// Implemented as no-op for retro-compatibility purposes
    fn get_index(&self) -> usize {
        0
    }
    /// Set index, for matching purposes
    ///
    /// Implemented as no-op for retro-compatibility purposes
    fn set_index(&mut self, _index: usize) {}
}

//------------------------------------------------------------------------------
// Implement SkimItem for raw strings

impl<T: AsRef<str> + Send + Sync + 'static> SkimItem for T {
    fn text(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.as_ref())
    }
}

//------------------------------------------------------------------------------
// Display Context
#[derive(Default)]
pub enum Matches {
    #[default]
    None,
    CharIndices(Vec<usize>),
    CharRange(usize, usize),
    ByteRange(usize, usize),
}

#[derive(Default)]
pub struct DisplayContext {
    pub score: i32,
    pub matches: Matches,
    pub container_width: usize,
    pub style: Style,
}

impl DisplayContext {
    pub fn to_line(self, cow: Cow<str>) -> Line {
        let text: String = cow.into_owned();

        match &self.matches {
            Matches::CharIndices(indices) => {
                let mut res = Line::default();
                let mut chars = text.chars();
                let mut prev_index = 0;
                for &index in indices {
                    let span_content = chars.by_ref().take(index - prev_index);
                    res.push_span(Span::raw(span_content.collect::<String>()));
                    res.push_span(Span::styled(chars.next().unwrap_or_default().to_string(), self.style));
                    prev_index = index + 1;
                }
                res.push_span(Span::raw(chars.collect::<String>()));
                res
            }
            // AnsiString::from((context.text, indices, context.highlight_attr)),
            #[allow(clippy::cast_possible_truncation)]
            Matches::CharRange(start, end) => {
                let mut chars = text.chars();
                let mut res = Line::raw(chars.by_ref().take(*start).collect::<String>());
                res.push_span(Span::styled(
                    chars.by_ref().take(*end - *start).collect::<String>(),
                    self.style,
                ));
                res.push_span(Span::raw(chars.collect::<String>()));
                res
            }
            Matches::ByteRange(start, end) => {
                let mut bytes = text.bytes();
                let mut res = Line::raw(String::from_utf8(bytes.by_ref().take(*start).collect()).unwrap());
                res.push_span(Span::styled(
                    String::from_utf8(bytes.by_ref().take(*end - *start).collect()).unwrap(),
                    self.style,
                ));
                res.push_span(Span::raw(String::from_utf8(bytes.collect()).unwrap()));
                res
            }
            Matches::None => Line::raw(text),
        }
    }
}

//------------------------------------------------------------------------------
// Preview Context

pub struct PreviewContext<'a> {
    pub query: &'a str,
    pub cmd_query: &'a str,
    pub width: usize,
    pub height: usize,
    pub current_index: usize,
    pub current_selection: &'a str,
    /// selected item indices (may or may not include current item)
    pub selected_indices: &'a [usize],
    /// selected item texts (may or may not include current item)
    pub selections: &'a [&'a str],
}

//------------------------------------------------------------------------------
// Preview
#[derive(Default, Copy, Clone, Debug)]
pub struct PreviewPosition {
    pub h_scroll: Size,
    pub h_offset: Size,
    pub v_scroll: Size,
    pub v_offset: Size,
}

pub enum ItemPreview {
    /// execute the command and print the command's output
    Command(String),
    /// Display the prepared text(lines)
    Text(String),
    /// Display the colored text(lines)
    AnsiText(String),
    CommandWithPos(String, PreviewPosition),
    TextWithPos(String, PreviewPosition),
    AnsiWithPos(String, PreviewPosition),
    /// Use global command settings to preview the item
    Global,
}

//==============================================================================
// A match engine will execute the matching algorithm

#[derive(Eq, PartialEq, Debug, Copy, Clone, Default)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[cfg_attr(feature = "cli", clap(rename_all = "snake_case"))]
pub enum CaseMatching {
    Respect,
    Ignore,
    #[default]
    Smart,
}

#[derive(PartialEq, Eq, Clone, Debug)]
#[allow(dead_code)]
pub enum MatchRange {
    ByteRange(usize, usize),
    // range of bytes
    Chars(Vec<usize>), // individual character indices matched
}

pub type Rank = [i32; 4];

#[derive(Clone)]
pub struct MatchResult {
    pub rank: Rank,
    pub matched_range: MatchRange,
}

impl MatchResult {
    #[must_use]
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

pub trait MatchEngine: Sync + Send + Display {
    fn match_item(&self, item: Arc<dyn SkimItem>) -> Option<MatchResult>;
}

pub trait MatchEngineFactory {
    fn create_engine_with_case(&self, query: &str, case: CaseMatching) -> Box<dyn MatchEngine>;
    fn create_engine(&self, query: &str) -> Box<dyn MatchEngine> {
        self.create_engine_with_case(query, CaseMatching::default())
    }
}

//------------------------------------------------------------------------------
// Preselection

/// A selector that determines whether an item should be "pre-selected" in multi-selection mode
pub trait Selector {
    fn should_select(&self, index: usize, item: &dyn SkimItem) -> bool;
}

//------------------------------------------------------------------------------
pub type SkimItemSender = Sender<Arc<dyn SkimItem>>;
pub type SkimItemReceiver = Receiver<Arc<dyn SkimItem>>;

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
    #[must_use]
    pub fn run_with(options: SkimOptions, source: Option<SkimItemReceiver>) -> Result<SkimOutput> {
        // let min_height = Skim::parse_height_string(&options.min_height);

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

        let mut app = App::from_options(options, theme.clone(), cmd.clone());

        let rt = tokio::runtime::Runtime::new()?;
        let mut final_event: Event = Event::Quit;
        rt.block_on(async {
            tui.enter()?;

            //------------------------------------------------------------------------------
            // reader
            // In interactive mode, expand {} with initial query (empty or from --query)
            let initial_cmd = if app.options.interactive && app.options.cmd.is_some() {
                cmd.replace("{}", &app.input.value)
            } else {
                cmd.clone()
            };
            let mut reader_control = reader.run(app.item_tx.clone(), &initial_cmd);

            //------------------------------------------------------------------------------
            // model + previewer
            // let _ = term.send_event(TermEvent::User(())); // interrupt the input thread
            // let _ = input_thread.join();

            let mut reader_done = false;
            let mut item_receiver_interval = tokio::time::interval(Duration::from_millis(500));
            let mut matcher_interval = tokio::time::interval(Duration::from_millis(500));

            const BUF_CAPACITY: usize = 1 << 16;
            loop {
                select! {
                    event = tui.next() => {
                        let evt = event.ok_or_eyre("Could not acquire next event")?;

                        // Handle reload event
                        if let Event::Reload(new_cmd) = &evt {
                            // Kill the current reader
                            reader_control.kill();
                            // Clear items
                            app.item_pool.clear();
                            app.restart_matcher(true);
                            // Start a new reader with the new command (no source, using cmd)
                            reader_control = reader.run(app.item_tx.clone(), new_cmd);
                            reader_done = false;
                        }

                        final_event = evt.clone();

                        if reader_control.is_done() && ! reader_done {
                            app.restart_matcher(true);
                            reader_done = true;
                        }
                        app.handle_event(&mut tui, &evt)?;
                    }
                    _ = item_receiver_interval.tick() => {
                      while let Ok(item) = app.item_rx.try_recv() {
                        let mut items = Vec::with_capacity(BUF_CAPACITY);
                        items.push(item);
                        app.handle_items(items);
                      }
                    }
                    _ = matcher_interval.tick() => {
                      app.restart_matcher(true);
                    }
                }

                if app.should_quit {
                    break;
                }
            }
            reader_control.kill();
            eyre::Ok(())
        })?;

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
            final_key: KeyCode::Enter, // TODO
            query: app.input.to_string(),
            is_abort: false,
            selected_items: app.results(),
        })
    }
}
