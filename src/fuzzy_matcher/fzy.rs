//! Fuzzy matching algorithm based on fzy by John Hawthorn.
//! https://github.com/jhawthorn/fzy
//!
//! This implements fzy's scoring algorithm which treats fuzzy matching as a
//! modified edit-distance problem using dynamic programming (Needleman-Wunsch style).
//! It uses two DP matrices:
//! - `M[i][j]`: the best possible score using the first `i` chars of the needle
//!   and the first `j` chars of the haystack.
//! - `D[i][j]`: the best score that *ends with a match* at position `(i, j)`.
//!
//! This separation enables affine gap penalties: a constant cost to open a gap
//! and a linear cost for extending it, plus a bonus for consecutive matches.
//!
//! # Example:
//! ```
//! use skim::fuzzy_matcher::FuzzyMatcher;
//! use skim::fuzzy_matcher::fzy::FzyMatcher;
//!
//! let matcher = FzyMatcher::default();
//!
//! assert_eq!(None, matcher.fuzzy_match("abc", "abx"));
//! assert!(matcher.fuzzy_match("axbycz", "abc").is_some());
//!
//! let (score, indices) = matcher.fuzzy_indices("axbycz", "abc").unwrap();
//! assert_eq!(indices, [0, 2, 4]);
//! ```

use std::cell::RefCell;

use thread_local::ThreadLocal;

use crate::fuzzy_matcher::util::cheap_matches;
use crate::fuzzy_matcher::{FuzzyMatcher, IndexType, ScoreType};

// ---------------------------------------------------------------------------
// Score constants (from fzy's config.def.h)
// ---------------------------------------------------------------------------
// fzy uses f64 scores internally. We convert to i64 at the boundary by scaling.

const SCORE_MIN: f64 = f64::NEG_INFINITY;
const SCORE_MAX: f64 = f64::INFINITY;

const SCORE_GAP_LEADING: f64 = -0.005;
const SCORE_GAP_TRAILING: f64 = -0.005;
const SCORE_GAP_INNER: f64 = -0.01;

const SCORE_MATCH_CONSECUTIVE: f64 = 1.0;
const SCORE_MATCH_SLASH: f64 = 0.9;
const SCORE_MATCH_WORD: f64 = 0.8;
const SCORE_MATCH_CAPITAL: f64 = 0.7;
const SCORE_MATCH_DOT: f64 = 0.6;

/// Maximum haystack length we will score. Longer candidates still match but
/// receive SCORE_MIN so they sort below reasonably-sized candidates.
const MATCH_MAX_LEN: usize = 1024;

// ---------------------------------------------------------------------------
// Bonus computation
// ---------------------------------------------------------------------------

/// Classifies a character into one of three bonus groups that determine
/// what bonus the *next* character receives when it matches.
///
/// Group 0 = no special bonus (default / non-word non-separator)
/// Group 1 = lowercase letter or digit (the "preceding char is alphanumeric" case)
/// Group 2 = uppercase letter (the "preceding char is upper" case — enables capital bonus)
#[inline]
fn bonus_index(ch: char) -> usize {
    if ch.is_ascii_uppercase() {
        2
    } else if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
        1
    } else {
        0
    }
}

/// Returns the bonus score for matching `ch` when the previous character in
/// the haystack is `prev_ch`.
///
/// The bonus table encodes:
/// - After a `/`: slash bonus (path separator)
/// - After `-`, `_`, ` `: word boundary bonus
/// - After `.`: dot bonus (file extensions)
/// - Uppercase letter after a lowercase letter: capital/camelCase bonus
#[inline]
fn compute_bonus(prev_ch: char, ch: char) -> f64 {
    match bonus_index(ch) {
        // Non-alphanumeric character being matched — no bonus from context
        0 => 0.0,
        // Lowercase / digit being matched
        1 => match prev_ch {
            '/' => SCORE_MATCH_SLASH,
            '-' | '_' | ' ' => SCORE_MATCH_WORD,
            '.' => SCORE_MATCH_DOT,
            _ => 0.0,
        },
        // Uppercase letter being matched — same as group 1 but also gets
        // a capital bonus when preceded by a lowercase letter
        2 => match prev_ch {
            '/' => SCORE_MATCH_SLASH,
            '-' | '_' | ' ' => SCORE_MATCH_WORD,
            '.' => SCORE_MATCH_DOT,
            c if c.is_ascii_lowercase() => SCORE_MATCH_CAPITAL,
            _ => 0.0,
        },
        _ => unreachable!(),
    }
}

// ---------------------------------------------------------------------------
// Core DP matching
// ---------------------------------------------------------------------------

/// Precompute the bonus for each position in the haystack.
fn precompute_bonus(haystack: &[char]) -> Vec<f64> {
    let mut bonuses = Vec::with_capacity(haystack.len());
    let mut prev = '/'; // treat start-of-string like after a path separator
    for &ch in haystack {
        bonuses.push(compute_bonus(prev, ch));
        prev = ch;
    }
    bonuses
}

/// Compute the fzy score for `needle` against `haystack`.
///
/// Returns `None` if the needle does not match.
/// When `positions` is `Some`, it will be filled with the matched indices.
fn fzy_score(
    needle: &[char],
    haystack: &[char],
    case_sensitive: bool,
    positions: Option<&mut Vec<IndexType>>,
) -> Option<f64> {
    let n = needle.len();
    let m = haystack.len();

    if n == 0 {
        return None;
    }

    if m > MATCH_MAX_LEN || n > m {
        return None;
    }

    // Special case: if lengths are equal (and we know it matches from the
    // cheap_matches pre-check), it must be an exact match.
    if n == m {
        if let Some(pos) = positions {
            pos.clear();
            pos.extend(0..n);
        }
        return Some(SCORE_MAX);
    }

    // Lowercase versions for case-insensitive comparison
    let lower_needle: Vec<char> = needle.iter().map(|c| c.to_ascii_lowercase()).collect();
    let lower_haystack: Vec<char> = haystack.iter().map(|c| c.to_ascii_lowercase()).collect();

    let match_bonus = precompute_bonus(haystack);

    // When we need positions, we must keep the full n×m matrices.
    // When we only need the score, we can use a rolling 1-row approach.
    if positions.is_some() {
        // Full matrix approach for position recovery
        let mut d_matrix: Vec<Vec<f64>> = vec![vec![SCORE_MIN; m]; n];
        let mut m_matrix: Vec<Vec<f64>> = vec![vec![SCORE_MIN; m]; n];

        // Fill row 0 (first needle character)
        {
            let mut prev_score = SCORE_MIN;
            for j in 0..m {
                if (case_sensitive && needle[0] == haystack[j])
                    || (!case_sensitive && lower_needle[0] == lower_haystack[j])
                {
                    let score = (j as f64) * SCORE_GAP_LEADING + match_bonus[j];
                    d_matrix[0][j] = score;
                    prev_score = score;
                    m_matrix[0][j] = score;
                } else {
                    d_matrix[0][j] = SCORE_MIN;
                    let gap = if n == 1 { SCORE_GAP_TRAILING } else { SCORE_GAP_INNER };
                    prev_score += gap;
                    m_matrix[0][j] = prev_score;
                }
            }
        }

        // Fill rows 1..n-1
        for i in 1..n {
            let mut prev_score = SCORE_MIN;
            let gap_score = if i == n - 1 {
                SCORE_GAP_TRAILING
            } else {
                SCORE_GAP_INNER
            };

            for j in 0..m {
                if (case_sensitive && needle[i] == haystack[j])
                    || (!case_sensitive && lower_needle[i] == lower_haystack[j])
                {
                    let mut score = SCORE_MIN;
                    if j > 0 {
                        // Previous row, previous column
                        let prev_m = m_matrix[i - 1][j - 1];
                        let prev_d = d_matrix[i - 1][j - 1];
                        score = f64::max(prev_m + match_bonus[j], prev_d + SCORE_MATCH_CONSECUTIVE);
                    }
                    d_matrix[i][j] = score;
                    prev_score = f64::max(score, prev_score + gap_score);
                    m_matrix[i][j] = prev_score;
                } else {
                    d_matrix[i][j] = SCORE_MIN;
                    prev_score += gap_score;
                    m_matrix[i][j] = prev_score;
                }
            }
        }

        let final_score = m_matrix[n - 1][m - 1];

        // Backtrace to find optimal match positions
        if let Some(pos) = positions {
            pos.clear();
            pos.resize(n, 0);

            let mut match_required = false;
            let mut j = m - 1;
            for i in (0..n).rev() {
                loop {
                    if d_matrix[i][j] != SCORE_MIN && (match_required || d_matrix[i][j] == m_matrix[i][j]) {
                        // Check if this was a consecutive match — if so, the
                        // previous needle char MUST also be a match at j-1.
                        match_required =
                            i > 0 && j > 0 && m_matrix[i][j] == d_matrix[i - 1][j - 1] + SCORE_MATCH_CONSECUTIVE;
                        pos[i] = j;
                        j = j.saturating_sub(1);
                        break;
                    }
                    if j == 0 {
                        break;
                    }
                    j -= 1;
                }
            }
        }

        Some(final_score)
    } else {
        // Score-only: rolling single row
        let mut d_row = vec![SCORE_MIN; m];
        let mut m_row = vec![SCORE_MIN; m];

        for i in 0..n {
            let mut prev_score = SCORE_MIN;
            let gap_score = if i == n - 1 {
                SCORE_GAP_TRAILING
            } else {
                SCORE_GAP_INNER
            };

            // We need the previous row's D and M values. In the rolling approach,
            // d_row and m_row currently hold the previous row. We overwrite in-place
            // using prev_d / prev_m to carry the diagonal values.
            let mut prev_d = SCORE_MIN;
            let mut prev_m = SCORE_MIN;

            for j in 0..m {
                let old_d = d_row[j];
                let old_m = m_row[j];

                if (case_sensitive && needle[i] == haystack[j])
                    || (!case_sensitive && lower_needle[i] == lower_haystack[j])
                {
                    let score = if i == 0 {
                        (j as f64) * SCORE_GAP_LEADING + match_bonus[j]
                    } else if j > 0 {
                        f64::max(prev_m + match_bonus[j], prev_d + SCORE_MATCH_CONSECUTIVE)
                    } else {
                        SCORE_MIN
                    };
                    d_row[j] = score;
                    prev_score = f64::max(score, prev_score + gap_score);
                    m_row[j] = prev_score;
                } else {
                    d_row[j] = SCORE_MIN;
                    prev_score += gap_score;
                    m_row[j] = prev_score;
                }

                prev_d = old_d;
                prev_m = old_m;
            }
        }

        Some(m_row[m - 1])
    }
}

/// Convert fzy's f64 score to skim's i64 score space.
/// We scale by 1000 to preserve three decimal places of precision.
#[inline]
fn f64_to_skim_score(score: f64) -> ScoreType {
    if score == SCORE_MAX {
        // Exact match — use a large sentinel
        i64::MAX / 2
    } else if score == SCORE_MIN || score.is_nan() {
        i64::MIN / 2
    } else {
        (score * 1000.0) as ScoreType
    }
}

// ---------------------------------------------------------------------------
// Public matcher struct
// ---------------------------------------------------------------------------

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
enum CaseMatching {
    Respect,
    Ignore,
    Smart,
}

/// Fuzzy matcher using the fzy algorithm.
///
/// This is a clean reimplementation of the scoring algorithm from
/// [fzy](https://github.com/jhawthorn/fzy) by John Hawthorn.
#[derive(Debug)]
pub struct FzyMatcher {
    case: CaseMatching,
    use_cache: bool,
    c_cache: ThreadLocal<RefCell<Vec<char>>>,
    p_cache: ThreadLocal<RefCell<Vec<char>>>,
}

impl Default for FzyMatcher {
    fn default() -> Self {
        Self {
            case: CaseMatching::Ignore,
            use_cache: true,
            c_cache: ThreadLocal::new(),
            p_cache: ThreadLocal::new(),
        }
    }
}

impl FzyMatcher {
    /// Sets the matcher to ignore case when matching.
    pub fn ignore_case(mut self) -> Self {
        self.case = CaseMatching::Ignore;
        self
    }

    /// Sets the matcher to use smart case (case-insensitive unless the pattern
    /// contains an uppercase letter).
    pub fn smart_case(mut self) -> Self {
        self.case = CaseMatching::Smart;
        self
    }

    /// Sets the matcher to respect case exactly.
    pub fn respect_case(mut self) -> Self {
        self.case = CaseMatching::Respect;
        self
    }

    /// Enables or disables thread-local caching of character buffers.
    pub fn use_cache(mut self, use_cache: bool) -> Self {
        self.use_cache = use_cache;
        self
    }

    fn contains_upper(string: &str) -> bool {
        string.chars().any(|ch| ch.is_uppercase())
    }

    fn is_case_sensitive(&self, pattern: &str) -> bool {
        match self.case {
            CaseMatching::Respect => true,
            CaseMatching::Ignore => false,
            CaseMatching::Smart => Self::contains_upper(pattern),
        }
    }
}

impl FuzzyMatcher for FzyMatcher {
    fn fuzzy_indices(&self, choice: &str, pattern: &str) -> Option<(ScoreType, Vec<IndexType>)> {
        let case_sensitive = self.is_case_sensitive(pattern);

        let mut choice_chars = self.c_cache.get_or(|| RefCell::new(Vec::new())).borrow_mut();
        let mut pattern_chars = self.p_cache.get_or(|| RefCell::new(Vec::new())).borrow_mut();

        choice_chars.clear();
        choice_chars.extend(choice.chars());

        pattern_chars.clear();
        pattern_chars.extend(pattern.chars());

        // Quick check: does the pattern even appear as a subsequence?
        cheap_matches(&choice_chars, &pattern_chars, case_sensitive)?;

        let mut positions = Vec::with_capacity(pattern_chars.len());
        let score = fzy_score(&pattern_chars, &choice_chars, case_sensitive, Some(&mut positions))?;

        if !self.use_cache {
            self.c_cache.get().map(|cell| cell.replace(vec![]));
            self.p_cache.get().map(|cell| cell.replace(vec![]));
        }

        Some((f64_to_skim_score(score), positions))
    }

    fn fuzzy_match(&self, choice: &str, pattern: &str) -> Option<ScoreType> {
        let case_sensitive = self.is_case_sensitive(pattern);

        let mut choice_chars = self.c_cache.get_or(|| RefCell::new(Vec::new())).borrow_mut();
        let mut pattern_chars = self.p_cache.get_or(|| RefCell::new(Vec::new())).borrow_mut();

        choice_chars.clear();
        choice_chars.extend(choice.chars());

        pattern_chars.clear();
        pattern_chars.extend(pattern.chars());

        cheap_matches(&choice_chars, &pattern_chars, case_sensitive)?;

        let score = fzy_score(&pattern_chars, &choice_chars, case_sensitive, None)?;

        if !self.use_cache {
            self.c_cache.get().map(|cell| cell.replace(vec![]));
            self.p_cache.get().map(|cell| cell.replace(vec![]));
        }

        Some(f64_to_skim_score(score))
    }
}

// ---------------------------------------------------------------------------
// Convenience free functions
// ---------------------------------------------------------------------------

/// Fuzzy match `choice` against `pattern` using the fzy algorithm, returning
/// the score and matched character indices.
pub fn fuzzy_indices(choice: &str, pattern: &str) -> Option<(ScoreType, Vec<IndexType>)> {
    FzyMatcher::default().ignore_case().fuzzy_indices(choice, pattern)
}

/// Fuzzy match `choice` against `pattern` using the fzy algorithm, returning
/// only the score.
pub fn fuzzy_match(choice: &str, pattern: &str) -> Option<ScoreType> {
    FzyMatcher::default().ignore_case().fuzzy_match(choice, pattern)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    use super::*;
    use crate::fuzzy_matcher::util::{assert_order, wrap_matches};

    fn wrap_fuzzy_match(choice: &str, pattern: &str) -> Option<String> {
        let (_score, indices) = fuzzy_indices(choice, pattern)?;
        Some(wrap_matches(choice, &indices))
    }

    #[test]
    fn test_no_match() {
        assert_eq!(None, fuzzy_match("abc", "abx"));
        assert_eq!(None, fuzzy_match("abc", "d"));
        assert_eq!(None, fuzzy_match("", "a"));
    }

    #[test]
    fn test_has_match() {
        assert!(fuzzy_match("axbycz", "abc").is_some());
        assert!(fuzzy_match("axbycz", "xyz").is_some());
        assert!(fuzzy_match("abc", "abc").is_some());
    }

    #[test]
    fn test_exact_match_is_max() {
        // When needle == haystack (ignoring case), fzy returns SCORE_MAX
        let matcher = FzyMatcher::default().ignore_case();
        let score = matcher.fuzzy_match("abc", "abc").unwrap();
        assert!(score > 1_000_000); // our scaled SCORE_MAX sentinel
    }

    #[test]
    fn test_match_indices() {
        assert_eq!("[a]x[b]y[c]z", &wrap_fuzzy_match("axbycz", "abc").unwrap());
        assert_eq!("a[x]b[y]c[z]", &wrap_fuzzy_match("axbycz", "xyz").unwrap());
    }

    #[test]
    fn test_consecutive_bonus() {
        // Consecutive matches should score higher than scattered ones
        let matcher = FzyMatcher::default().ignore_case();
        let consecutive = matcher.fuzzy_match("foobar", "foo").unwrap();
        let scattered = matcher.fuzzy_match("fxoxo", "foo").unwrap();
        assert!(
            consecutive > scattered,
            "consecutive={} > scattered={}",
            consecutive,
            scattered
        );
    }

    #[test]
    fn test_word_boundary_bonus() {
        // Matching at word boundaries should score higher
        let matcher = FzyMatcher::default().ignore_case();
        let boundary = matcher.fuzzy_match("foo_bar_baz", "fbb").unwrap();
        let inner = matcher.fuzzy_match("fooobarbaz", "fbb").unwrap();
        assert!(boundary > inner, "boundary={} > inner={}", boundary, inner);
    }

    #[test]
    fn test_path_separator_bonus() {
        // Matching after a '/' should get a high bonus
        let matcher = FzyMatcher::default().ignore_case();
        let path = matcher.fuzzy_match("src/lib/foo.rs", "foo").unwrap();
        let no_path = matcher.fuzzy_match("srcxlibxfoo.rs", "foo").unwrap();
        assert!(path > no_path, "path={} > no_path={}", path, no_path);
    }

    #[test]
    fn test_camel_case_bonus() {
        let matcher = FzyMatcher::default().ignore_case();
        let camel = matcher.fuzzy_match("FooBarBaz", "fbb").unwrap();
        let no_camel = matcher.fuzzy_match("foobarbaz", "fbb").unwrap();
        assert!(camel > no_camel, "camel={} > no_camel={}", camel, no_camel);
    }

    #[test]
    fn test_shorter_match_preferred() {
        let matcher = FzyMatcher::default().ignore_case();
        let short = matcher.fuzzy_match("ab", "ab").unwrap();
        let long = matcher.fuzzy_match("axxxxxxb", "ab").unwrap();
        assert!(short > long, "short={} > long={}", short, long);
    }

    #[test]
    fn test_match_quality_ordering() {
        let matcher = FzyMatcher::default();
        // Case preference
        assert_order(&matcher, "monad", &["monad", "Monad", "mONAD"]);
        // Initials
        assert_order(&matcher, "ab", &["ab", "aoo_boo", "acb"]);
        // Shorter is better
        assert_order(&matcher, "ma", &["map", "many", "maximum"]);
    }

    #[test]
    fn test_unicode_match() {
        let matcher = FzyMatcher::default().ignore_case();
        let result = matcher.fuzzy_indices("Hello, 世界", "H世");
        assert!(result.is_some());
        let (_, indices) = result.unwrap();
        assert_eq!(indices, vec![0, 7]);
    }

    #[test]
    fn test_smart_case() {
        let matcher = FzyMatcher::default().smart_case();
        // lowercase pattern → case insensitive
        assert!(matcher.fuzzy_match("FooBar", "foobar").is_some());
        // uppercase in pattern → case sensitive
        assert!(matcher.fuzzy_match("foobar", "FooBar").is_none());
        assert!(matcher.fuzzy_match("FooBar", "FooBar").is_some());
    }

    #[test]
    fn test_respect_case() {
        let matcher = FzyMatcher::default().respect_case();
        assert!(matcher.fuzzy_match("abc", "ABC").is_none());
        assert!(matcher.fuzzy_match("ABC", "ABC").is_some());
    }

    #[test]
    fn test_long_haystack() {
        // Haystacks longer than MATCH_MAX_LEN should return None from scoring
        // but still match via cheap_matches (the engine handles this)
        let matcher = FzyMatcher::default().ignore_case();
        let long = "a".repeat(MATCH_MAX_LEN + 1);
        assert_eq!(None, matcher.fuzzy_match(&long, "a"));
    }
}
