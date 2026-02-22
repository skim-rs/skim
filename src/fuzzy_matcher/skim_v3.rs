//! SkimV3 fuzzy matching algorithm.
//!
//! Uses a Smith-Waterman local alignment approach with affine gap penalties
//! and context-sensitive bonuses.
//!
//! ## Key design choices
//!
//! - **Single score per cell** (u16 saturating) plus a 2-bit direction tag
//!   for traceback. Gap open vs extend is tracked via the direction tag.
//! - **Semi-global alignment**: the pattern must be fully consumed, but
//!   alignment can start/end at any position in the choice string.
//! - **SIMD batch scoring**: 8 items at once using `wide::i16x32`.
//!
//!
//! Optimizations to explore:
//! - SSW SIMD "Striped" (https://github.com/mengyao/Complete-Striped-Smith-Waterman-Library)
//! - Banding & interpair pruning to add better filtering heuristics, making sure this is pertinent
//! to our problem

use std::cell::RefCell;

use thread_local::ThreadLocal;

use crate::{
    CaseMatching,
    fuzzy_matcher::{FuzzyMatcher, IndexType, MatchIndices, ScoreType},
};

// ---------------------------------------------------------------------------
// Scoring constants
// ---------------------------------------------------------------------------

type Score = u16;

/// Points awarded for each correctly matched character.
const MATCH_BONUS: Score = 16;

/// Extra bonus when the match is at position 0 of the choice string.
const START_OF_STRING_BONUS: Score = 12;

/// Extra bonus when the match follows a word separator.
const START_OF_WORD_BONUS: Score = 8;

/// Extra bonus for a camelCase transition.
const CAMEL_CASE_BONUS: Score = 6;

/// Bonus for each additional consecutive matched character.
const CONSECUTIVE_BONUS: Score = 8;

/// Multiplier applied to the very first pattern character's positional bonus.
const FIRST_CHAR_BONUS_MULTIPLIER: Score = 2;

/// Cost to open a gap (skip characters in either string).
const GAP_OPEN: Score = 6;

/// Cost to extend a gap by one more character.
const GAP_EXTEND: Score = 4;

/// Penalty for aligning a pattern char to a different choice char (typos only).
const MISMATCH_PENALTY: Score = 10;

// ---------------------------------------------------------------------------
// Byte-level helpers
// ---------------------------------------------------------------------------

trait Atom: PartialEq + Into<char> + Copy {
    #[inline(always)]
    fn eq(self, other: Self, respect_case: bool) -> bool
    where
        Self: PartialEq + Sized,
    {
        if respect_case {
            self == other
        } else {
            self.eq_ignore_case(other)
        }
    }
    fn eq_ignore_case(self, other: Self) -> bool;
    fn is_lowercase(self) -> bool;
}
impl Atom for u8 {
    #[inline(always)]
    fn eq_ignore_case(self, b: Self) -> bool {
        self.eq_ignore_ascii_case(&b)
    }
    #[inline(always)]
    fn is_lowercase(self) -> bool {
        self.is_ascii_lowercase()
    }
}
impl Atom for char {
    #[inline(always)]
    fn eq_ignore_case(self, b: Self) -> bool {
        self.to_lowercase().eq(b.to_lowercase())
    }
    #[inline(always)]
    fn is_lowercase(self) -> bool {
        self.is_ascii_lowercase()
    }
}

#[derive(Default, Debug)]
struct SWMatrix {
    data: Vec<Cell>,
    cols: usize,
    rows: usize,
}
impl SWMatrix {
    pub fn zero(rows: usize, cols: usize) -> Self {
        let mut res = SWMatrix::default();
        res.resize(rows, cols);
        res
    }
    #[inline(always)]
    pub fn at(&mut self, row: usize, col: usize) -> &mut Cell {
        if row >= self.rows {
            panic!("Row index {row} is out of bounds for rows {}", self.rows);
        }
        if col >= self.cols {
            panic!("Col index {col} is out of bounds for cols {}", self.cols);
        }
        &mut self.data[row * self.cols + col]
    }
    pub fn resize(&mut self, rows: usize, cols: usize) {
        if rows * cols > self.rows * self.cols {
            self.data.resize(rows * cols, CELL_ZERO);
        }
        self.rows = rows;
        self.cols = cols;
    }
}

fn precompute_bonuses<C: Atom>(cho: &[C], buf: &mut Vec<Score>) {
    buf.clear();
    buf.reserve(cho.len());
    for (j, &ch) in cho.iter().enumerate() {
        let mut bonus: Score = 0;
        if j == 0 {
            bonus += START_OF_STRING_BONUS;
        } else {
            let prev = cho[j - 1];
            if matches!(prev.into(), ' ' | '/' | '\\' | '-' | '_' | '.') {
                bonus += START_OF_WORD_BONUS;
            }
            if prev.is_lowercase() && ch.is_lowercase() {
                bonus += CAMEL_CASE_BONUS;
            }
        }
        buf.push(bonus);
    }
}

// ---------------------------------------------------------------------------
// Packed cell for full DP: score (u16) + direction (u8) in u32
// ---------------------------------------------------------------------------

/// Direction the optimal path took to reach a cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum Dir {
    /// Diagonal: match or mismatch (came from [i-1][j-1])
    Diag = 0,
    /// Up: gap in choice (came from [i-1][j], skip pattern char)
    Up = 1,
    /// Left: gap in pattern (came from [i][j-1], skip choice char)
    Left = 2,
    /// No valid path (score == 0)
    None = 3,
}

/// Packed cell: bits [31:16] = score, bits [1:0] = direction.
/// Using u32 for cache-friendly single-vector full DP.
#[derive(Debug, Copy, Clone)]
struct Cell(u32);

const CELL_ZERO: Cell = Cell::new(0, Dir::None);

impl Cell {
    #[inline(always)]
    pub const fn new(score: Score, dir: Dir) -> Cell {
        Cell(((score as u32) << 2) | (dir as u32))
    }
    #[inline(always)]
    pub fn score(self) -> Score {
        (self.0 >> 2) as Score
    }
    #[inline(always)]
    fn dir(self) -> Dir {
        match self.0 & 3 {
            0 => Dir::Diag,
            1 => Dir::Up,
            2 => Dir::Left,
            _ => Dir::None,
        }
    }
}

// ---------------------------------------------------------------------------
// SkimV3Matcher
// ---------------------------------------------------------------------------

/// SkimV3 fuzzy matcher: Smith-Waterman local alignment with affine gap
/// penalties and context-sensitive bonuses.
#[derive(Debug, Default)]
pub struct SkimV3Matcher {
    pub(crate) case: CaseMatching,
    pub(crate) allow_typos: bool,
    full_buf: ThreadLocal<RefCell<SWMatrix>>,
    #[allow(clippy::type_complexity)]
    char_buf: ThreadLocal<RefCell<(Vec<char>, Vec<char>)>>,
    bonus_buf: ThreadLocal<RefCell<Vec<Score>>>,
}

impl SkimV3Matcher {
    /// Create a new `SkimV3Matcher` with the given settings.
    pub fn new(case: CaseMatching, allow_typos: bool) -> Self {
        Self {
            case,
            allow_typos,
            ..Default::default()
        }
    }

    #[inline]
    fn respect_case<C: Atom>(&self, pattern: &[C]) -> bool {
        self.case == CaseMatching::Respect
            || (self.case == CaseMatching::Smart && !pattern.iter().all(|b| b.is_lowercase()))
    }
}

// ---------------------------------------------------------------------------
// Prefilters
// ---------------------------------------------------------------------------

fn is_subsequence<C: Atom>(pattern: &[C], choice: &[C], respect_case: bool) -> bool {
    let mut pi = 0;
    for &c in choice {
        if pi < pattern.len() && pattern[pi].eq(c, respect_case) {
            pi += 1;
        }
    }
    pi == pattern.len()
}

fn cheap_typo_prefilter<C: Atom>(pattern: &[C], choice: &[C], respect_case: bool) -> bool {
    let n = pattern.len();
    if n > choice.len() * 2 + 2 {
        return false;
    }
    let mut pi = 0;
    for &c in choice {
        if pi * 2 < n && pattern[pi].eq(c, respect_case) {
            pi += 1;
        }
    }
    pi * 2 >= n
}

#[inline]
fn find_begin<C: Atom>(cho: &[C], pat: &[C], end: usize, respect_case: bool) -> usize {
    let first = pat[0];
    let limit = end.min(cho.len().saturating_sub(1));
    cho.iter()
        .take(limit + 1)
        .position(|x| first.eq(*x, respect_case))
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Smith-Waterman DP — Score-only (column-major, stack arrays)
// ---------------------------------------------------------------------------

const COLMAJOR_MAX_N: usize = 16;

/// Column-major score-only for byte slices (n <= 16, stack allocated).
fn score_only_colmajor<const ALLOW_TYPOS: bool, C: Atom>(
    cho: &[C],
    pat: &[C],
    bonuses: &[Score],
    respect_case: bool,
) -> Option<(Score, usize)> {
    let n = pat.len();
    let m = cho.len();

    // Two columns + direction tracking
    let mut score_a = [0u16; COLMAJOR_MAX_N + 1];
    let mut score_b = [0u16; COLMAJOR_MAX_N + 1];
    let mut diag_a = [false; COLMAJOR_MAX_N + 1];
    let mut diag_b = [false; COLMAJOR_MAX_N + 1];

    let mut prev_s = &mut score_a;
    let mut cur_s = &mut score_b;
    let mut prev_d = &mut diag_a;
    let mut cur_d = &mut diag_b;

    let mut global_best: Score = 0;
    let mut global_best_j: usize = 0;

    for j in 1..=m {
        let cj = cho[j - 1];
        let bonus_j = bonuses[j - 1];

        cur_s[0] = 0;
        cur_d[0] = false;

        for i in 1..=n {
            let pi = pat[i - 1];
            let is_match = pi.eq(cj, respect_case);
            let is_first = i == 1;

            let diag_score = prev_s[i - 1];
            let diag_was_diag = prev_d[i - 1];

            let up_score = cur_s[i - 1]; // same col, prev row (already computed)
            let up_was_diag = cur_d[i - 1];

            let left_score = prev_s[i]; // prev col, same row
            let left_was_diag = prev_d[i];

            // DIAGONAL
            let mut bonus = bonus_j;
            if diag_was_diag {
                bonus += CONSECUTIVE_BONUS;
            }
            if is_first {
                bonus *= FIRST_CHAR_BONUS_MULTIPLIER;
            }

            let diag_val = if is_match {
                diag_score.saturating_add(MATCH_BONUS + bonus)
            } else if ALLOW_TYPOS {
                diag_score.saturating_sub(MISMATCH_PENALTY)
            } else {
                0
            };

            // UP (typos only)
            let up_val = if ALLOW_TYPOS {
                let pen = if up_was_diag { GAP_OPEN } else { GAP_EXTEND };
                up_score.saturating_sub(pen)
            } else {
                0
            };

            // LEFT
            let left_val = {
                let pen = if left_was_diag { GAP_OPEN } else { GAP_EXTEND };
                left_score.saturating_sub(pen)
            };

            let best = diag_val.max(up_val).max(left_val);
            cur_s[i] = best;
            cur_d[i] = best > 0 && diag_val >= up_val && diag_val >= left_val;

            if i == n && best > global_best {
                global_best = best;
                global_best_j = j;
            }
        }

        std::mem::swap(&mut prev_s, &mut cur_s);
        std::mem::swap(&mut prev_d, &mut cur_d);
    }

    if global_best > 0 {
        Some((global_best, global_best_j))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Full DP with traceback — packed Cell (u32 = score + dir)
// ---------------------------------------------------------------------------

/// Full DP for byte slices using packed cells.
fn full_dp<const ALLOW_TYPOS: bool, C: Atom>(
    cho: &[C],
    pat: &[C],
    bonuses: &[Score],
    respect_case: bool,
    full_buf: &ThreadLocal<RefCell<SWMatrix>>,
) -> Option<(Score, MatchIndices)> {
    let n = pat.len();
    let m = cho.len();

    let mut buf = full_buf
        .get_or(|| RefCell::new(SWMatrix::zero(n + 1, m + 1)))
        .borrow_mut();
    buf.resize(n + 1, m + 1);

    // Initialize row 0 and column 0
    for j in 0..=m {
        *buf.at(0, j) = CELL_ZERO;
    }
    for i in 1..=n {
        *buf.at(i, 0) = CELL_ZERO;
    }

    let mut best_score = 0;
    let mut best_j = 0;

    for i in 1..=n {
        let pi = pat[i - 1];
        let is_first = i == 1;

        for j in 1..=m {
            let cj = cho[j - 1];
            let is_match = pi.eq(cj, respect_case);

            // DIAGONAL
            let diag_cell = *buf.at(i - 1, j - 1);
            let diag_score = diag_cell.score();
            let diag_was_diag = diag_cell.dir() == Dir::Diag;

            let mut bonus = bonuses[j - 1];
            if diag_was_diag {
                bonus += CONSECUTIVE_BONUS;
            }
            if is_first {
                bonus *= FIRST_CHAR_BONUS_MULTIPLIER;
            }

            let diag_val = if is_match {
                diag_score.saturating_add(MATCH_BONUS + bonus)
            } else if ALLOW_TYPOS {
                diag_score.saturating_sub(MISMATCH_PENALTY)
            } else {
                0
            };

            // UP (skip pattern char, typos only)
            let up_val = if ALLOW_TYPOS {
                let up_cell = *buf.at(i - 1, j);
                let pen = if up_cell.dir() == Dir::Diag {
                    GAP_OPEN
                } else {
                    GAP_EXTEND
                };
                up_cell.score().saturating_sub(pen)
            } else {
                0
            };

            // LEFT (skip choice char)
            let left_val = {
                let left_cell = *buf.at(i, j - 1);
                let pen = if left_cell.dir() == Dir::Diag {
                    GAP_OPEN
                } else {
                    GAP_EXTEND
                };
                left_cell.score().saturating_sub(pen)
            };

            let best = diag_val.max(up_val).max(left_val);

            let dir = if best == 0 {
                Dir::None
            } else if diag_val >= up_val && diag_val >= left_val && (is_match || ALLOW_TYPOS) {
                Dir::Diag
            } else if ALLOW_TYPOS && up_val >= left_val {
                Dir::Up
            } else {
                Dir::Left
            };

            let score = Cell::new(best, dir);
            *buf.at(i, j) = score;
            if best > best_score {
                best_score = best;
                best_j = j;
            }
        }
    }

    if best_score == 0 {
        return None;
    }

    // Traceback
    let mut indices: MatchIndices = Vec::with_capacity(n);
    let mut i = n;
    let mut j = best_j;

    while i > 0 && j > 0 {
        let c = *buf.at(i, j);
        match c.dir() {
            Dir::Diag => {
                if pat[i - 1].eq(cho[j - 1], respect_case) {
                    indices.push((j - 1) as IndexType);
                }
                i -= 1;
                j -= 1;
            }
            Dir::Up => {
                i -= 1;
            }
            Dir::Left => {
                j -= 1;
            }
            Dir::None => break,
        }
    }

    indices.sort_unstable();
    indices.dedup();

    Some((best_score, indices))
}

// ---------------------------------------------------------------------------
// FuzzyMatcher trait implementation
// ---------------------------------------------------------------------------

impl FuzzyMatcher for SkimV3Matcher {
    fn fuzzy_match(&self, choice: &str, pattern: &str) -> Option<ScoreType> {
        if pattern.is_empty() {
            return Some(0);
        }
        if choice.is_empty() {
            return None;
        }

        if choice.is_ascii() && pattern.is_ascii() {
            let cho = choice.as_bytes();
            let pat = pattern.as_bytes();
            let respect_case = self.respect_case(pat);

            if !self.allow_typos && !is_subsequence(pat, cho, respect_case) {
                return None;
            }
            if self.allow_typos && !cheap_typo_prefilter(pat, cho, respect_case) {
                return None;
            }

            let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
            precompute_bonuses(cho, &mut bonus_buf);

            let result = if self.allow_typos {
                score_only_colmajor::<true, _>(cho, pat, &bonus_buf, respect_case)
            } else {
                score_only_colmajor::<false, _>(cho, pat, &bonus_buf, respect_case)
            };
            return result.map(|(s, _end)| s as ScoreType);
        }

        let mut bufs = self
            .char_buf
            .get_or(|| RefCell::new((Vec::new(), Vec::new())))
            .borrow_mut();
        let (ref mut pat_buf, ref mut cho_buf) = *bufs;
        pat_buf.clear();
        pat_buf.extend(pattern.chars());
        cho_buf.clear();
        cho_buf.extend(choice.chars());

        let respect_case = self.respect_case(pat_buf);

        if !self.allow_typos && !is_subsequence(pat_buf, cho_buf, respect_case) {
            return None;
        }
        if self.allow_typos && !cheap_typo_prefilter(pat_buf, cho_buf, respect_case) {
            return None;
        }

        let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
        precompute_bonuses(cho_buf, &mut bonus_buf);

        let result = if self.allow_typos {
            score_only_colmajor::<true, _>(cho_buf, pat_buf, &bonus_buf, respect_case)
        } else {
            score_only_colmajor::<false, _>(cho_buf, pat_buf, &bonus_buf, respect_case)
        };
        result.map(|(s, _end)| s as ScoreType)
    }

    fn fuzzy_match_range(&self, choice: &str, pattern: &str) -> Option<(ScoreType, usize, usize)> {
        if pattern.is_empty() {
            return Some((0, 0, 0));
        }
        if choice.is_empty() {
            return None;
        }

        if choice.is_ascii() && pattern.is_ascii() {
            let cho = choice.as_bytes();
            let pat = pattern.as_bytes();
            let respect_case = self.respect_case(pat);

            if !self.allow_typos && !is_subsequence(pat, cho, respect_case) {
                return None;
            }
            if self.allow_typos && !cheap_typo_prefilter(pat, cho, respect_case) {
                return None;
            }

            let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
            precompute_bonuses(cho, &mut bonus_buf);

            let result = if self.allow_typos {
                score_only_colmajor::<true, _>(cho, pat, &bonus_buf, respect_case)
            } else {
                score_only_colmajor::<false, _>(cho, pat, &bonus_buf, respect_case)
            };
            return result.map(|(s, end_col)| {
                let end = if end_col > 0 { end_col - 1 } else { 0 };
                let begin = find_begin(cho, pat, end, respect_case);
                (s as ScoreType, begin, end)
            });
        }

        let mut bufs = self
            .char_buf
            .get_or(|| RefCell::new((Vec::new(), Vec::new())))
            .borrow_mut();
        let (ref mut pat_buf, ref mut cho_buf) = *bufs;
        pat_buf.clear();
        pat_buf.extend(pattern.chars());
        cho_buf.clear();
        cho_buf.extend(choice.chars());

        let respect_case = self.respect_case(pat_buf);

        if !self.allow_typos && !is_subsequence(pat_buf, cho_buf, respect_case) {
            return None;
        }
        if self.allow_typos && !cheap_typo_prefilter(pat_buf, cho_buf, respect_case) {
            return None;
        }

        let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
        precompute_bonuses(cho_buf, &mut bonus_buf);

        let result = if self.allow_typos {
            score_only_colmajor::<true, _>(cho_buf, pat_buf, &bonus_buf, respect_case)
        } else {
            score_only_colmajor::<false, _>(cho_buf, pat_buf, &bonus_buf, respect_case)
        };
        result.map(|(s, end_col)| {
            let end = if end_col > 0 { end_col - 1 } else { 0 };
            let begin = find_begin(cho_buf, pat_buf, end, respect_case);
            (s as ScoreType, begin, end)
        })
    }

    fn fuzzy_indices(&self, choice: &str, pattern: &str) -> Option<(ScoreType, MatchIndices)> {
        if pattern.is_empty() {
            return Some((0, MatchIndices::new()));
        }
        if choice.is_empty() {
            return None;
        }

        if choice.is_ascii() && pattern.is_ascii() {
            let cho = choice.as_bytes();
            let pat = pattern.as_bytes();
            let respect_case = self.respect_case(pat);

            if !self.allow_typos && !is_subsequence(pat, cho, respect_case) {
                return None;
            }
            if self.allow_typos && !cheap_typo_prefilter(pat, cho, respect_case) {
                return None;
            }

            let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
            precompute_bonuses(cho, &mut bonus_buf);

            let result = if self.allow_typos {
                full_dp::<true, _>(cho, pat, &bonus_buf, respect_case, &self.full_buf)
            } else {
                full_dp::<false, _>(cho, pat, &bonus_buf, respect_case, &self.full_buf)
            };
            return result.map(|(s, idx)| (s as ScoreType, idx));
        }

        let mut bufs = self
            .char_buf
            .get_or(|| RefCell::new((Vec::new(), Vec::new())))
            .borrow_mut();
        let (ref mut pat_buf, ref mut cho_buf) = *bufs;
        pat_buf.clear();
        pat_buf.extend(pattern.chars());
        cho_buf.clear();
        cho_buf.extend(choice.chars());

        let respect_case = self.respect_case(pat_buf);

        // Prefilter
        if !self.allow_typos && !is_subsequence(pat_buf, cho_buf, respect_case) {
            return None;
        }
        if self.allow_typos && !cheap_typo_prefilter(pat_buf, cho_buf, respect_case) {
            return None;
        }

        let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
        precompute_bonuses(cho_buf, &mut bonus_buf);

        let result = if self.allow_typos {
            full_dp::<true, _>(cho_buf, pat_buf, &bonus_buf, respect_case, &self.full_buf)
        } else {
            full_dp::<false, _>(cho_buf, pat_buf, &bonus_buf, respect_case, &self.full_buf)
        };
        result.map(|(s, idx)| (s as ScoreType, idx))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fuzzy_matcher::FuzzyMatcher;

    fn matcher() -> SkimV3Matcher {
        SkimV3Matcher::default()
    }

    fn matcher_typos() -> SkimV3Matcher {
        SkimV3Matcher {
            allow_typos: true,
            ..Default::default()
        }
    }

    fn score(choice: &str, pattern: &str) -> Option<i64> {
        matcher().fuzzy_match(choice, pattern)
    }

    fn score_typos(choice: &str, pattern: &str) -> Option<i64> {
        matcher_typos().fuzzy_match(choice, pattern)
    }

    fn indices(choice: &str, pattern: &str) -> Option<MatchIndices> {
        matcher().fuzzy_indices(choice, pattern).map(|(_, v)| v)
    }

    // ----- Basic matching -----

    #[test]
    fn empty_pattern_always_matches() {
        assert_eq!(score("anything", ""), Some(0));
        assert_eq!(score("", ""), Some(0));
    }

    #[test]
    fn empty_choice_never_matches() {
        assert!(score("", "a").is_none());
    }

    #[test]
    fn exact_match_scores_positive() {
        assert!(score("hello", "hello").unwrap() > 0);
    }

    #[test]
    fn no_match_returns_none() {
        assert!(score("abc", "xyz").is_none());
    }

    #[test]
    fn subsequence_match() {
        assert!(score("axbycz", "abc").is_some());
        let idx = indices("axbycz", "abc").unwrap();
        assert_eq!(idx.as_slice(), &[0, 2, 4]);
    }

    // ----- Scoring quality -----

    #[test]
    fn contiguous_beats_scattered() {
        let contiguous = score("ab", "ab").unwrap();
        let scattered = score("axb", "ab").unwrap();
        assert!(
            contiguous > scattered,
            "contiguous={contiguous} should beat scattered={scattered}"
        );
    }

    #[test]
    fn fewer_gaps_beats_more_gaps() {
        let one_gap = score("abxc", "abc").unwrap();
        let two_gaps = score("axbxc", "abc").unwrap();
        assert!(one_gap > two_gaps, "one_gap={one_gap} should beat two_gaps={two_gaps}");
    }

    #[test]
    fn word_start_bonus() {
        let boundary = score("src/reader.rs", "reader").unwrap();
        let stitched = score("src/tui/header.rs", "reader").unwrap();
        assert!(
            boundary > stitched,
            "word-boundary={boundary} should beat stitched={stitched}"
        );
    }

    #[test]
    fn start_of_string_bonus() {
        let at_start = score("abc", "a").unwrap();
        let at_mid = score("xabc", "a").unwrap();
        assert!(at_start > at_mid, "start={at_start} should beat mid={at_mid}");
    }

    #[test]
    fn consecutive_match_preferred() {
        let consecutive = score("foobar", "oob").unwrap();
        let spread = score("oxoxb", "oob").unwrap();
        assert!(
            consecutive > spread,
            "consecutive={consecutive} should beat spread={spread}"
        );
    }

    #[test]
    fn camel_case_bonus() {
        let camel = score("FooBar", "fb").unwrap();
        let flat = score("fxxbxx", "fb").unwrap();
        assert!(camel > flat, "camel={camel} should beat flat={flat}");
    }

    // ----- Case sensitivity -----

    #[test]
    fn smart_case_insensitive_lowercase_pattern() {
        let m = SkimV3Matcher {
            case: CaseMatching::Smart,
            allow_typos: false,
            ..Default::default()
        };
        assert!(m.fuzzy_match("FooBar", "foobar").is_some());
    }

    #[test]
    fn smart_case_sensitive_uppercase_pattern() {
        let m = SkimV3Matcher {
            case: CaseMatching::Smart,
            allow_typos: false,
            ..Default::default()
        };
        assert!(m.fuzzy_match("foobar", "FooBar").is_none());
        assert!(m.fuzzy_match("FooBar", "FooBar").is_some());
    }

    #[test]
    fn respect_case() {
        let m = SkimV3Matcher {
            case: CaseMatching::Respect,
            allow_typos: false,
            ..Default::default()
        };
        assert!(m.fuzzy_match("abc", "ABC").is_none());
        assert!(m.fuzzy_match("ABC", "ABC").is_some());
    }

    #[test]
    fn ignore_case() {
        let m = SkimV3Matcher {
            case: CaseMatching::Ignore,
            allow_typos: false,
            ..Default::default()
        };
        assert!(m.fuzzy_match("abc", "ABC").is_some());
    }

    // ----- Typo tolerance -----

    #[test]
    fn no_typos_rejects_mismatch() {
        assert!(score("hxllo", "hello").is_none());
    }

    #[test]
    fn typos_accepts_mismatch() {
        assert!(score_typos("hxllo", "hello").is_some());
    }

    #[test]
    fn no_typos_rejects_transposition() {
        assert!(score("hlelo", "hello").is_none());
    }

    #[test]
    fn typos_accepts_transposition() {
        assert!(score_typos("hlelo", "hello").is_some());
    }

    #[test]
    fn exact_match_same_with_and_without_typos() {
        let with = score_typos("hello", "hello").unwrap();
        let without = score("hello", "hello").unwrap();
        assert_eq!(
            with, without,
            "exact match score should be identical regardless of typo flag"
        );
    }

    #[test]
    fn typo_match_scores_less_than_exact() {
        let exact = score_typos("hello", "hello").unwrap();
        let typo = score_typos("hxllo", "hello").unwrap();
        assert!(exact > typo, "exact={exact} should beat typo={typo}");
    }

    // ----- Traceback correctness -----

    #[test]
    fn indices_exact_match() {
        let idx = indices("hello", "hello").unwrap();
        assert_eq!(idx.as_slice(), &[0, 1, 2, 3, 4]);
    }

    #[test]
    fn transposition_matches() {
        let result = matcher_typos().fuzzy_indices("abdc", "abcd");
        assert!(result.is_some(), "transposed input should match with typos");
        let (score_trans, _) = result.unwrap();

        let (score_exact, _) = matcher_typos().fuzzy_indices("abcd", "abcd").unwrap();
        assert!(
            score_exact > score_trans,
            "exact={score_exact} should beat transposed={score_trans}"
        );
    }

    // ----- Reader ranking regression -----

    #[test]
    fn reader_ranking() {
        let pattern = "reader";
        let dense = score("src/reader.rs", pattern).unwrap();
        let sparse = score(
            "tests/snapshots/normalize__insta_normalize_accented_item_unaccented_query.snap",
            pattern,
        )
        .unwrap_or(0);
        assert!(dense > sparse, "dense={dense} should beat sparse={sparse}");
    }

    // ----- Ordering sanity -----

    #[test]
    fn ordering_ab() {
        use crate::fuzzy_matcher::util::assert_order;
        let m = SkimV3Matcher {
            case: CaseMatching::Ignore,
            allow_typos: false,
            ..Default::default()
        };
        assert_order(&m, "ab", &["ab", "aoo_boo", "acb"]);
    }

    #[test]
    fn ordering_print() {
        use crate::fuzzy_matcher::util::assert_order;
        let m = SkimV3Matcher {
            case: CaseMatching::Ignore,
            allow_typos: false,
            ..Default::default()
        };
        assert_order(&m, "print", &["printf", "sprintf"]);
    }

    // ----- Score-only vs full DP consistency -----

    #[test]
    fn score_only_matches_full_dp() {
        let m = SkimV3Matcher {
            case: CaseMatching::Ignore,
            allow_typos: true,
            ..Default::default()
        };
        let cases = [
            ("hello world", "hlo"),
            ("src/reader.rs", "reader"),
            ("FooBar", "fb"),
            ("axbycz", "abc"),
            ("hxllo", "hello"),
        ];
        for (choice, pattern) in &cases {
            let score_only = m.fuzzy_match(choice, pattern);
            let full = m.fuzzy_indices(choice, pattern).map(|(s, _)| s);
            assert_eq!(
                score_only, full,
                "score mismatch for ({choice}, {pattern}): score_only={score_only:?} full={full:?}"
            );
        }
    }

    // ----- Non-ASCII fallback -----

    #[test]
    fn non_ascii_matching() {
        let m = matcher();
        assert!(m.fuzzy_match("café", "café").is_some());
        assert!(m.fuzzy_match("naïve", "naive").is_none());
    }
}
