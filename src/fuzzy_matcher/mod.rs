//! Fuzzy matching algorithms and implementations.
//!
//! This module provides different fuzzy matching algorithms including
//! skim's own algorithm and clangd's algorithm for matching text patterns.

/// Clangd fuzzy matching algorithm
pub mod clangd;
pub mod frizbee;
/// Fzy fuzzy matching algorithm
pub mod fzy;
/// Skim fuzzy matching algorithm
pub mod skim;
/// SkimV3 fuzzy matching algorithm (Smith-Waterman with affine gaps)
pub mod skim_v3;
mod util;

pub(crate) type IndexType = usize;
pub(crate) type ScoreType = i64;

/// Stack-allocated match indices. 4 elements covers patterns up to 4 chars
/// without heap allocation (same as one `Vec` header: pointer + len + cap).
pub type MatchIndices = Vec<IndexType>;

/// Trait for fuzzy matching text patterns against choices
pub trait FuzzyMatcher: Send + Sync {
    /// fuzzy match choice with pattern, and return the score & matched indices of characters
    fn fuzzy_indices(&self, choice: &str, pattern: &str) -> Option<(i64, MatchIndices)>;

    /// fuzzy match choice with pattern, and return the score of matching
    fn fuzzy_match(&self, choice: &str, pattern: &str) -> Option<i64> {
        self.fuzzy_indices(choice, pattern).map(|(score, _)| score)
    }

    /// Fuzzy match and return (score, begin_char_index, end_char_index) without
    /// computing per-character match indices. This avoids the Vec allocation and
    /// traceback that `fuzzy_indices` requires, making it much faster for ranking.
    ///
    /// `begin` is the character index of the first matched pattern character,
    /// `end` is the character index of the last matched pattern character.
    ///
    /// Default implementation falls back to `fuzzy_indices`.
    fn fuzzy_match_range(&self, choice: &str, pattern: &str) -> Option<(i64, usize, usize)> {
        self.fuzzy_indices(choice, pattern).map(|(score, indices)| {
            let begin = indices.first().copied().unwrap_or(0);
            let end = indices.last().copied().unwrap_or(0);
            (score, begin, end)
        })
    }
}
