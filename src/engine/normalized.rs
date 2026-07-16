//! Normalized match engine for matching with Unicode normalization (removing diacritics).
//!
//! This engine wraps another engine and normalizes both the query and item text before matching,
//! then maps the results back to the original text.

use std::borrow::Cow;
use std::fmt::{Display, Error, Formatter};

use crate::engine::util::{
    map_byte_range_to_original, map_char_indices_to_original, normalize_with_byte_mapping, normalize_with_char_mapping,
};
use crate::{CaseMatching, MatchEngine, MatchEngineFactory, MatchRange, MatchResult, SkimItem};

/// Engine that normalizes text before matching
pub struct NormalizedEngine {
    /// The underlying engine to match normalized text
    inner: Box<dyn MatchEngine>,
}

impl NormalizedEngine {
    /// Creates a new normalized match engine
    pub fn new(inner: Box<dyn MatchEngine>) -> Self {
        Self { inner }
    }
}

impl MatchEngine for NormalizedEngine {
    fn match_item(&self, item: &dyn SkimItem) -> Option<MatchResult> {
        let item_text = item.text();

        // Normalize the item text
        let (normalized_text, char_mapping) = normalize_with_char_mapping(&item_text);
        let (_, byte_mapping) = normalize_with_byte_mapping(&item_text);

        // Create a wrapper item with normalized text
        let normalized_item: &dyn SkimItem = &NormalizedItem(normalized_text);

        // Match using the inner engine
        let mut result = self.inner.match_item(normalized_item)?;

        // Map the matched range back to the original text
        result.matched_range = match result.matched_range {
            MatchRange::Chars(indices) => MatchRange::Chars(map_char_indices_to_original(&indices, &char_mapping)),
            MatchRange::CharRange(start, end) => {
                let orig_start = char_mapping.get(start).copied().unwrap_or(start);
                let orig_end = if end > 0 {
                    char_mapping.get(end - 1).copied().map_or(end, |e| e + 1)
                } else {
                    0
                };
                MatchRange::CharRange(orig_start, orig_end)
            }
            MatchRange::ByteRange(start, end) => {
                let (orig_start, orig_end) = map_byte_range_to_original(start, end, &byte_mapping, &item_text);
                MatchRange::ByteRange(orig_start, orig_end)
            }
        };

        Some(result)
    }
}

impl Display for NormalizedEngine {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "(Normalized: {})", self.inner)
    }
}

/// Simple string wrapper implementing `SkimItem` for normalized matching
struct NormalizedItem(String);

impl SkimItem for NormalizedItem {
    fn text(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.0)
    }
}

//------------------------------------------------------------------------------
// NormalizedEngineFactory - wraps another factory and handles normalization

/// Factory that handles normalization by wrapping another engine factory
pub struct NormalizedEngineFactory {
    inner: Box<dyn MatchEngineFactory>,
}

impl NormalizedEngineFactory {
    /// Creates a new normalized engine factory
    pub fn new(inner: impl MatchEngineFactory + 'static) -> Self {
        Self { inner: Box::new(inner) }
    }
}

impl MatchEngineFactory for NormalizedEngineFactory {
    fn create_engine_with_case(&self, query: &str, case: CaseMatching) -> Box<dyn MatchEngine> {
        // Normalize the query
        let (normalized_query, _) = normalize_with_char_mapping(query);

        // Create the inner engine with the normalized query
        let inner_engine = self.inner.create_engine_with_case(&normalized_query, case);

        // Wrap it in a NormalizedEngine
        Box::new(NormalizedEngine::new(inner_engine))
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    use super::*;
    use crate::engine::exact::{ExactEngine, ExactMatchingParam};
    use crate::prelude::ExactOrFuzzyEngineFactory;

    #[test]
    fn matches_through_diacritics() {
        // Inner exact engine searches for the ASCII form; the normalized engine
        // strips the accent from the item text before matching.
        let inner = Box::new(ExactEngine::builder("cafe", ExactMatchingParam::default()).build());
        let engine = NormalizedEngine::new(inner);
        let result = engine.match_item(&"café".to_string());
        assert!(result.is_some());
    }

    #[test]
    fn no_match_returns_none() {
        let inner = Box::new(ExactEngine::builder("zzz", ExactMatchingParam::default()).build());
        let engine = NormalizedEngine::new(inner);
        assert!(engine.match_item(&"café".to_string()).is_none());
    }

    #[test]
    fn display_includes_inner_engine() {
        let inner = Box::new(ExactEngine::builder("x", ExactMatchingParam::default()).build());
        let engine = NormalizedEngine::new(inner);
        assert!(format!("{engine}").starts_with("(Normalized:"));
    }

    #[test]
    fn factory_creates_normalized_engine() {
        let factory = NormalizedEngineFactory::new(ExactOrFuzzyEngineFactory::builder().build());
        let engine = factory.create_engine_with_case("cafe", CaseMatching::Smart);
        // The accented item should match the normalized query.
        assert!(engine.match_item(&"café".to_string()).is_some());
    }

    /// Inner engine that always returns a fixed `CharRange`, so the normalized
    /// engine's `CharRange` remapping branch is exercised.
    struct CharRangeStub(usize, usize);

    impl MatchEngine for CharRangeStub {
        fn match_item(&self, _item: &dyn SkimItem) -> Option<MatchResult> {
            Some(MatchResult {
                rank: crate::Rank::default(),
                matched_range: MatchRange::CharRange(self.0, self.1),
            })
        }
    }

    impl Display for CharRangeStub {
        fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
            write!(f, "CharRangeStub")
        }
    }

    #[test]
    fn char_range_is_mapped_back_to_original() {
        // café → cafe is a 1:1 normalization, so the char range is unchanged.
        let engine = NormalizedEngine::new(Box::new(CharRangeStub(1, 3)));
        let result = engine.match_item(&"café".to_string()).unwrap();
        assert_eq!(result.matched_range, MatchRange::CharRange(1, 3));
    }

    #[test]
    fn empty_char_range_maps_to_zero() {
        // An empty range (end == 0) maps straight back to (0, 0).
        let engine = NormalizedEngine::new(Box::new(CharRangeStub(0, 0)));
        let result = engine.match_item(&"café".to_string()).unwrap();
        assert_eq!(result.matched_range, MatchRange::CharRange(0, 0));
    }

    /// Inner engine returning fixed `Chars` indices, exercising the
    /// `map_char_indices_to_original` remapping branch.
    struct CharsStub(Vec<usize>);

    impl MatchEngine for CharsStub {
        fn match_item(&self, _item: &dyn SkimItem) -> Option<MatchResult> {
            Some(MatchResult {
                rank: crate::Rank::default(),
                matched_range: MatchRange::Chars(self.0.clone()),
            })
        }
    }

    impl Display for CharsStub {
        fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
            write!(f, "CharsStub")
        }
    }

    #[test]
    fn chars_indices_are_mapped_back_to_original() {
        // 1:1 normalization (café → cafe) keeps the char indices unchanged.
        let engine = NormalizedEngine::new(Box::new(CharsStub(vec![0, 2])));
        let result = engine.match_item(&"café".to_string()).unwrap();
        assert_eq!(result.matched_range, MatchRange::Chars(vec![0, 2]));
    }
}
