//! Normalized match engine for matching with Unicode normalization (removing diacritics).
//!
//! This engine wraps another engine and normalizes both the query and item text before matching,
//! then maps the results back to the original text.

use std::borrow::Cow;
use std::fmt::{Display, Error, Formatter};
use std::sync::Arc;

use crate::engine::util::{map_byte_range_to_original, map_char_indices_to_original};
use crate::engine::util::{normalize_with_byte_mapping, normalize_with_char_mapping};
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
    fn match_item(&self, item: Arc<dyn SkimItem>) -> Option<MatchResult> {
        let item_text = item.text();

        // Normalize the item text
        let (normalized_text, char_mapping) = normalize_with_char_mapping(&item_text);
        let (_, byte_mapping) = normalize_with_byte_mapping(&item_text);

        // Create a wrapper item with normalized text
        let normalized_item: Arc<dyn SkimItem> = Arc::new(NormalizedItem(normalized_text));

        // Match using the inner engine
        let mut result = self.inner.match_item(normalized_item)?;

        // Map the matched range back to the original text
        result.matched_range = match result.matched_range {
            MatchRange::Chars(indices) => MatchRange::Chars(map_char_indices_to_original(&indices, &char_mapping)),
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

/// Simple string wrapper implementing SkimItem for normalized matching
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
