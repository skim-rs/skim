//! Fuzzy matching algorithms and implementations.
//!
//! This module provides different fuzzy matching algorithms including
//! skim's own algorithm and clangd's algorithm for matching text patterns.

/// Clangd fuzzy matching algorithm
pub mod clangd;
/// Skim fuzzy matching algorithm
pub mod skim;
mod util;

#[cfg(not(feature = "compact_matcher"))]
type IndexType = usize;
#[cfg(not(feature = "compact_matcher"))]
type ScoreType = i64;

#[cfg(feature = "compact_matcher")]
type IndexType = u32;
#[cfg(feature = "compact_matcher")]
type ScoreType = i32;

/// Trait for fuzzy matching text patterns against choices
pub trait FuzzyMatcher: Send + Sync {
    /// fuzzy match choice with pattern, and return the score & matched indices of characters
    fn fuzzy_indices(&self, choice: &str, pattern: &str) -> Option<(ScoreType, Vec<IndexType>)>;

    /// fuzzy match choice with pattern, and return the score of matching
    fn fuzzy_match(&self, choice: &str, pattern: &str) -> Option<ScoreType> {
        self.fuzzy_indices(choice, pattern).map(|(score, _)| score)
    }
}
