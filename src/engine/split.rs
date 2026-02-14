//! Split match engine for matching against different parts of items based on a delimiter.
//!
//! This engine splits both the query and item text on a delimiter character, then matches
//! the query parts against the corresponding item parts.

use crate::{MatchEngine, MatchEngineFactory, MatchRange, MatchResult, SkimItem};
use std::fmt::{Display, Error, Formatter};

/// Engine that matches by splitting query and item on a delimiter
pub struct SplitMatchEngine {
    /// The engine to match the "before delimiter" part
    before_engine: Box<dyn MatchEngine>,
    /// The engine to match the "after delimiter" part  
    after_engine: Box<dyn MatchEngine>,
    /// The delimiter character used for splitting
    delimiter: char,
}

impl SplitMatchEngine {
    /// Creates a new split match engine
    pub fn new(before_engine: Box<dyn MatchEngine>, after_engine: Box<dyn MatchEngine>, delimiter: char) -> Self {
        Self {
            before_engine,
            after_engine,
            delimiter,
        }
    }
}

impl MatchEngine for SplitMatchEngine {
    fn match_item(&self, item: &dyn SkimItem) -> Option<MatchResult> {
        let text = item.text();

        // Find the delimiter in the item text (by char position)
        let delimiter_char_idx = text.chars().position(|c| c == self.delimiter)?;

        // Get byte position for slicing
        let delimiter_byte_pos = text.char_indices().nth(delimiter_char_idx).map(|(i, _)| i)?;

        let text_before = &text[..delimiter_byte_pos];
        let text_after = &text[delimiter_byte_pos + self.delimiter.len_utf8()..];

        // Create wrapper items for each part
        let before_item: &dyn SkimItem = &StringItem(text_before.to_string());
        let after_item: &dyn SkimItem = &StringItem(text_after.to_string());

        // Match both parts
        let before_result = self.before_engine.match_item(before_item)?;
        let after_result = self.after_engine.match_item(after_item)?;

        // Combine the results - use rank from first result (like AndEngine does)
        let rank = before_result.rank;

        let mut combined_indices: Vec<usize> = match before_result.matched_range {
            MatchRange::Chars(indices) => indices,
            MatchRange::ByteRange(start, end) => {
                // Convert byte range to char indices for the before part
                text_before
                    .char_indices()
                    .enumerate()
                    .filter(|(_, (byte_idx, _))| *byte_idx >= start && *byte_idx < end)
                    .map(|(char_idx, _)| char_idx)
                    .collect()
            }
        };

        // Offset for the "after" part: delimiter_char_idx + 1 (to skip the delimiter)
        let offset = delimiter_char_idx + 1;

        let after_indices: Vec<usize> = match after_result.matched_range {
            MatchRange::Chars(indices) => indices.into_iter().map(|i| i + offset).collect(),
            MatchRange::ByteRange(start, end) => {
                // Convert byte range to char indices for the after part
                text_after
                    .char_indices()
                    .enumerate()
                    .filter(|(_, (byte_idx, _))| *byte_idx >= start && *byte_idx < end)
                    .map(|(char_idx, _)| char_idx + offset)
                    .collect()
            }
        };

        combined_indices.extend(after_indices);
        combined_indices.sort();
        combined_indices.dedup();

        Some(MatchResult {
            rank,
            matched_range: MatchRange::Chars(combined_indices),
        })
    }
}

impl Display for SplitMatchEngine {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(
            f,
            "(Split[{}]: {} | {})",
            self.delimiter, self.before_engine, self.after_engine
        )
    }
}

/// Simple string wrapper implementing SkimItem for split matching
struct StringItem(String);

impl SkimItem for StringItem {
    fn text(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Borrowed(&self.0)
    }
}

//------------------------------------------------------------------------------
// SplitMatchEngineFactory - wraps another factory and handles split matching

/// Factory that handles split matching by wrapping another engine factory
pub struct SplitMatchEngineFactory {
    inner: Box<dyn MatchEngineFactory>,
    delimiter: char,
}

impl SplitMatchEngineFactory {
    /// Creates a new split match engine factory
    pub fn new(inner: impl MatchEngineFactory + 'static, delimiter: char) -> Self {
        Self {
            inner: Box::new(inner),
            delimiter,
        }
    }
}

impl MatchEngineFactory for SplitMatchEngineFactory {
    fn create_engine_with_case(&self, query: &str, case: crate::CaseMatching) -> Box<dyn MatchEngine> {
        // Check if the query contains the delimiter
        if let Some(delimiter_pos) = query.find(self.delimiter) {
            let query_before = &query[..delimiter_pos];
            let query_after = &query[delimiter_pos + self.delimiter.len_utf8()..];

            // Create engines for each part using the inner factory
            let before_engine = self.inner.create_engine_with_case(query_before, case);
            let after_engine = self.inner.create_engine_with_case(query_after, case);

            Box::new(SplitMatchEngine::new(before_engine, after_engine, self.delimiter))
        } else {
            // No delimiter in query, pass through to inner factory
            self.inner.create_engine_with_case(query, case)
        }
    }
}
