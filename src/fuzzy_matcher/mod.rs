//! Fuzzy matching algorithms and implementations.
//!
//! This module provides different fuzzy matching algorithms including
//! skim's own algorithm and clangd's algorithm for matching text patterns.

/// Arinae fuzzy matching algorithm (Smith-Waterman with affine gaps)
pub mod arinae;
/// Clangd fuzzy matching algorithm
pub mod clangd;
#[cfg(frizbee)]
pub mod frizbee;
/// Fzy fuzzy matching algorithm
pub mod fzy;
/// Skim fuzzy matching algorithm
pub mod skim;
mod util;

pub(crate) type IndexType = usize;
pub(crate) type ScoreType = i64;

pub(crate) type MatchIndices = Vec<IndexType>;

/// Trait for fuzzy matching text patterns against choices
pub trait FuzzyMatcher: Send + Sync {
    /// fuzzy match choice with pattern, and return the score & matched indices of characters
    fn fuzzy_indices(&self, choice: &str, pattern: &str) -> Option<(i64, MatchIndices)>;

    /// fuzzy match choice with pattern, and return the score of matching
    fn fuzzy_match(&self, choice: &str, pattern: &str) -> Option<i64> {
        self.fuzzy_indices(choice, pattern).map(|(score, _)| score)
    }

    /// Fuzzy match and return (score, `begin_char_index`, `end_char_index`) without
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

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    use super::*;

    /// A matcher that only implements `fuzzy_indices`, so it exercises the
    /// default `fuzzy_match` / `fuzzy_match_range` implementations.
    struct StubMatcher;

    impl FuzzyMatcher for StubMatcher {
        fn fuzzy_indices(&self, choice: &str, pattern: &str) -> Option<(i64, MatchIndices)> {
            if pattern.is_empty() {
                return Some((0, vec![]));
            }
            // Match only when the pattern is a prefix of the choice.
            choice
                .starts_with(pattern)
                .then(|| (10, (0..pattern.chars().count()).collect()))
        }
    }

    #[test]
    fn default_fuzzy_match_uses_indices_score() {
        assert_eq!(StubMatcher.fuzzy_match("hello", "he"), Some(10));
        assert_eq!(StubMatcher.fuzzy_match("hello", "xy"), None);
    }

    #[test]
    fn default_fuzzy_match_range_spans_first_to_last() {
        assert_eq!(StubMatcher.fuzzy_match_range("hello", "hel"), Some((10, 0, 2)));
        assert_eq!(StubMatcher.fuzzy_match_range("hello", "zz"), None);
    }

    #[test]
    fn default_fuzzy_match_range_empty_indices_default_to_zero() {
        // Empty pattern yields an empty index list, so begin/end fall back to 0.
        assert_eq!(StubMatcher.fuzzy_match_range("hello", ""), Some((0, 0, 0)));
    }
}
