//! Fuzzy matching algorithms and implementations.
//!
//! This module provides different fuzzy matching algorithms including
//! skim's own algorithm and clangd's algorithm for matching text patterns.

/// Clangd fuzzy matching algorithm
pub mod clangd;
#[cfg(feature = "nightly-frizbee")]
pub mod frizbee;
/// Skim fuzzy matching algorithm
pub mod skim;
mod util;

pub(crate) type IndexType = usize;
pub(crate) type ScoreType = i64;

/// Trait for fuzzy matching text patterns against choices
pub trait FuzzyMatcher: Send + Sync {
    /// fuzzy match choice with pattern, and return the score & matched indices of characters
    fn fuzzy_indices(&self, choice: &str, pattern: &str) -> Option<(i64, Vec<usize>)>;

    /// fuzzy match choice with pattern, and return the score of matching
    fn fuzzy_match(&self, choice: &str, pattern: &str) -> Option<i64> {
        self.fuzzy_indices(choice, pattern).map(|(score, _)| score)
    }
}
