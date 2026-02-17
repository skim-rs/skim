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
use std::fmt::Display;
use std::sync::Arc;

use ratatui::{
    style::Style,
    text::{Line, Span},
};

pub use crate::engine::fuzzy::FuzzyAlgorithm;
pub use crate::item::RankCriteria;
pub use crate::options::SkimOptions;
pub use crate::output::SkimOutput;
pub use crate::skim::*;
pub use crate::skim_item::SkimItem;
use crate::tui::Size;

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
mod skim;
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

/// Position and scroll information for preview display
#[derive(Default, Copy, Clone, Debug)]
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

/// Case sensitivity mode for matching
#[derive(Eq, PartialEq, Debug, Copy, Clone, Default)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum), clap(rename_all = "snake_case"))]
pub enum CaseMatching {
    /// Case-sensitive matching
    Respect,
    /// Case-insensitive matching
    Ignore,
    /// Smart case: case-insensitive unless query contains uppercase
    #[default]
    Smart,
}

/// Represents the range of a match in an item
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum MatchRange {
    /// Range of bytes (start, end)
    ByteRange(usize, usize),
    /// Individual character indices that matched
    Chars(Vec<usize>),
}

/// Rank tuple used for sorting match results
/// The field will be ordered based on the `tiebreak` parameter
pub type Rank = [i32; 5];

/// Result of matching a query against an item
#[derive(Clone)]
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
