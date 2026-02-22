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
//! - **SIMD batch scoring**: 8 items at once using `wide::i32x8`.
//!
//!
//! Optimizations to explore:
//! - SIMD "Striped" approach for single-choice SIMD optimization to remove the batch variants
//! altogether
//! - Banding & interpair pruning to add better filtering heuristics, making sure this is pertinent
//! to our problem
//! - Try out BLAST to see if the trades we make are OK for our use case

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

/// Penalty for a case-insensitive match where the cases differ.
const CASE_MISMATCH_PENALTY: Score = 1;

/// Cost to open a gap (skip characters in either string).
const GAP_OPEN: Score = 6;

/// Cost to extend a gap by one more character.
const GAP_EXTEND: Score = 2;

/// Penalty for aligning a pattern char to a different choice char (typos only).
const MISMATCH_PENALTY: Score = 10;

// ---------------------------------------------------------------------------
// Byte-level helpers
// ---------------------------------------------------------------------------

#[inline(always)]
fn eq_byte(a: u8, b: u8, respect_case: bool) -> bool {
    if respect_case {
        a == b
    } else {
        a.eq_ignore_ascii_case(&b)
    }
}

#[inline(always)]
fn eq_char(a: char, b: char, respect_case: bool) -> bool {
    if respect_case {
        a == b
    } else {
        a.eq_ignore_ascii_case(&b)
    }
}

fn precompute_bonuses_bytes(cho: &[u8], buf: &mut Vec<Score>) {
    buf.clear();
    buf.reserve(cho.len());
    for (j, &ch) in cho.iter().enumerate() {
        let mut bonus: Score = 0;
        if j == 0 {
            bonus += START_OF_STRING_BONUS;
        } else {
            let prev = cho[j - 1];
            if matches!(prev, b' ' | b'/' | b'\\' | b'-' | b'_' | b'.') {
                bonus += START_OF_WORD_BONUS;
            }
            if prev.is_ascii_lowercase() && ch.is_ascii_uppercase() {
                bonus += CAMEL_CASE_BONUS;
            }
        }
        buf.push(bonus);
    }
}

fn precompute_bonuses_chars(cho: &[char], buf: &mut Vec<Score>) {
    buf.clear();
    buf.reserve(cho.len());
    for (j, &ch) in cho.iter().enumerate() {
        let mut bonus: Score = 0;
        if j == 0 {
            bonus += START_OF_STRING_BONUS;
        } else {
            let prev = cho[j - 1];
            if matches!(prev, ' ' | '/' | '\\' | '-' | '_' | '.') {
                bonus += START_OF_WORD_BONUS;
            }
            if prev.is_lowercase() && ch.is_uppercase() {
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
type Cell = u32;

const CELL_ZERO: Cell = 0 | (Dir::None as u32);

#[inline(always)]
fn cell_pack(score: Score, dir: Dir) -> Cell {
    ((score as u32) << 2) | (dir as u32)
}

#[inline(always)]
fn cell_score(c: Cell) -> Score {
    (c >> 2) as Score
}

#[inline(always)]
fn cell_dir(c: Cell) -> Dir {
    match c & 3 {
        0 => Dir::Diag,
        1 => Dir::Up,
        2 => Dir::Left,
        _ => Dir::None,
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
    full_buf: ThreadLocal<RefCell<Vec<Cell>>>,
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
    fn respect_case_bytes(&self, pattern: &[u8]) -> bool {
        self.case == CaseMatching::Respect
            || (self.case == CaseMatching::Smart && pattern.iter().any(|b| b.is_ascii_uppercase()))
    }

    #[inline]
    fn respect_case_chars(&self, pattern: &[char]) -> bool {
        self.case == CaseMatching::Respect
            || (self.case == CaseMatching::Smart && pattern.iter().any(|c| c.is_uppercase()))
    }
}

// ---------------------------------------------------------------------------
// Prefilters
// ---------------------------------------------------------------------------

fn is_subsequence_bytes(pattern: &[u8], choice: &[u8], respect_case: bool) -> bool {
    let mut pi = 0;
    for &c in choice {
        if pi < pattern.len() && eq_byte(pattern[pi], c, respect_case) {
            pi += 1;
        }
    }
    pi == pattern.len()
}

fn is_subsequence_chars(pattern: &[char], choice: &[char], respect_case: bool) -> bool {
    let mut pi = 0;
    for &c in choice {
        if pi < pattern.len() && eq_char(pattern[pi], c, respect_case) {
            pi += 1;
        }
    }
    pi == pattern.len()
}

fn cheap_typo_prefilter_bytes(pattern: &[u8], choice: &[u8], respect_case: bool) -> bool {
    let n = pattern.len();
    if n > choice.len() * 2 + 2 {
        return false;
    }
    let mut pi = 0;
    for &c in choice {
        if pi < n && eq_byte(pattern[pi], c, respect_case) {
            pi += 1;
        }
    }
    pi >= 1
}

fn cheap_typo_prefilter_chars(pattern: &[char], choice: &[char], respect_case: bool) -> bool {
    let n = pattern.len();
    if n > choice.len() * 2 + 2 {
        return false;
    }
    let mut pi = 0;
    for &c in choice {
        if pi < n && eq_char(pattern[pi], c, respect_case) {
            pi += 1;
        }
    }
    pi >= 1
}

#[inline]
fn find_begin_byte(cho: &[u8], pat: &[u8], end: usize, respect_case: bool) -> usize {
    let first = pat[0];
    let limit = end.min(cho.len().saturating_sub(1));
    for j in 0..=limit {
        if eq_byte(first, cho[j], respect_case) {
            return j;
        }
    }
    0
}

#[inline]
fn find_begin_char(cho: &[char], pat: &[char], end: usize, respect_case: bool) -> usize {
    let first = pat[0];
    let limit = end.min(cho.len().saturating_sub(1));
    for j in 0..=limit {
        if eq_char(first, cho[j], respect_case) {
            return j;
        }
    }
    0
}

// ---------------------------------------------------------------------------
// Smith-Waterman DP — Score-only (column-major, stack arrays)
// ---------------------------------------------------------------------------

const COLMAJOR_MAX_N: usize = 16;

/// Column-major score-only for byte slices (n <= 16, stack allocated).
fn score_only_bytes_colmajor<const ALLOW_TYPOS: bool>(
    cho: &[u8],
    pat: &[u8],
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
            let is_match = eq_byte(pi, cj, respect_case);
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
                let case_pen = if !respect_case && pi != cj {
                    CASE_MISMATCH_PENALTY
                } else {
                    0
                };
                diag_score.saturating_add(MATCH_BONUS + bonus).saturating_sub(case_pen)
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

/// Column-major score-only for char slices.
fn score_only_chars_colmajor<const ALLOW_TYPOS: bool>(
    cho: &[char],
    pat: &[char],
    bonuses: &[Score],
    respect_case: bool,
) -> Option<(Score, usize)> {
    let n = pat.len();
    let m = cho.len();

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
            let is_match = eq_char(pi, cj, respect_case);
            let is_first = i == 1;

            let diag_score = prev_s[i - 1];
            let diag_was_diag = prev_d[i - 1];
            let up_score = cur_s[i - 1];
            let up_was_diag = cur_d[i - 1];
            let left_score = prev_s[i];
            let left_was_diag = prev_d[i];

            let mut bonus = bonus_j;
            if diag_was_diag {
                bonus += CONSECUTIVE_BONUS;
            }
            if is_first {
                bonus *= FIRST_CHAR_BONUS_MULTIPLIER;
            }

            let diag_val = if is_match {
                let case_pen = if !respect_case && pi != cj {
                    CASE_MISMATCH_PENALTY
                } else {
                    0
                };
                diag_score.saturating_add(MATCH_BONUS + bonus).saturating_sub(case_pen)
            } else if ALLOW_TYPOS {
                diag_score.saturating_sub(MISMATCH_PENALTY)
            } else {
                0
            };

            let up_val = if ALLOW_TYPOS {
                let pen = if up_was_diag { GAP_OPEN } else { GAP_EXTEND };
                up_score.saturating_sub(pen)
            } else {
                0
            };

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
fn full_dp_bytes<const ALLOW_TYPOS: bool>(
    cho: &[u8],
    pat: &[u8],
    bonuses: &[Score],
    respect_case: bool,
    full_buf: &ThreadLocal<RefCell<Vec<Cell>>>,
) -> Option<(Score, MatchIndices)> {
    let n = pat.len();
    let m = cho.len();
    let cols = m + 1;
    let total = (n + 1) * cols;

    let mut buf = full_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
    if buf.len() < total {
        buf.resize(total, CELL_ZERO);
    }

    #[inline(always)]
    fn idx(i: usize, j: usize, cols: usize) -> usize {
        i * cols + j
    }

    // Initialize row 0 and column 0
    for j in 0..cols {
        buf[idx(0, j, cols)] = CELL_ZERO;
    }
    for i in 1..=n {
        buf[idx(i, 0, cols)] = CELL_ZERO;
    }

    for i in 1..=n {
        let pi = pat[i - 1];
        let is_first = i == 1;

        for j in 1..=m {
            let cj = cho[j - 1];
            let is_match = eq_byte(pi, cj, respect_case);
            let ci = idx(i, j, cols);

            // DIAGONAL
            let diag_cell = buf[idx(i - 1, j - 1, cols)];
            let diag_score = cell_score(diag_cell);
            let diag_was_diag = cell_dir(diag_cell) == Dir::Diag;

            let mut bonus = bonuses[j - 1];
            if diag_was_diag {
                bonus += CONSECUTIVE_BONUS;
            }
            if is_first {
                bonus *= FIRST_CHAR_BONUS_MULTIPLIER;
            }

            let diag_val = if is_match {
                let case_pen = if !respect_case && pi != cj {
                    CASE_MISMATCH_PENALTY
                } else {
                    0
                };
                diag_score.saturating_add(MATCH_BONUS + bonus).saturating_sub(case_pen)
            } else if ALLOW_TYPOS {
                diag_score.saturating_sub(MISMATCH_PENALTY)
            } else {
                0
            };

            // UP (skip pattern char, typos only)
            let up_val = if ALLOW_TYPOS {
                let up_cell = buf[idx(i - 1, j, cols)];
                let pen = if cell_dir(up_cell) == Dir::Diag {
                    GAP_OPEN
                } else {
                    GAP_EXTEND
                };
                cell_score(up_cell).saturating_sub(pen)
            } else {
                0
            };

            // LEFT (skip choice char)
            let left_val = {
                let left_cell = buf[idx(i, j - 1, cols)];
                let pen = if cell_dir(left_cell) == Dir::Diag {
                    GAP_OPEN
                } else {
                    GAP_EXTEND
                };
                cell_score(left_cell).saturating_sub(pen)
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

            buf[ci] = cell_pack(best, dir);
        }
    }

    // Find best in last row
    let mut best_score: Score = 0;
    let mut best_j = 0usize;
    for j in 0..cols {
        let s = cell_score(buf[idx(n, j, cols)]);
        if s > best_score {
            best_score = s;
            best_j = j;
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
        let c = buf[idx(i, j, cols)];
        match cell_dir(c) {
            Dir::Diag => {
                if eq_byte(pat[i - 1], cho[j - 1], respect_case) {
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

/// Full DP for char slices using packed cells.
fn full_dp_chars<const ALLOW_TYPOS: bool>(
    cho: &[char],
    pat: &[char],
    bonuses: &[Score],
    respect_case: bool,
    full_buf: &ThreadLocal<RefCell<Vec<Cell>>>,
) -> Option<(Score, MatchIndices)> {
    let n = pat.len();
    let m = cho.len();
    let cols = m + 1;
    let total = (n + 1) * cols;

    let mut buf = full_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
    if buf.len() < total {
        buf.resize(total, CELL_ZERO);
    }

    #[inline(always)]
    fn idx(i: usize, j: usize, cols: usize) -> usize {
        i * cols + j
    }

    for j in 0..cols {
        buf[idx(0, j, cols)] = CELL_ZERO;
    }
    for i in 1..=n {
        buf[idx(i, 0, cols)] = CELL_ZERO;
    }

    for i in 1..=n {
        let pi = pat[i - 1];
        let is_first = i == 1;

        for j in 1..=m {
            let cj = cho[j - 1];
            let is_match = eq_char(pi, cj, respect_case);
            let ci = idx(i, j, cols);

            let diag_cell = buf[idx(i - 1, j - 1, cols)];
            let diag_score = cell_score(diag_cell);
            let diag_was_diag = cell_dir(diag_cell) == Dir::Diag;

            let mut bonus = bonuses[j - 1];
            if diag_was_diag {
                bonus += CONSECUTIVE_BONUS;
            }
            if is_first {
                bonus *= FIRST_CHAR_BONUS_MULTIPLIER;
            }

            let diag_val = if is_match {
                let case_pen = if !respect_case && pi != cj {
                    CASE_MISMATCH_PENALTY
                } else {
                    0
                };
                diag_score.saturating_add(MATCH_BONUS + bonus).saturating_sub(case_pen)
            } else if ALLOW_TYPOS {
                diag_score.saturating_sub(MISMATCH_PENALTY)
            } else {
                0
            };

            let up_val = if ALLOW_TYPOS {
                let up_cell = buf[idx(i - 1, j, cols)];
                let pen = if cell_dir(up_cell) == Dir::Diag {
                    GAP_OPEN
                } else {
                    GAP_EXTEND
                };
                cell_score(up_cell).saturating_sub(pen)
            } else {
                0
            };

            let left_val = {
                let left_cell = buf[idx(i, j - 1, cols)];
                let pen = if cell_dir(left_cell) == Dir::Diag {
                    GAP_OPEN
                } else {
                    GAP_EXTEND
                };
                cell_score(left_cell).saturating_sub(pen)
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

            buf[ci] = cell_pack(best, dir);
        }
    }

    let mut best_score: Score = 0;
    let mut best_j = 0usize;
    for j in 0..cols {
        let s = cell_score(buf[idx(n, j, cols)]);
        if s > best_score {
            best_score = s;
            best_j = j;
        }
    }

    if best_score == 0 {
        return None;
    }

    let mut indices: MatchIndices = Vec::with_capacity(n);
    let mut i = n;
    let mut j = best_j;

    while i > 0 && j > 0 {
        let c = buf[idx(i, j, cols)];
        match cell_dir(c) {
            Dir::Diag => {
                if eq_char(pat[i - 1], cho[j - 1], respect_case) {
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
            let respect_case = self.respect_case_bytes(pat);

            if !self.allow_typos && !is_subsequence_bytes(pat, cho, respect_case) {
                return None;
            }
            if self.allow_typos && !cheap_typo_prefilter_bytes(pat, cho, respect_case) {
                return None;
            }

            let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
            precompute_bonuses_bytes(cho, &mut bonus_buf);

            let result = if self.allow_typos {
                score_only_bytes_colmajor::<true>(cho, pat, &bonus_buf, respect_case)
            } else {
                score_only_bytes_colmajor::<false>(cho, pat, &bonus_buf, respect_case)
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

        let respect_case = self.respect_case_chars(pat_buf);

        if !self.allow_typos && !is_subsequence_chars(pat_buf, cho_buf, respect_case) {
            return None;
        }
        if self.allow_typos && !cheap_typo_prefilter_chars(pat_buf, cho_buf, respect_case) {
            return None;
        }

        let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
        precompute_bonuses_chars(cho_buf, &mut bonus_buf);

        let result = if self.allow_typos {
            score_only_chars_colmajor::<true>(cho_buf, pat_buf, &bonus_buf, respect_case)
        } else {
            score_only_chars_colmajor::<false>(cho_buf, pat_buf, &bonus_buf, respect_case)
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
            let respect_case = self.respect_case_bytes(pat);

            if !self.allow_typos && !is_subsequence_bytes(pat, cho, respect_case) {
                return None;
            }
            if self.allow_typos && !cheap_typo_prefilter_bytes(pat, cho, respect_case) {
                return None;
            }

            let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
            precompute_bonuses_bytes(cho, &mut bonus_buf);

            let result = if self.allow_typos {
                score_only_bytes_colmajor::<true>(cho, pat, &bonus_buf, respect_case)
            } else {
                score_only_bytes_colmajor::<false>(cho, pat, &bonus_buf, respect_case)
            };
            return result.map(|(s, end_col)| {
                let end = if end_col > 0 { end_col - 1 } else { 0 };
                let begin = find_begin_byte(cho, pat, end, respect_case);
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

        let respect_case = self.respect_case_chars(pat_buf);

        if !self.allow_typos && !is_subsequence_chars(pat_buf, cho_buf, respect_case) {
            return None;
        }
        if self.allow_typos && !cheap_typo_prefilter_chars(pat_buf, cho_buf, respect_case) {
            return None;
        }

        let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
        precompute_bonuses_chars(cho_buf, &mut bonus_buf);

        let result = if self.allow_typos {
            score_only_chars_colmajor::<true>(cho_buf, pat_buf, &bonus_buf, respect_case)
        } else {
            score_only_chars_colmajor::<false>(cho_buf, pat_buf, &bonus_buf, respect_case)
        };
        result.map(|(s, end_col)| {
            let end = if end_col > 0 { end_col - 1 } else { 0 };
            let begin = find_begin_char(cho_buf, pat_buf, end, respect_case);
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
            let respect_case = self.respect_case_bytes(pat);

            if !self.allow_typos && !is_subsequence_bytes(pat, cho, respect_case) {
                return None;
            }
            if self.allow_typos && !cheap_typo_prefilter_bytes(pat, cho, respect_case) {
                return None;
            }

            let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
            precompute_bonuses_bytes(cho, &mut bonus_buf);

            let result = if self.allow_typos {
                full_dp_bytes::<true>(cho, pat, &bonus_buf, respect_case, &self.full_buf)
            } else {
                full_dp_bytes::<false>(cho, pat, &bonus_buf, respect_case, &self.full_buf)
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

        let respect_case = self.respect_case_chars(pat_buf);

        if !self.allow_typos && !is_subsequence_chars(pat_buf, cho_buf, respect_case) {
            return None;
        }
        if self.allow_typos && !cheap_typo_prefilter_chars(pat_buf, cho_buf, respect_case) {
            return None;
        }

        let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
        precompute_bonuses_chars(cho_buf, &mut bonus_buf);

        let result = if self.allow_typos {
            full_dp_chars::<true>(cho_buf, pat_buf, &bonus_buf, respect_case, &self.full_buf)
        } else {
            full_dp_chars::<false>(cho_buf, pat_buf, &bonus_buf, respect_case, &self.full_buf)
        };
        result.map(|(s, idx)| (s as ScoreType, idx))
    }

    fn batch_score_bytes(&self, items: &[&[u8]], pattern: &[u8], respect_case: bool) -> Option<Vec<Option<i32>>> {
        Some(self.batch_fuzzy_match_bytes(items, pattern, respect_case))
    }
}

// ---------------------------------------------------------------------------
// SIMD batched score-only DP (8 items at once using wide::i32x8)
// ---------------------------------------------------------------------------

const SIMD_LANES: usize = 8;
const SIMD_MAX_CHOICE_LEN: usize = 128;

use wide::{CmpEq, CmpGt, i32x8};

/// SIMD batch score-only using i32x8.
fn batch_score_only_bytes_simd<const ALLOW_TYPOS: bool>(
    choices: &[&[u8]; SIMD_LANES],
    choice_lens: &[usize; SIMD_LANES],
    bonuses: &[&[Score]; SIMD_LANES],
    pat: &[u8],
    respect_case: bool,
    batch_count: usize,
) -> [Option<i32>; SIMD_LANES] {
    let n = pat.len();
    debug_assert!(n <= COLMAJOR_MAX_N);
    debug_assert!(batch_count > 0 && batch_count <= SIMD_LANES);

    let zero_v = i32x8::splat(0);
    let match_bonus_v = i32x8::splat(MATCH_BONUS as i32);
    let mismatch_penalty_v = i32x8::splat(MISMATCH_PENALTY as i32);
    let case_mismatch_penalty_v = i32x8::splat(CASE_MISMATCH_PENALTY as i32);
    let gap_open_v = i32x8::splat(GAP_OPEN as i32);
    let gap_extend_v = i32x8::splat(GAP_EXTEND as i32);
    let consec_bonus_v = i32x8::splat(CONSECUTIVE_BONUS as i32);

    let max_m = *choice_lens.iter().take(batch_count).max().unwrap_or(&0);

    // Two columns of height (n+1)
    struct ColState {
        score: [i32x8; COLMAJOR_MAX_N + 1],
        is_diag: [i32x8; COLMAJOR_MAX_N + 1], // -1 = true, 0 = false
    }

    impl ColState {
        fn new() -> Self {
            Self {
                score: [i32x8::splat(0); COLMAJOR_MAX_N + 1],
                is_diag: [i32x8::splat(0); COLMAJOR_MAX_N + 1],
            }
        }
    }

    let mut col_a = ColState::new();
    let mut col_b = ColState::new();

    let mut prev = &mut col_a;
    let mut cur = &mut col_b;

    let mut global_best = zero_v;

    for j in 1..=max_m {
        let j_valid = {
            let mut arr = [0i32; SIMD_LANES];
            for lane in 0..batch_count {
                if j <= choice_lens[lane] {
                    arr[lane] = -1;
                }
            }
            i32x8::new(arr)
        };

        let cj_bytes = {
            let mut arr = [0i32; SIMD_LANES];
            for lane in 0..batch_count {
                if j <= choice_lens[lane] {
                    arr[lane] = choices[lane][j - 1] as i32;
                }
            }
            i32x8::new(arr)
        };

        let bonus_j = {
            let mut arr = [0i32; SIMD_LANES];
            for lane in 0..batch_count {
                if j <= choice_lens[lane] {
                    arr[lane] = bonuses[lane][j - 1] as i32;
                }
            }
            i32x8::new(arr)
        };

        cur.score[0] = zero_v;
        cur.is_diag[0] = zero_v;

        for i in 1..=n {
            let pi = pat[i - 1];
            let pi_v = i32x8::splat(pi as i32);

            // Character comparison
            let is_exact_match = if respect_case {
                pi_v.simd_eq(cj_bytes)
            } else {
                let pi_lower = i32x8::splat((pi | 0x20) as i32);
                let cj_lower = cj_bytes | i32x8::splat(0x20);
                let pi_is_alpha = if pi.is_ascii_alphabetic() {
                    i32x8::splat(-1)
                } else {
                    zero_v
                };
                let cj_is_alpha = {
                    let cj_ge_a = cj_bytes.simd_gt(i32x8::splat(b'A' as i32 - 1));
                    let cj_le_z = (i32x8::splat(b'z' as i32 + 1)).simd_gt(cj_bytes);
                    let cj_ge_la = cj_bytes.simd_gt(i32x8::splat(b'a' as i32 - 1));
                    let cj_upper = cj_ge_a & (i32x8::splat(b'Z' as i32 + 1)).simd_gt(cj_bytes);
                    cj_upper | cj_ge_la & cj_le_z
                };
                let both_alpha = pi_is_alpha & cj_is_alpha;
                let ci_match = (pi_lower.simd_eq(cj_lower)) & both_alpha;
                let exact_match = pi_v.simd_eq(cj_bytes);
                ci_match | exact_match
            };

            let cases_differ = if !respect_case {
                is_exact_match & !pi_v.simd_eq(cj_bytes)
            } else {
                zero_v
            };

            // DIAGONAL
            let diag_score = prev.score[i - 1];
            let prev_was_diag = prev.is_diag[i - 1];

            let mut bonus = bonus_j;
            bonus += prev_was_diag & consec_bonus_v;
            if i == 1 {
                bonus = bonus + bonus;
            }

            let match_score = (diag_score + match_bonus_v + bonus) - (cases_differ & case_mismatch_penalty_v);
            let mismatch_score = if ALLOW_TYPOS {
                (diag_score - mismatch_penalty_v).max(zero_v)
            } else {
                zero_v
            };
            let diag_val = is_exact_match.blend(match_score, mismatch_score).max(zero_v);

            // UP (skip pattern char, typos only)
            let up_val = if ALLOW_TYPOS {
                let up_score = cur.score[i - 1];
                let up_was_diag = cur.is_diag[i - 1];
                let penalty = up_was_diag.blend(gap_open_v, gap_extend_v);
                (up_score - penalty).max(zero_v)
            } else {
                zero_v
            };

            // LEFT (skip choice char)
            let left_score = prev.score[i];
            let left_was_diag = prev.is_diag[i];
            let penalty = left_was_diag.blend(gap_open_v, gap_extend_v);
            let left_val = (left_score - penalty).max(zero_v);

            let best = diag_val.max(up_val).max(left_val);

            // Track direction
            let diag_won = best.simd_gt(zero_v) & diag_val.simd_gt(up_val.max(left_val) - i32x8::splat(1));
            let is_diag_mask = if ALLOW_TYPOS {
                diag_won
            } else {
                diag_won & is_exact_match
            };

            cur.score[i] = j_valid.blend(best, zero_v);
            cur.is_diag[i] = j_valid.blend(is_diag_mask, zero_v);
        }

        let row_n = cur.score[n];
        global_best = global_best.max(row_n);

        std::mem::swap(&mut prev, &mut cur);
    }

    let scores = global_best.to_array();
    let mut result = [None; SIMD_LANES];
    for lane in 0..batch_count {
        if scores[lane] > 0 {
            result[lane] = Some(scores[lane]);
        }
    }
    result
}

/// Public batch scoring API.
impl SkimV3Matcher {
    /// Batch score-only fuzzy matching for byte strings using SIMD.
    pub fn batch_fuzzy_match_bytes(&self, items: &[&[u8]], pattern: &[u8], respect_case: bool) -> Vec<Option<i32>> {
        let n = pattern.len();
        if n == 0 {
            return items.iter().map(|_| Some(0)).collect();
        }
        if n > COLMAJOR_MAX_N {
            return items
                .iter()
                .map(|cho| {
                    let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
                    precompute_bonuses_bytes(cho, &mut bonus_buf);
                    let result = if self.allow_typos {
                        score_only_bytes_colmajor::<true>(cho, pattern, &bonus_buf, respect_case)
                    } else {
                        score_only_bytes_colmajor::<false>(cho, pattern, &bonus_buf, respect_case)
                    };
                    result.map(|(s, _)| s as i32)
                })
                .collect();
        }

        let mut results: Vec<Option<i32>> = Vec::with_capacity(items.len());
        let empty: &[u8] = &[];
        let empty_bonus: &[Score] = &[];

        let mut bonus_storage: Vec<Vec<Score>> = Vec::with_capacity(SIMD_LANES);
        for _ in 0..SIMD_LANES {
            bonus_storage.push(Vec::new());
        }

        let mut idx = 0;
        while idx < items.len() {
            let batch_count = (items.len() - idx).min(SIMD_LANES);

            let mut choices: [&[u8]; SIMD_LANES] = [empty; SIMD_LANES];
            let mut choice_lens: [usize; SIMD_LANES] = [0; SIMD_LANES];
            let mut scalar_fallback: [bool; SIMD_LANES] = [false; SIMD_LANES];

            for lane in 0..batch_count {
                let cho = items[idx + lane];
                let pass = if self.allow_typos {
                    cheap_typo_prefilter_bytes(pattern, cho, respect_case)
                } else {
                    is_subsequence_bytes(pattern, cho, respect_case)
                };

                if !pass {
                    scalar_fallback[lane] = true;
                    continue;
                }
                if cho.len() > SIMD_MAX_CHOICE_LEN {
                    scalar_fallback[lane] = true;
                    continue;
                }

                choices[lane] = cho;
                choice_lens[lane] = cho.len();
                precompute_bonuses_bytes(cho, &mut bonus_storage[lane]);
            }

            let mut bonus_refs: [&[Score]; SIMD_LANES] = [empty_bonus; SIMD_LANES];
            for lane in 0..batch_count {
                if !scalar_fallback[lane] {
                    bonus_refs[lane] = &bonus_storage[lane];
                }
            }

            let active_count = (0..batch_count).filter(|&l| !scalar_fallback[l]).count();

            if active_count == 0 {
                for lane in 0..batch_count {
                    if scalar_fallback[lane] && items[idx + lane].len() > SIMD_MAX_CHOICE_LEN {
                        let cho = items[idx + lane];
                        let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
                        precompute_bonuses_bytes(cho, &mut bonus_buf);
                        let result = if self.allow_typos {
                            score_only_bytes_colmajor::<true>(cho, pattern, &bonus_buf, respect_case)
                        } else {
                            score_only_bytes_colmajor::<false>(cho, pattern, &bonus_buf, respect_case)
                        };
                        results.push(result.map(|(s, _)| s as i32));
                    } else {
                        results.push(None);
                    }
                }
            } else {
                let batch_results = if self.allow_typos {
                    batch_score_only_bytes_simd::<true>(
                        &choices,
                        &choice_lens,
                        &bonus_refs,
                        pattern,
                        respect_case,
                        SIMD_LANES,
                    )
                } else {
                    batch_score_only_bytes_simd::<false>(
                        &choices,
                        &choice_lens,
                        &bonus_refs,
                        pattern,
                        respect_case,
                        SIMD_LANES,
                    )
                };

                for lane in 0..batch_count {
                    if scalar_fallback[lane] && items[idx + lane].len() > SIMD_MAX_CHOICE_LEN {
                        let cho = items[idx + lane];
                        let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
                        precompute_bonuses_bytes(cho, &mut bonus_buf);
                        let result = if self.allow_typos {
                            score_only_bytes_colmajor::<true>(cho, pattern, &bonus_buf, respect_case)
                        } else {
                            score_only_bytes_colmajor::<false>(cho, pattern, &bonus_buf, respect_case)
                        };
                        results.push(result.map(|(s, _)| s as i32));
                    } else if scalar_fallback[lane] {
                        results.push(None);
                    } else {
                        results.push(batch_results[lane]);
                    }
                }
            }

            idx += batch_count;
        }

        results
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

    // ----- SIMD batch matching -----

    fn to_score32(v: Option<i64>) -> Option<i32> {
        v.map(|x| x as i32)
    }

    #[test]
    fn batch_matches_scalar_no_typos() {
        let m = matcher();
        let pattern = b"test";
        let items: Vec<&[u8]> = vec![
            b"test",
            b"testing",
            b"attest",
            b"best",
            b"the_test_file",
            b"no_match_here",
            b"src/test/main.rs",
            b"contest",
        ];

        let batch_results = m.batch_fuzzy_match_bytes(&items, pattern, false);
        assert_eq!(batch_results.len(), items.len());
        for (i, item) in items.iter().enumerate() {
            let scalar = to_score32(m.fuzzy_match(std::str::from_utf8(item).unwrap(), "test"));
            assert_eq!(
                batch_results[i],
                scalar,
                "mismatch for item {:?}: batch={:?} scalar={:?}",
                std::str::from_utf8(item).unwrap(),
                batch_results[i],
                scalar
            );
        }
    }

    #[test]
    fn batch_matches_scalar_typos() {
        let m = matcher_typos();
        let pattern = b"test";
        let items: Vec<&[u8]> = vec![
            b"test",
            b"tset",
            b"tast",
            b"testing",
            b"the_test_file",
            b"xxxx",
            b"contest",
            b"attest",
        ];

        let batch_results = m.batch_fuzzy_match_bytes(&items, pattern, false);
        assert_eq!(batch_results.len(), items.len());
        for (i, item) in items.iter().enumerate() {
            let scalar = to_score32(m.fuzzy_match(std::str::from_utf8(item).unwrap(), "test"));
            assert_eq!(
                batch_results[i],
                scalar,
                "mismatch for item {:?}: batch={:?} scalar={:?}",
                std::str::from_utf8(item).unwrap(),
                batch_results[i],
                scalar
            );
        }
    }

    #[test]
    fn batch_fewer_than_8_items() {
        let m = matcher_typos();
        let pattern = b"abc";
        let items: Vec<&[u8]> = vec![b"abc", b"abxc", b"xyz"];

        let batch_results = m.batch_fuzzy_match_bytes(&items, pattern, false);
        assert_eq!(batch_results.len(), 3);
        for (i, item) in items.iter().enumerate() {
            let scalar = to_score32(m.fuzzy_match(std::str::from_utf8(item).unwrap(), "abc"));
            assert_eq!(
                batch_results[i],
                scalar,
                "mismatch for item {:?}: batch={:?} scalar={:?}",
                std::str::from_utf8(item).unwrap(),
                batch_results[i],
                scalar
            );
        }
    }

    #[test]
    fn batch_more_than_8_items() {
        let m = matcher_typos();
        let pattern = b"rs";
        let items: Vec<&[u8]> = vec![
            b"src/main.rs",
            b"lib.rs",
            b"Cargo.toml",
            b"README.md",
            b"test.rs",
            b"foo/bar.rs",
            b"no_match",
            b"baz.rs",
            b"extra1.rs",
            b"extra2.txt",
            b"result.rs",
        ];

        let batch_results = m.batch_fuzzy_match_bytes(&items, pattern, false);
        assert_eq!(batch_results.len(), items.len());
        for (i, item) in items.iter().enumerate() {
            let scalar = to_score32(m.fuzzy_match(std::str::from_utf8(item).unwrap(), "rs"));
            assert_eq!(
                batch_results[i],
                scalar,
                "mismatch for item {:?}: batch={:?} scalar={:?}",
                std::str::from_utf8(item).unwrap(),
                batch_results[i],
                scalar
            );
        }
    }

    #[test]
    fn batch_empty_pattern() {
        let m = matcher();
        let items: Vec<&[u8]> = vec![b"foo", b"bar"];
        let batch_results = m.batch_fuzzy_match_bytes(&items, b"", false);
        assert_eq!(batch_results.len(), 2);
        for r in &batch_results {
            assert_eq!(*r, Some(0));
        }
    }

    #[test]
    fn batch_single_char_pattern() {
        let m = matcher_typos();
        let pattern = b"x";
        let items: Vec<&[u8]> = vec![b"fox", b"xyz", b"aaa", b"box", b"hex", b"mmm", b"x", b"xx"];

        let batch_results = m.batch_fuzzy_match_bytes(&items, pattern, false);
        assert_eq!(batch_results.len(), items.len());
        for (i, item) in items.iter().enumerate() {
            let scalar = to_score32(m.fuzzy_match(std::str::from_utf8(item).unwrap(), "x"));
            assert_eq!(
                batch_results[i],
                scalar,
                "mismatch for item {:?}: batch={:?} scalar={:?}",
                std::str::from_utf8(item).unwrap(),
                batch_results[i],
                scalar
            );
        }
    }

    #[test]
    fn batch_varied_lengths() {
        let m = matcher_typos();
        let pattern = b"test";
        let items: Vec<&[u8]> = vec![
            b"t",
            b"te",
            b"tes",
            b"test",
            b"this_is_a_fairly_long_path/to/some/test/file",
            b"a/very/deeply/nested/directory/structure/with/test/somewhere/in/it.txt",
            b"x",
            b"test_test_test_test",
        ];

        let batch_results = m.batch_fuzzy_match_bytes(&items, pattern, false);
        assert_eq!(batch_results.len(), items.len());
        for (i, item) in items.iter().enumerate() {
            let scalar = to_score32(m.fuzzy_match(std::str::from_utf8(item).unwrap(), "test"));
            assert_eq!(
                batch_results[i],
                scalar,
                "mismatch for item {:?} (len={}): batch={:?} scalar={:?}",
                std::str::from_utf8(item).unwrap(),
                item.len(),
                batch_results[i],
                scalar
            );
        }
    }

    #[test]
    fn batch_case_sensitive() {
        let m = SkimV3Matcher::new(CaseMatching::Respect, true);
        let pattern = b"Test";
        let items: Vec<&[u8]> = vec![b"Test", b"test", b"TEST", b"Testing", b"testing"];

        let batch_results = m.batch_fuzzy_match_bytes(&items, pattern, true);
        assert_eq!(batch_results.len(), items.len());
        for (i, item) in items.iter().enumerate() {
            let scalar = to_score32(
                m.fuzzy_indices(std::str::from_utf8(item).unwrap(), "Test")
                    .map(|(s, _)| s),
            );
            assert_eq!(
                batch_results[i],
                scalar,
                "mismatch for item {:?}: batch={:?} scalar={:?}",
                std::str::from_utf8(item).unwrap(),
                batch_results[i],
                scalar
            );
        }
    }

    #[test]
    fn batch_stress_test_against_scalar() {
        let patterns: &[&[u8]] = &[b"ab", b"test", b"xyz", b"rs", b"a", b"main", b"src"];
        let items: Vec<&[u8]> = vec![
            b"src/main.rs",
            b"test/foo.rs",
            b"lib.rs",
            b"Cargo.toml",
            b"README.md",
            b"a",
            b"ab",
            b"abc",
            b"abcd",
            b"abcde",
            b"xyzzy",
            b"foo/bar/baz/qux.txt",
            b"hello_world.py",
            b"test_test_test",
            b"aaaa",
            b"zzzz",
            b"the_quick_brown_fox_jumps_over_the_lazy_dog",
            b"src/engine/fuzzy.rs",
            b"benches/filter.rs",
            b"some/very/deep/nested/path/to/a/file/named/test.rs",
        ];

        for allow_typos in [false, true] {
            let m = SkimV3Matcher::new(CaseMatching::Ignore, allow_typos);
            for pat in patterns {
                let batch_results = m.batch_fuzzy_match_bytes(&items, pat, false);
                assert_eq!(batch_results.len(), items.len());
                for (i, item) in items.iter().enumerate() {
                    let scalar = to_score32(
                        m.fuzzy_match(std::str::from_utf8(item).unwrap(), std::str::from_utf8(pat).unwrap()),
                    );
                    assert_eq!(
                        batch_results[i],
                        scalar,
                        "mismatch: pattern={:?} item={:?} typos={} batch={:?} scalar={:?}",
                        std::str::from_utf8(pat).unwrap(),
                        std::str::from_utf8(item).unwrap(),
                        allow_typos,
                        batch_results[i],
                        scalar
                    );
                }
            }
        }
    }
}
