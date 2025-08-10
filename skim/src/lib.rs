#[macro_use]
extern crate log;

use std::any::Any;
use std::borrow::Cow;
use std::fmt::Display;
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::thread;

use crossbeam::channel::{Receiver, Sender};
use skim_tuikit::prelude::{Event as TermEvent, *};

pub use crate::ansi::AnsiString;
pub use crate::engine::fuzzy::FuzzyAlgorithm;
use crate::event::{EventReceiver, EventSender};
pub use crate::item::RankCriteria;
use crate::model::Model;
pub use crate::options::SkimOptions;
pub use crate::output::SkimOutput;
use crate::reader::Reader;
pub use skim_common::spinlock;
pub use skim_tuikit as tuikit;

mod ansi;
mod engine;
mod event;
pub mod field;
mod global;
mod header;
mod helper;
mod input;
pub mod item;
mod matcher;
mod model;
pub mod options;
mod orderedvec;
mod output;
pub mod prelude;
mod previewer;
mod query;
pub mod reader;
mod selection;
mod theme;
pub mod tmux;
mod util;

// Optional ratatui-based UI module (only when ratatui-ui feature is enabled)
#[cfg(feature = "ratatui-ui")]
pub mod ui;

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
    fn display<'a>(&'a self, context: DisplayContext<'a>) -> AnsiString<'a> {
        AnsiString::from(context)
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
pub enum Matches<'a> {
    None,
    CharIndices(&'a [usize]),
    CharRange(usize, usize),
    ByteRange(usize, usize),
}

pub struct DisplayContext<'a> {
    pub text: &'a str,
    pub score: i32,
    pub matches: Matches<'a>,
    pub container_width: usize,
    pub highlight_attr: Attr,
}

impl<'a> From<DisplayContext<'a>> for AnsiString<'a> {
    fn from(context: DisplayContext<'a>) -> Self {
        match context.matches {
            Matches::CharIndices(indices) => AnsiString::from((context.text, indices, context.highlight_attr)),
            #[allow(clippy::cast_possible_truncation)]
            Matches::CharRange(start, end) => {
                AnsiString::new_str(context.text, vec![(context.highlight_attr, (start as u32, end as u32))])
            }
            Matches::ByteRange(start, end) => {
                let ch_start = context.text[..start].chars().count();
                let ch_end = ch_start + context.text[start..end].chars().count();
                #[allow(clippy::cast_possible_truncation)]
                AnsiString::new_str(
                    context.text,
                    vec![(context.highlight_attr, (ch_start as u32, ch_end as u32))],
                )
            }
            Matches::None => AnsiString::new_str(context.text, vec![]),
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

pub type Rank = [i32; 5];

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
    pub fn run_with(options: &SkimOptions, source: Option<SkimItemReceiver>) -> Option<SkimOutput> {
        // Feature flag: use environment variable to choose UI system
        #[cfg(feature = "ratatui-ui")]
        {
            let use_ratatui = std::env::var("SKIM_USE_RATATUI").is_ok();
            if use_ratatui {
                return Self::run_with_ratatui(options, source);
            }
        }
        
        // Default: use legacy tuikit system
        Self::run_with_legacy(options, source)
    }
    
    /// Run with legacy tuikit system (stable, always available)
    fn run_with_legacy(options: &SkimOptions, source: Option<SkimItemReceiver>) -> Option<SkimOutput> {
        let min_height = Skim::parse_height_string(&options.min_height);
        let height = Skim::parse_height_string(&options.height);

        let (tx, rx): (EventSender, EventReceiver) = channel();
        let term = Arc::new(
            Term::with_options(
                TermOptions::default()
                    .min_height(min_height)
                    .height(height)
                    .clear_on_exit(!options.no_clear)
                    .disable_alternate_screen(options.no_clear_start)
                    .clear_on_start(!options.no_clear_start)
                    .hold(options.select_1 || options.exit_0 || options.sync),
            )
            .unwrap(),
        );
        if !options.no_mouse {
            let _ = term.enable_mouse_support();
        }

        //------------------------------------------------------------------------------
        // input
        let mut input = input::Input::new();
        input.parse_keymaps(options.bind.iter().map(String::as_str));
        input.parse_expect_keys(options.expect.iter().map(String::as_str));

        let tx_clone = tx.clone();
        let term_clone = term.clone();
        let input_thread = thread::spawn(move || {
            loop {
                if let Ok(key) = term_clone.poll_event() {
                    if key == TermEvent::User(()) {
                        break;
                    }

                    let (key, action_chain) = input.translate_event(key);
                    for event in action_chain {
                        let _ = tx_clone.send((key, event));
                    }
                }
            }
        });

        //------------------------------------------------------------------------------
        // reader

        let reader = Reader::with_options(options).source(source);

        //------------------------------------------------------------------------------
        // model + previewer
        let mut model = Model::new(rx, tx, reader, term.clone(), options);
        let ret = model.start();
        let _ = term.send_event(TermEvent::User(())); // interrupt the input thread
        let _ = input_thread.join();
        ret
    }
    
    /// Run with experimental ratatui system (only available when ratatui-ui feature is enabled)
    #[cfg(feature = "ratatui-ui")]
    fn run_with_ratatui(options: &SkimOptions, source: Option<SkimItemReceiver>) -> Option<SkimOutput> {
        // Import ratatui-specific modules
        use crate::ui::UICoordinator;
        
        let mut ui_coordinator = match UICoordinator::new(options) {
            Ok(coordinator) => coordinator,
            Err(e) => {
                eprintln!("Failed to create ratatui UI coordinator: {}", e);
                return None;
            }
        };

        // Set item source if provided, otherwise create default source
        if let Some(source) = source {
            ui_coordinator.set_item_source(source);
        } else {
            // Create source like the legacy system does
            use std::io::{BufReader, IsTerminal};
            use crate::helper::item_reader::{SkimItemReader, SkimItemReaderOption};
            
            if !std::io::stdin().is_terminal() {
                // Read from stdin when piped
                let reader_opts = SkimItemReaderOption::default();
                let item_reader = SkimItemReader::new(reader_opts);
                let stdin_source = item_reader.of_bufread(BufReader::new(std::io::stdin()));
                ui_coordinator.set_item_source(stdin_source);
            } else {
                // Use default command when no input (like legacy system)
                let default_command = match std::env::var("SKIM_DEFAULT_COMMAND").as_ref().map(String::as_ref) {
                    Ok("") | Err(_) => "find .".to_owned(),
                    Ok(val) => val.to_owned(),
                };
                
                // Use the cmd_collector to execute the default command
                let components_to_stop = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
                let (cmd_source, _tx_interrupt) = options.cmd_collector.borrow_mut().invoke(&default_command, components_to_stop);
                ui_coordinator.set_item_source(cmd_source);
            }
        }

        // Run the ratatui event loop
        match ui_coordinator.run() {
            Ok(output) => Some(output),
            Err(e) => {
                eprintln!("Ratatui UI coordinator failed: {}", e);
                None
            }
        }
    }

    /// Converts a &str to a TermHeight, based on whether or not it ends with a percent sign
    ///
    /// Will clamp percentages into [0, 100] and fixed into [0, MAX_USIZE]
    /// 10 -> TermHeight::Fixed(10)
    /// 10% -> TermHeight::Percent(10)
    fn parse_height_string(string: &str) -> TermHeight {
        if string.ends_with('%') {
            let inner = string[0..string.len() - 1].parse().unwrap_or(100);
            TermHeight::Percent(inner.clamp(0, 100))
        } else {
            let inner = string.parse().unwrap_or(0);
            TermHeight::Fixed(inner)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_height_string_fixed() {
        let TermHeight::Fixed(h) = Skim::parse_height_string("10") else {
            panic!("Expected fixed, found percent");
        };
        assert_eq!(h, 10)
    }
    #[test]
    fn parse_height_string_percent() {
        let TermHeight::Percent(h) = Skim::parse_height_string("10%") else {
            panic!("Expected percent, found fixed");
        };
        assert_eq!(h, 10)
    }
    #[test]
    fn parse_height_string_percent_neg() {
        let TermHeight::Percent(h) = Skim::parse_height_string("-20%") else {
            panic!("Expected fixed, found percent");
        };
        assert_eq!(h, 100)
    }
    #[test]
    fn parse_height_string_percent_too_large() {
        let TermHeight::Percent(h) = Skim::parse_height_string("120%") else {
            panic!("Expected percent, found fixed");
        };
        assert_eq!(h, 100)
    }
    #[test]
    fn parse_height_string_fixed_neg() {
        let TermHeight::Fixed(h) = Skim::parse_height_string("-20") else {
            panic!("Expected fixed, found percent");
        };
        assert_eq!(h, 0)
    }
}
