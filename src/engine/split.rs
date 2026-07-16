//! Split match engine for matching against different parts of items based on a delimiter.
//!
//! This engine splits both the query and item text on a delimiter character, then matches
//! the query parts against the corresponding item parts.

use crate::fuzzy_matcher::MatchIndices;
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

        let mut combined_indices: MatchIndices = match before_result.matched_range {
            MatchRange::Chars(indices) => indices,
            MatchRange::CharRange(start, end) => (start..end).collect(),
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

        let after_indices: MatchIndices = match after_result.matched_range {
            MatchRange::Chars(indices) => indices.into_iter().map(|i| i + offset).collect(),
            MatchRange::CharRange(start, end) => (start..end).map(|i| i + offset).collect(),
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
        combined_indices.sort_unstable();
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

/// Simple string wrapper implementing `SkimItem` for split matching
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

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    use super::*;
    use crate::engine::exact::{ExactEngine, ExactMatchingParam};
    use crate::prelude::ExactOrFuzzyEngineFactory;

    /// A stub engine that returns a fixed match range, regardless of the item.
    struct StubEngine(MatchRange);

    impl MatchEngine for StubEngine {
        fn match_item(&self, _item: &dyn SkimItem) -> Option<MatchResult> {
            Some(MatchResult {
                rank: crate::Rank::default(),
                matched_range: self.0.clone(),
            })
        }
    }

    impl Display for StubEngine {
        fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
            write!(f, "Stub")
        }
    }

    fn exact(query: &str) -> Box<dyn MatchEngine> {
        Box::new(ExactEngine::builder(query, ExactMatchingParam::default()).build())
    }

    #[test]
    fn no_delimiter_in_item_returns_none() {
        let engine = SplitMatchEngine::new(exact("a"), exact("b"), ':');
        assert!(engine.match_item(&"no delimiter here".to_string()).is_none());
    }

    #[test]
    fn matches_both_sides_of_delimiter() {
        let engine = SplitMatchEngine::new(exact("ab"), exact("cd"), ':');
        let result = engine.match_item(&"ab:cd".to_string());
        assert!(result.is_some());
    }

    #[test]
    fn char_range_results_are_offset_and_combined() {
        // Before matches chars 0..2 of "ab"; after matches chars 0..2 of "cd"
        // which become 3..5 after the delimiter offset.
        let engine = SplitMatchEngine::new(
            Box::new(StubEngine(MatchRange::CharRange(0, 2))),
            Box::new(StubEngine(MatchRange::CharRange(0, 2))),
            ':',
        );
        let result = engine.match_item(&"ab:cd".to_string()).unwrap();
        assert_eq!(result.matched_range, MatchRange::Chars(vec![0, 1, 3, 4]));
    }

    #[test]
    fn after_engine_failure_returns_none() {
        let engine = SplitMatchEngine::new(exact("ab"), exact("zzz"), ':');
        assert!(engine.match_item(&"ab:cd".to_string()).is_none());
    }

    #[test]
    fn byte_range_results_drop_chars_outside_range() {
        // ByteRange(1, 2) over the three-char "abc"/"def" parts covers only the
        // middle byte: 'a'/'d' (byte 0) fail the `>= start` check, 'c'/'f'
        // (byte 2) fail the `< end` check, so only 'b'/'e' survive.
        let engine = SplitMatchEngine::new(
            Box::new(StubEngine(MatchRange::ByteRange(1, 2))),
            Box::new(StubEngine(MatchRange::ByteRange(1, 2))),
            ':',
        );
        let result = engine.match_item(&"abc:def".to_string()).unwrap();
        // before: char 1 ('b'); after: char 1 of "def" offset past "abc:" → char 5 ('e').
        assert_eq!(result.matched_range, MatchRange::Chars(vec![1, 5]));
    }

    #[test]
    fn byte_range_results_include_chars_within_range() {
        // ByteRange(0, 2) covers both chars of each part, so nothing is dropped.
        let engine = SplitMatchEngine::new(
            Box::new(StubEngine(MatchRange::ByteRange(0, 2))),
            Box::new(StubEngine(MatchRange::ByteRange(0, 2))),
            ':',
        );
        let result = engine.match_item(&"ab:cd".to_string()).unwrap();
        assert_eq!(result.matched_range, MatchRange::Chars(vec![0, 1, 3, 4]));
    }

    #[test]
    fn display_shows_both_engines_and_delimiter() {
        let engine = SplitMatchEngine::new(exact("a"), exact("b"), ':');
        let s = format!("{engine}");
        assert!(s.starts_with("(Split[:]:"));
    }

    #[test]
    fn factory_without_delimiter_passes_through() {
        let factory = SplitMatchEngineFactory::new(ExactOrFuzzyEngineFactory::builder().build(), ':');
        let engine = factory.create_engine_with_case("foo", crate::CaseMatching::Smart);
        // Plain query, no delimiter → behaves like the inner engine.
        assert!(engine.match_item(&"foobar".to_string()).is_some());
    }

    #[test]
    fn factory_with_delimiter_builds_split_engine() {
        let factory = SplitMatchEngineFactory::new(ExactOrFuzzyEngineFactory::builder().build(), ':');
        let engine = factory.create_engine_with_case("ab:cd", crate::CaseMatching::Smart);
        assert!(engine.match_item(&"ab:cd".to_string()).is_some());
        assert!(engine.match_item(&"ab:xy".to_string()).is_none());
    }
}
