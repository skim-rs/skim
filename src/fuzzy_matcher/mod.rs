//! Fuzzy matching algorithms and implementations.
//!
//! This module provides different fuzzy matching algorithms including
//! skim's own algorithm and clangd's algorithm for matching text patterns.

/// Arinae fuzzy matching algorithm (Smith-Waterman with affine gaps)
pub mod arinae;
/// Clangd fuzzy matching algorithm
pub mod clangd;
#[cfg(feature = "frizbee")]
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

    /// Regression test for a fuzzer-found panic (fuzz target `fuzzy_match`).
    ///
    /// 'İ' (U+0130) lowercases to two chars, and `char_equal` used to be
    /// asymmetric for such characters. `cheap_matches` compares
    /// `(choice, pattern)` while the matchers' `allow_match` helpers compare
    /// `(pattern, choice)`, so the cheap pre-filter accepted a candidate the DP
    /// then refused to match. The clangd matcher's backtracking loop walked off
    /// the start of its matrix, panicking with "attempt to subtract with
    /// overflow" in debug and an out-of-bounds index in release.
    #[test]
    fn multichar_lowercase_does_not_panic() {
        use crate::fuzzy_matcher::clangd::ClangdMatcher;
        use crate::fuzzy_matcher::fzy::FzyMatcher;
        use crate::fuzzy_matcher::skim::SkimMatcherV2;

        let skim = SkimMatcherV2::default();
        let fzy = FzyMatcher::default();
        let clangd = ClangdMatcher::default();
        let matchers: [(&str, &dyn FuzzyMatcher); 3] = [("skim", &skim), ("fzy", &fzy), ("clangd", &clangd)];

        // The exact crashing input from the fuzz artifact, plus related shapes.
        let cases = [
            ("Jİ:I", "İ:İ"),
            ("I", "İ"),
            ("İ", "I"),
            ("i", "İ"),
            ("İ", "i"),
            ("Jİ:Iİ", "İİ"),
            ("straße", "STRASSE"),
            ("ﬄy", "ffl"),
        ];

        for (choice, pattern) in cases {
            let num_chars = choice.chars().count();
            for (name, matcher) in matchers {
                // Must not panic, and any returned index must be a valid char
                // index into `choice` (the invariant asserted by the fuzzer).
                if let Some((_score, indices)) = matcher.fuzzy_indices(choice, pattern) {
                    for idx in indices {
                        assert!(
                            idx < num_chars,
                            "{name}: match index {idx} out of bounds for {choice:?} ({num_chars} chars)"
                        );
                    }
                }
            }
        }
    }
}
