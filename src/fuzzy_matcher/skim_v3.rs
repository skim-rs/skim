//! SkimV3 fuzzy matching algorithm.
//!
//! Combines the Needleman-Wunsch sequence alignment framework with a variation
//! of Damerau-Levenshtein distance that supports:
//!
//! - **Affine gap penalties** (Gotoh-style): opening a gap is expensive,
//!   extending one is cheap.  Separate penalties for insertion gaps (skipping
//!   choice characters) and deletion gaps (skipping pattern characters).
//! - **Transpositions**: adjacent swaps in the pattern (`ab` → `ba`) are
//!   recognised and penalised less than two independent mismatches.
//! - **Context bonuses**: matches at word boundaries, after separators, at
//!   camelCase transitions, at the start of the string, and in consecutive
//!   runs all receive bonus points, so that the alignment naturally prefers
//!   "meaningful" positions.
//!
//! ## Performance
//!
//! The implementation is heavily optimised for throughput:
//!
//! - **Byte-level fast path**: when both pattern and choice are ASCII, the DP
//!   operates directly on `&[u8]` slices — no `Vec<char>` allocation needed.
//! - **Flat 1-D buffer** with `ThreadLocal` reuse eliminates per-call
//!   allocations.
//! - **Score-only fast path** (`fuzzy_match`) uses a 3-row sliding window
//!   instead of the full `(n+1) × (m+1)` matrix, cutting memory by ~`n/3`×.
//! - **Precomputed bonus array** avoids redundant character classification in
//!   the inner loop.
//! - **Compact cell layout** (12 bytes) maximises cache-line utilisation.
//! - **Early rejection**: a cheap subsequence / length check avoids entering
//!   the DP for obvious non-matches.

use std::cell::RefCell;

use thread_local::ThreadLocal;

use crate::{
    CaseMatching,
    fuzzy_matcher::{FuzzyMatcher, IndexType, MatchIndices, ScoreType},
};

// ---------------------------------------------------------------------------
// Scoring constants
// ---------------------------------------------------------------------------

type Score = i32;

/// Points awarded for each correctly matched character.
const MATCH_BONUS: Score = 16;

/// Extra bonus when the match is at position 0 of the choice string.
const START_OF_STRING_BONUS: Score = 12;

/// Extra bonus when the match follows a word separator.
const START_OF_WORD_BONUS: Score = 10;

/// Extra bonus for a camelCase transition.
const CAMEL_CASE_BONUS: Score = 8;

/// Bonus for each additional consecutive matched character.
const CONSECUTIVE_BONUS: Score = 6;

/// Multiplier applied to the very first pattern character's positional bonus.
const FIRST_CHAR_BONUS_MULTIPLIER: Score = 2;

/// Penalty for a case-insensitive match where the cases differ.
const CASE_MISMATCH_PENALTY: Score = 2;

// --- Insertion gaps (skipping choice characters) ---

/// Cost to open an insertion gap (transition from M → gap).
const INS_OPEN: Score = 8;

/// Cost to extend an insertion gap by one more choice character.
const INS_EXTEND: Score = 1;

// --- Deletion gaps (skipping pattern characters – typos-on only) ---

/// Cost to open a deletion gap.
const DEL_OPEN: Score = 10;

/// Cost to extend a deletion gap.
const DEL_EXTEND: Score = 2;

// --- Substitution / transposition (typos-on only) ---

/// Penalty for aligning a pattern character to a different choice character.
const MISMATCH_PENALTY: Score = 10;

/// Penalty for a transposition (adjacent swap: `ab` ↔ `ba`).
const TRANSPOSITION_PENALTY: Score = 6;

/// Sentinel that won't overflow when subtracted from.
const NEG_INF: Score = Score::MIN / 2;

// ---------------------------------------------------------------------------
// Byte-level helpers
// ---------------------------------------------------------------------------

/// Compare two bytes for equality, optionally ignoring ASCII case.
#[inline(always)]
fn eq_byte(a: u8, b: u8, respect_case: bool) -> bool {
    if respect_case {
        a == b
    } else {
        a.eq_ignore_ascii_case(&b)
    }
}

/// Compute the match-type encoding for aligning byte `p` (pattern) with byte
/// `c` (choice).  Returns:
///   2  = exact-case match
///   1  = case-insensitive match (cases differ)
///  -1  = mismatch (only when allow_typos)
///   0  = forbidden (no match and typos disabled)
#[inline(always)]
fn match_type(p: u8, c: u8, respect_case: bool, allow_typos: bool) -> i8 {
    if eq_byte(p, c, respect_case) {
        if respect_case || p == c { 2 } else { 1 }
    } else if allow_typos {
        -1
    } else {
        0
    }
}

/// Compute a positional bonus for matching at choice position `j`.
#[inline(always)]
fn context_bonus_precomputed(
    base_bonus: Score,
    prev_was_consecutive_match: bool,
    is_first_pattern_char: bool,
) -> Score {
    let mut bonus = base_bonus;
    if prev_was_consecutive_match {
        bonus += CONSECUTIVE_BONUS;
    }
    if is_first_pattern_char {
        bonus *= FIRST_CHAR_BONUS_MULTIPLIER;
    }
    bonus
}

/// Precompute per-position base bonuses for a byte-slice choice string.
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

/// Precompute per-position base bonuses for a char-slice choice string.
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
// Traceback enum (only used when indices are needed)
// ---------------------------------------------------------------------------

/// Which table/operation produced the best predecessor for M[i][j].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[allow(dead_code)]
enum Origin {
    M = 0,
    D = 1,
    Ins = 2,
    Trans = 3,
    None = 4,
}

// ---------------------------------------------------------------------------
// Compact DP cell — 16 bytes, fits 4 per cache line
// ---------------------------------------------------------------------------

/// Score-only cell (no traceback info).
/// 12 bytes + 4 padding = 16 bytes, fits 4 per cache line.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct ScoreCell {
    m: Score,
    d: Score,
    /// Combined insertion gap score: max of "opened from M" and "extended".
    ins: Score,
}

impl ScoreCell {
    const INIT: Self = Self {
        m: NEG_INF,
        d: NEG_INF,
        ins: NEG_INF,
    };

    #[inline(always)]
    fn best(&self) -> Score {
        self.m.max(self.d).max(self.ins)
    }
}

// NOTE: The full DP now uses ScoreCell directly (12 bytes per cell, ~5 per
// cache line) instead of the old 16-byte FullCell.  The traceback origin
// (`m_origin`) is recomputed during the O(n+m) traceback pass rather than
// stored during the O(n*m) fill, eliminating ~n*m store instructions.

// ---------------------------------------------------------------------------
// SkimV3Matcher
// ---------------------------------------------------------------------------

/// SkimV3 fuzzy matcher: Needleman-Wunsch with Damerau-Levenshtein variation,
/// affine gap penalties, and context-sensitive bonuses.
#[derive(Debug, Default)]
pub struct SkimV3Matcher {
    /// Case matching strategy.
    pub(crate) case: CaseMatching,
    /// When `false`, deletion gaps, mismatches, and transpositions are
    /// forbidden.
    pub(crate) allow_typos: bool,
    /// Reusable buffers (per-thread).
    score_buf: ThreadLocal<RefCell<Vec<ScoreCell>>>,
    full_buf: ThreadLocal<RefCell<Vec<ScoreCell>>>,
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

    /// Resolve case sensitivity for a byte-level pattern.
    #[inline]
    fn respect_case_bytes(&self, pattern: &[u8]) -> bool {
        self.case == CaseMatching::Respect
            || (self.case == CaseMatching::Smart && pattern.iter().any(|b| b.is_ascii_uppercase()))
    }

    /// Resolve case sensitivity for a char-level pattern.
    #[inline]
    fn respect_case_chars(&self, pattern: &[char]) -> bool {
        self.case == CaseMatching::Respect
            || (self.case == CaseMatching::Smart && pattern.iter().any(|c| c.is_uppercase()))
    }

    // =======================================================================
    // Byte-level score-only fast path (3-row sliding window)
    // =======================================================================

    fn score_only_bytes(
        &self,
        cho: &[u8],
        pat: &[u8],
        bonuses: &[Score],
        respect_case: bool,
    ) -> Option<(Score, usize)> {
        if self.allow_typos {
            score_only_bytes_inner::<true>(cho, pat, bonuses, respect_case, &self.score_buf)
        } else {
            score_only_bytes_inner::<false>(cho, pat, bonuses, respect_case, &self.score_buf)
        }
    }

    // =======================================================================
    // Byte-level full DP with traceback
    // =======================================================================

    fn full_dp_bytes(
        &self,
        cho: &[u8],
        pat: &[u8],
        bonuses: &[Score],
        respect_case: bool,
    ) -> Option<(Score, MatchIndices)> {
        if self.allow_typos {
            full_dp_bytes_inner::<true>(cho, pat, bonuses, respect_case, &self.full_buf)
        } else {
            full_dp_bytes_inner::<false>(cho, pat, bonuses, respect_case, &self.full_buf)
        }
    }

    // =======================================================================
    // Char-level DP (fallback for non-ASCII)
    // =======================================================================

    fn score_only_chars(
        &self,
        cho: &[char],
        pat: &[char],
        bonuses: &[Score],
        respect_case: bool,
    ) -> Option<(Score, usize)> {
        if self.allow_typos {
            score_only_chars_inner::<true>(cho, pat, bonuses, respect_case, &self.score_buf)
        } else {
            score_only_chars_inner::<false>(cho, pat, bonuses, respect_case, &self.score_buf)
        }
    }

    fn full_dp_chars(
        &self,
        cho: &[char],
        pat: &[char],
        bonuses: &[Score],
        respect_case: bool,
    ) -> Option<(Score, MatchIndices)> {
        if self.allow_typos {
            full_dp_chars_inner::<true>(cho, pat, bonuses, respect_case, &self.full_buf)
        } else {
            full_dp_chars_inner::<false>(cho, pat, bonuses, respect_case, &self.full_buf)
        }
    }
}

// ---------------------------------------------------------------------------
// Full DP with traceback — const-generic free functions
// ---------------------------------------------------------------------------

/// Byte-level full DP. Const-generic `ALLOW_TYPOS` eliminates dead branches.
fn full_dp_bytes_inner<const ALLOW_TYPOS: bool>(
    cho: &[u8],
    pat: &[u8],
    bonuses: &[Score],
    respect_case: bool,
    full_buf: &ThreadLocal<RefCell<Vec<ScoreCell>>>,
) -> Option<(Score, MatchIndices)> {
    let n = pat.len();
    let m = cho.len();
    let cols = m + 1;
    let total = (n + 1) * cols;

    let mut buf = full_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
    if buf.len() < total {
        buf.resize(total, ScoreCell::INIT);
    }
    // For ALLOW_TYPOS=true, every cell in rows 1..=n gets ALL fields written
    // in the inner loop, so we only need to initialize row 0 and column 0.
    // For ALLOW_TYPOS=false, some cells may not get m written,
    // so we must fill the entire buffer.
    if !ALLOW_TYPOS {
        buf[..total].fill(ScoreCell::INIT);
    }

    #[inline(always)]
    fn idx(i: usize, j: usize, cols: usize) -> usize {
        i * cols + j
    }

    // Base case: row 0 — free alignment start at any position in choice
    buf[idx(0, 0, cols)] = ScoreCell {
        m: 0,
        d: NEG_INF,
        ins: NEG_INF,
    };
    for j in 1..cols {
        buf[idx(0, j, cols)] = ScoreCell {
            m: 0,
            d: NEG_INF,
            ins: 0,
        };
    }

    // Initialize column 0 for rows 1..=n (never written by inner loop)
    for i in 1..=n {
        buf[idx(i, 0, cols)] = ScoreCell::INIT;
    }

    for i in 1..=n {
        let pi = pat[i - 1];

        // Cache previous-column values to reduce buffer reads.
        // left_* tracks buf[i][j-1] which was just written.
        let mut left_m = NEG_INF; // buf[idx(i, 0, cols)].m = NEG_INF
        let mut left_ins = NEG_INF; // buf[idx(i, 0, cols)].ins = NEG_INF

        for j in 1..=m {
            let ci = idx(i, j, cols);
            let cj = cho[j - 1];

            // --- D (deletion gap: skip pattern char) ---
            if ALLOW_TYPOS {
                let above = idx(i - 1, j, cols);
                let d_open = buf[above].m - DEL_OPEN;
                let d_ext = buf[above].d - DEL_EXTEND;
                buf[ci].d = d_open.max(d_ext);
            }

            // --- Insertion (skip choice char) ---
            let ins_from_m = left_m - INS_OPEN;
            let ins_ext = left_ins - INS_EXTEND;
            let ins_val = ins_from_m.max(ins_ext);
            buf[ci].ins = ins_val;

            // --- M (match/mismatch: align pat[i-1] with cho[j-1]) ---
            let diag = idx(i - 1, j - 1, cols);
            let pred_m = buf[diag].m;
            let pred_d = if ALLOW_TYPOS { buf[diag].d } else { NEG_INF };
            let pred_ins = buf[diag].ins;

            let (best_pred, _pred_origin) = best_of_three(pred_m, pred_d, pred_ins);

            if ALLOW_TYPOS {
                let mt = match_type(pi, cj, respect_case, true);

                let align_score = if mt > 0 {
                    let from_m = _pred_origin == Origin::M;
                    let bonus = context_bonus_precomputed(bonuses[j - 1], from_m, i == 1);
                    let case_pen = if mt == 1 { CASE_MISMATCH_PENALTY } else { 0 };
                    best_pred + MATCH_BONUS + bonus - case_pen
                } else {
                    best_pred - MISMATCH_PENALTY
                };

                buf[ci].m = align_score;

                // Update left-column cache
                left_m = align_score;
                left_ins = ins_val;
            } else if best_pred > NEG_INF {
                let mt = match_type(pi, cj, respect_case, false);
                if mt > 0 {
                    let from_m = _pred_origin == Origin::M;
                    let bonus = context_bonus_precomputed(bonuses[j - 1], from_m, i == 1);
                    let case_pen = if mt == 1 { CASE_MISMATCH_PENALTY } else { 0 };
                    let align_score = best_pred + MATCH_BONUS + bonus - case_pen;
                    buf[ci].m = align_score;
                    left_m = align_score;
                    left_ins = ins_val;
                } else {
                    // No match, M stays at NEG_INF (from INIT)
                    left_m = NEG_INF;
                    left_ins = ins_val;
                }
            } else {
                left_m = NEG_INF;
                left_ins = ins_val;
            }

            // --- Transposition ---
            if ALLOW_TYPOS && i >= 2 && j >= 2 {
                let t1 = eq_byte(pi, cho[j - 2], respect_case);
                let t2 = eq_byte(pat[i - 2], cj, respect_case);
                if t1 && t2 {
                    let diag2 = idx(i - 2, j - 2, cols);
                    let ts = buf[diag2].best() - TRANSPOSITION_PENALTY;
                    if ts > buf[ci].m {
                        buf[ci].m = ts;
                        left_m = ts; // Update cache
                    }
                }
            }
        }
    }

    // Find best score and column in last row (single pass)
    let last_row_start = idx(n, 0, cols);
    let mut best_score = NEG_INF;
    let mut best_j = 0usize;
    for j in 0..cols {
        let s = buf[last_row_start + j].best();
        if s > best_score {
            best_score = s;
            best_j = j;
        }
    }

    if best_score <= 0 {
        return None;
    }

    // Traceback — recompute m_origin on the fly (O(n+m) steps, negligible)
    let mut j = best_j;
    let mut i = n;
    let mut indices: MatchIndices = MatchIndices::with_capacity(n);

    let end_cell = &buf[idx(n, j, cols)];
    let end_best = end_cell.best();
    let mut cur = if end_cell.m == end_best {
        Origin::M
    } else if end_cell.d == end_best {
        Origin::D
    } else {
        Origin::Ins
    };

    while i > 0 && j > 0 {
        match cur {
            Origin::Trans => {
                if i >= 2 && j >= 2 {
                    indices.push((j - 1) as IndexType);
                    indices.push((j - 2) as IndexType);
                    i -= 2;
                    j -= 2;
                    cur = Origin::M;
                } else {
                    break;
                }
            }
            Origin::M => {
                if eq_byte(pat[i - 1], cho[j - 1], respect_case) {
                    indices.push((j - 1) as IndexType);
                }

                // Recompute m_origin for this cell.
                let next = recompute_m_origin_bytes::<ALLOW_TYPOS>(&buf, cho, pat, bonuses, respect_case, i, j, cols);
                i -= 1;
                j -= 1;
                cur = next;
            }
            Origin::D => {
                let d_val = buf[idx(i, j, cols)].d;
                let above = idx(i - 1, j, cols);
                let from_m = buf[above].m > NEG_INF && (buf[above].m - DEL_OPEN) == d_val;
                i -= 1;
                cur = if from_m { Origin::M } else { Origin::D };
            }
            Origin::Ins => {
                // Recompute ins_from_m during traceback (O(n+m) total, negligible cost).
                let left = idx(i, j - 1, cols);
                let ins_from_m = buf[left].m - INS_OPEN;
                let ins_val = buf[idx(i, j, cols)].ins;
                if ins_from_m >= ins_val {
                    j -= 1;
                    cur = Origin::M;
                } else {
                    j -= 1;
                    cur = Origin::Ins;
                }
            }
            Origin::None => break,
        }
    }

    indices.sort_unstable();
    indices.dedup();

    Some((best_score, indices))
}

/// Char-level full DP. Const-generic `ALLOW_TYPOS` eliminates dead branches.
fn full_dp_chars_inner<const ALLOW_TYPOS: bool>(
    cho: &[char],
    pat: &[char],
    bonuses: &[Score],
    respect_case: bool,
    full_buf: &ThreadLocal<RefCell<Vec<ScoreCell>>>,
) -> Option<(Score, MatchIndices)> {
    let n = pat.len();
    let m = cho.len();
    let cols = m + 1;
    let total = (n + 1) * cols;

    let mut buf = full_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
    if buf.len() < total {
        buf.resize(total, ScoreCell::INIT);
    }
    if !ALLOW_TYPOS {
        buf[..total].fill(ScoreCell::INIT);
    }

    #[inline(always)]
    fn idx(i: usize, j: usize, cols: usize) -> usize {
        i * cols + j
    }

    buf[idx(0, 0, cols)] = ScoreCell {
        m: 0,
        d: NEG_INF,
        ins: NEG_INF,
    };
    for j in 1..cols {
        buf[idx(0, j, cols)] = ScoreCell {
            m: 0,
            d: NEG_INF,
            ins: 0,
        };
    }

    for i in 1..=n {
        buf[idx(i, 0, cols)] = ScoreCell::INIT;
    }

    for i in 1..=n {
        let pi = pat[i - 1];

        // Cache previous-column values to reduce buffer reads.
        // left_* tracks buf[i][j-1] which was just written.
        let mut left_m = NEG_INF; // buf[idx(i, 0, cols)].m = NEG_INF
        let mut left_ins = NEG_INF; // buf[idx(i, 0, cols)].ins = NEG_INF

        for j in 1..=m {
            let ci = idx(i, j, cols);
            let cj = cho[j - 1];

            // --- D (deletion gap: skip pattern char) ---
            if ALLOW_TYPOS {
                let above = idx(i - 1, j, cols);
                let d_open = buf[above].m - DEL_OPEN;
                let d_ext = buf[above].d - DEL_EXTEND;
                buf[ci].d = d_open.max(d_ext);
            }

            // --- Insertion (skip choice char) ---
            let ins_from_m = left_m - INS_OPEN;
            let ins_ext = left_ins - INS_EXTEND;
            let ins_val = ins_from_m.max(ins_ext);
            buf[ci].ins = ins_val;

            // --- M (match/mismatch: align pat[i-1] with cho[j-1]) ---
            let diag = idx(i - 1, j - 1, cols);
            let pred_m = buf[diag].m;
            let pred_d = if ALLOW_TYPOS { buf[diag].d } else { NEG_INF };
            let pred_ins = buf[diag].ins;

            let (best_pred, _pred_origin) = best_of_three(pred_m, pred_d, pred_ins);

            if ALLOW_TYPOS {
                let is_match = eq_char(pi, cj, respect_case);

                let align_score = if is_match {
                    let from_m = _pred_origin == Origin::M;
                    let bonus = context_bonus_precomputed(bonuses[j - 1], from_m, i == 1);
                    let case_pen = if !respect_case && pi != cj {
                        CASE_MISMATCH_PENALTY
                    } else {
                        0
                    };
                    best_pred + MATCH_BONUS + bonus - case_pen
                } else {
                    best_pred - MISMATCH_PENALTY
                };

                buf[ci].m = align_score;

                // Update left-column cache
                left_m = align_score;
                left_ins = ins_val;
            } else if best_pred > NEG_INF {
                let is_match = eq_char(pi, cj, respect_case);
                if is_match {
                    let from_m = _pred_origin == Origin::M;
                    let bonus = context_bonus_precomputed(bonuses[j - 1], from_m, i == 1);
                    let case_pen = if !respect_case && pi != cj {
                        CASE_MISMATCH_PENALTY
                    } else {
                        0
                    };
                    let align_score = best_pred + MATCH_BONUS + bonus - case_pen;
                    buf[ci].m = align_score;
                    left_m = align_score;
                    left_ins = ins_val;
                } else {
                    // No match, M stays at NEG_INF (from INIT)
                    left_m = NEG_INF;
                    left_ins = ins_val;
                }
            } else {
                left_m = NEG_INF;
                left_ins = ins_val;
            }

            // --- Transposition ---
            if ALLOW_TYPOS
                && i >= 2
                && j >= 2
                && eq_char(pi, cho[j - 2], respect_case)
                && eq_char(pat[i - 2], cj, respect_case)
            {
                let diag2 = idx(i - 2, j - 2, cols);
                let ts = buf[diag2].best() - TRANSPOSITION_PENALTY;
                if ts > buf[ci].m {
                    buf[ci].m = ts;
                    left_m = ts; // Update cache
                }
            }
        }
    }

    // Find best score and column in last row (single pass)
    let last_row_start = idx(n, 0, cols);
    let mut best_score = NEG_INF;
    let mut best_j = 0usize;
    for j in 0..cols {
        let s = buf[last_row_start + j].best();
        if s > best_score {
            best_score = s;
            best_j = j;
        }
    }

    if best_score <= 0 {
        return None;
    }

    let mut j = best_j;
    let mut i = n;
    let mut indices: MatchIndices = MatchIndices::with_capacity(n);

    let end_cell = &buf[idx(n, j, cols)];
    let end_best = end_cell.best();
    let mut cur = if end_cell.m == end_best {
        Origin::M
    } else if end_cell.d == end_best {
        Origin::D
    } else {
        Origin::Ins
    };

    while i > 0 && j > 0 {
        match cur {
            Origin::Trans => {
                if i >= 2 && j >= 2 {
                    indices.push((j - 1) as IndexType);
                    indices.push((j - 2) as IndexType);
                    i -= 2;
                    j -= 2;
                    cur = Origin::M;
                } else {
                    break;
                }
            }
            Origin::M => {
                if eq_char(pat[i - 1], cho[j - 1], respect_case) {
                    indices.push((j - 1) as IndexType);
                }

                // Recompute m_origin for this cell.
                let next = recompute_m_origin_chars::<ALLOW_TYPOS>(&buf, cho, pat, bonuses, respect_case, i, j, cols);
                i -= 1;
                j -= 1;
                cur = next;
            }
            Origin::D => {
                let d_val = buf[idx(i, j, cols)].d;
                let above = idx(i - 1, j, cols);
                let from_m = buf[above].m > NEG_INF && (buf[above].m - DEL_OPEN) == d_val;
                i -= 1;
                cur = if from_m { Origin::M } else { Origin::D };
            }
            Origin::Ins => {
                let left = idx(i, j - 1, cols);
                let ins_from_m = buf[left].m - INS_OPEN;
                let ins_val = buf[idx(i, j, cols)].ins;
                if ins_from_m >= ins_val {
                    j -= 1;
                    cur = Origin::M;
                } else {
                    j -= 1;
                    cur = Origin::Ins;
                }
            }
            Origin::None => break,
        }
    }

    indices.sort_unstable();
    indices.dedup();

    Some((best_score, indices))
}

impl FuzzyMatcher for SkimV3Matcher {
    fn fuzzy_match(&self, choice: &str, pattern: &str) -> Option<ScoreType> {
        if pattern.is_empty() {
            return Some(0);
        }
        if choice.is_empty() {
            return None;
        }

        // Fast path: both strings are ASCII → work on bytes directly.
        if choice.is_ascii() && pattern.is_ascii() {
            let cho = choice.as_bytes();
            let pat = pattern.as_bytes();
            let respect_case = self.respect_case_bytes(pat);

            // Early rejection
            if !self.allow_typos && !is_subsequence_bytes(pat, cho, respect_case) {
                return None;
            }
            if self.allow_typos && !cheap_typo_prefilter_bytes(pat, cho, respect_case) {
                return None;
            }

            let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
            precompute_bonuses_bytes(cho, &mut bonus_buf);

            return self
                .score_only_bytes(cho, pat, &bonus_buf, respect_case)
                .map(|(s, _end)| s as ScoreType);
        }

        // Slow path: non-ASCII → collect chars.
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

        self.score_only_chars(cho_buf, pat_buf, &bonus_buf, respect_case)
            .map(|(s, _end)| s as ScoreType)
    }

    fn fuzzy_match_range(&self, choice: &str, pattern: &str) -> Option<(ScoreType, usize, usize)> {
        if pattern.is_empty() {
            return Some((0, 0, 0));
        }
        if choice.is_empty() {
            return None;
        }

        // Fast path: both strings are ASCII → work on bytes directly.
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

            return self
                .score_only_bytes(cho, pat, &bonus_buf, respect_case)
                .map(|(s, end_col)| {
                    // end_col is 1-based column → char index = end_col - 1
                    let end = if end_col > 0 { end_col - 1 } else { 0 };
                    // Find begin: scan backward from end for pat[0]
                    let begin = find_begin_byte(cho, pat, end, respect_case);
                    (s as ScoreType, begin, end)
                });
        }

        // Slow path: non-ASCII → collect chars.
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

        self.score_only_chars(cho_buf, pat_buf, &bonus_buf, respect_case)
            .map(|(s, end_col)| {
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

        // Fast path: both strings are ASCII → work on bytes directly.
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

            return self
                .full_dp_bytes(cho, pat, &bonus_buf, respect_case)
                .map(|(s, idx)| (s as ScoreType, idx));
        }

        // Slow path: non-ASCII → collect chars.
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

        self.full_dp_chars(cho_buf, pat_buf, &bonus_buf, respect_case)
            .map(|(s, idx)| (s as ScoreType, idx))
    }

    fn batch_score_bytes(&self, items: &[&[u8]], pattern: &[u8], respect_case: bool) -> Option<Vec<Option<i32>>> {
        Some(self.batch_fuzzy_match_bytes(items, pattern, respect_case))
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

#[inline(always)]
fn eq_char(a: char, b: char, respect_case: bool) -> bool {
    if respect_case {
        a == b
    } else {
        a.eq_ignore_ascii_case(&b)
    }
}

/// Quick subsequence check (bytes).
fn is_subsequence_bytes(pattern: &[u8], choice: &[u8], respect_case: bool) -> bool {
    let mut pi = 0;
    for &c in choice {
        if pi < pattern.len() && eq_byte(pattern[pi], c, respect_case) {
            pi += 1;
        }
    }
    pi == pattern.len()
}

/// Quick subsequence check (chars).
fn is_subsequence_chars(pattern: &[char], choice: &[char], respect_case: bool) -> bool {
    let mut pi = 0;
    for &c in choice {
        if pi < pattern.len() && eq_char(pattern[pi], c, respect_case) {
            pi += 1;
        }
    }
    pi == pattern.len()
}

/// Find the earliest position in `cho[0..=end]` that matches `pat[0]`.
/// This gives a cheap lower-bound on `begin` for ranking purposes.
/// When typos are enabled the first pattern character might not literally
/// appear (it could be a mismatch/deletion), so we fall back to 0.
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

/// Find the earliest position in `cho[0..=end]` that matches `pat[0]` (char version).
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

/// Cheap pre-filter for the typos path (byte version).
///
/// Rejects items that cannot possibly score > 0 in the DP.
/// Uses a greedy subsequence scan: counts how many pattern characters
/// appear in order in the choice string.  With zero ordered matches,
/// the DP can only accumulate mismatches/deletions, yielding a negative
/// score, so we reject.  We also reject if the choice is implausibly
/// short relative to the pattern.
fn cheap_typo_prefilter_bytes(pattern: &[u8], choice: &[u8], respect_case: bool) -> bool {
    let n = pattern.len();
    // Pattern much longer than choice → impossible even with all deletions
    if n > choice.len() * 2 + 2 {
        return false;
    }

    // Greedy forward subsequence scan: count how many pattern chars
    // appear (in order) in the choice.
    let mut pi = 0;
    for &c in choice {
        if pi < n && eq_byte(pattern[pi], c, respect_case) {
            pi += 1;
        }
    }

    // With 0 matches, score = -n * MISMATCH_PENALTY which is always < 0.
    // With ≥1 match, there exists a scoring alignment that *might* be > 0
    // (depending on context bonuses), so we let it through.
    pi >= 1
}

/// Cheap pre-filter for the typos path (char version).
///
/// Same logic as the byte version, but for non-ASCII char slices.
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

/// Subtract a cost from a DP score.
///
/// Because `NEG_INF = i32::MIN / 2` and total accumulated penalties never
/// exceed ~200 000, plain subtraction cannot wrap around `i32::MIN`.
/// This is branchless and measurably faster than a guarded version.
#[inline(always)]
fn sat_sub(val: Score, cost: Score) -> Score {
    val - cost
}

/// Return the (max_value, origin) out of the three table predecessors.
#[inline(always)]
fn best_of_three(m_val: Score, d_val: Score, ins_val: Score) -> (Score, Origin) {
    let mut best = m_val;
    let mut origin = Origin::M;
    if d_val > best {
        best = d_val;
        origin = Origin::D;
    }
    if ins_val > best {
        best = ins_val;
        origin = Origin::Ins;
    }
    (best, origin)
}

// ---------------------------------------------------------------------------
// Traceback origin recomputation helpers
// ---------------------------------------------------------------------------

/// Recompute `m_origin` for cell `(i, j)` during traceback (byte path).
///
/// This replaces storing `m_origin` in the DP buffer during fill.
/// Called O(n+m) times during traceback — negligible cost.
#[inline(always)]
fn recompute_m_origin_bytes<const ALLOW_TYPOS: bool>(
    buf: &[ScoreCell],
    cho: &[u8],
    pat: &[u8],
    bonuses: &[Score],
    respect_case: bool,
    i: usize,
    j: usize,
    cols: usize,
) -> Origin {
    #[inline(always)]
    fn idx(i: usize, j: usize, cols: usize) -> usize {
        i * cols + j
    }

    // Recompute the "regular" M alignment score from the diagonal predecessor.
    let diag = idx(i - 1, j - 1, cols);
    let pred_m = buf[diag].m;
    let pred_d = if ALLOW_TYPOS { buf[diag].d } else { NEG_INF };
    let pred_ins = buf[diag].ins;
    let (best_pred, pred_origin) = best_of_three(pred_m, pred_d, pred_ins);

    let pi = pat[i - 1];
    let cj = cho[j - 1];

    let regular_align_score = if ALLOW_TYPOS {
        let mt = match_type(pi, cj, respect_case, true);
        if mt > 0 {
            let from_m = pred_origin == Origin::M;
            let bonus = context_bonus_precomputed(bonuses[j - 1], from_m, i == 1);
            let case_pen = if mt == 1 { CASE_MISMATCH_PENALTY } else { 0 };
            best_pred + MATCH_BONUS + bonus - case_pen
        } else {
            best_pred - MISMATCH_PENALTY
        }
    } else {
        // No-typos: only matches reach here via traceback.
        let mt = match_type(pi, cj, respect_case, false);
        if mt > 0 {
            let from_m = pred_origin == Origin::M;
            let bonus = context_bonus_precomputed(bonuses[j - 1], from_m, i == 1);
            let case_pen = if mt == 1 { CASE_MISMATCH_PENALTY } else { 0 };
            best_pred + MATCH_BONUS + bonus - case_pen
        } else {
            NEG_INF
        }
    };

    // Check if transposition overwrote the regular score.
    if ALLOW_TYPOS && i >= 2 && j >= 2 {
        let t1 = eq_byte(pi, cho[j - 2], respect_case);
        let t2 = eq_byte(pat[i - 2], cj, respect_case);
        if t1 && t2 {
            let diag2 = idx(i - 2, j - 2, cols);
            let ts = buf[diag2].best() - TRANSPOSITION_PENALTY;
            if ts > regular_align_score {
                return Origin::Trans;
            }
        }
    }

    pred_origin
}

/// Recompute `m_origin` for cell `(i, j)` during traceback (char path).
#[inline(always)]
fn recompute_m_origin_chars<const ALLOW_TYPOS: bool>(
    buf: &[ScoreCell],
    cho: &[char],
    pat: &[char],
    bonuses: &[Score],
    respect_case: bool,
    i: usize,
    j: usize,
    cols: usize,
) -> Origin {
    #[inline(always)]
    fn idx(i: usize, j: usize, cols: usize) -> usize {
        i * cols + j
    }

    let diag = idx(i - 1, j - 1, cols);
    let pred_m = buf[diag].m;
    let pred_d = if ALLOW_TYPOS { buf[diag].d } else { NEG_INF };
    let pred_ins = buf[diag].ins;
    let (best_pred, pred_origin) = best_of_three(pred_m, pred_d, pred_ins);

    let pi = pat[i - 1];
    let cj = cho[j - 1];

    let regular_align_score = if ALLOW_TYPOS {
        let is_match = eq_char(pi, cj, respect_case);
        if is_match {
            let from_m = pred_origin == Origin::M;
            let bonus = context_bonus_precomputed(bonuses[j - 1], from_m, i == 1);
            let case_pen = if !respect_case && pi != cj {
                CASE_MISMATCH_PENALTY
            } else {
                0
            };
            best_pred + MATCH_BONUS + bonus - case_pen
        } else {
            best_pred - MISMATCH_PENALTY
        }
    } else {
        let is_match = eq_char(pi, cj, respect_case);
        if is_match {
            let from_m = pred_origin == Origin::M;
            let bonus = context_bonus_precomputed(bonuses[j - 1], from_m, i == 1);
            let case_pen = if !respect_case && pi != cj {
                CASE_MISMATCH_PENALTY
            } else {
                0
            };
            best_pred + MATCH_BONUS + bonus - case_pen
        } else {
            NEG_INF
        }
    };

    if ALLOW_TYPOS && i >= 2 && j >= 2 && eq_char(pi, cho[j - 2], respect_case) && eq_char(pat[i - 2], cj, respect_case)
    {
        let diag2 = idx(i - 2, j - 2, cols);
        let ts = buf[diag2].best() - TRANSPOSITION_PENALTY;
        if ts > regular_align_score {
            return Origin::Trans;
        }
    }

    pred_origin
}

// ---------------------------------------------------------------------------
// Const-generic score-only DP kernels
// ---------------------------------------------------------------------------

/// Maximum pattern length for the column-major fast path.
/// For patterns up to this length, the entire DP state fits in stack arrays,
/// eliminating all buffer allocation and indexing overhead.
const COLMAJOR_MAX_N: usize = 16;

/// Score-only DP kernel for byte slices, const-generic on ALLOW_TYPOS.
///
/// When `ALLOW_TYPOS` is false, the compiler eliminates all deletion-gap,
/// mismatch, and transposition code at compile time.
/// Returns `Some((score, end_col))` where `end_col` is the 1-based column of the best
/// score in the last row. The caller can convert to a 0-based character index via `end_col - 1`.
fn score_only_bytes_inner<const ALLOW_TYPOS: bool>(
    cho: &[u8],
    pat: &[u8],
    bonuses: &[Score],
    respect_case: bool,
    score_buf_tls: &ThreadLocal<RefCell<Vec<ScoreCell>>>,
) -> Option<(Score, usize)> {
    let n = pat.len();
    if n <= COLMAJOR_MAX_N {
        return score_only_bytes_colmajor::<ALLOW_TYPOS>(cho, pat, bonuses, respect_case);
    }
    score_only_bytes_rowmajor::<ALLOW_TYPOS>(cho, pat, bonuses, respect_case, score_buf_tls)
}

/// Column-major score-only DP for byte slices.
///
/// Outer loop over choice characters (j), inner loop over pattern characters (i).
/// For small `n` (≤ COLMAJOR_MAX_N), the entire state lives in stack arrays —
/// no heap allocation, no TLS buffer, no flat-buffer indexing.
fn score_only_bytes_colmajor<const ALLOW_TYPOS: bool>(
    cho: &[u8],
    pat: &[u8],
    bonuses: &[Score],
    respect_case: bool,
) -> Option<(Score, usize)> {
    let n = pat.len();
    let m = cho.len();

    // Three columns of height (n+1): prev2, prev, cur.
    // Stack-allocated arrays sized to COLMAJOR_MAX_N+1.
    let mut col_a = [ScoreCell::INIT; COLMAJOR_MAX_N + 1];
    let mut col_b = [ScoreCell::INIT; COLMAJOR_MAX_N + 1];
    let mut col_c = [ScoreCell::INIT; COLMAJOR_MAX_N + 1];

    // Base case: column 0 (j=0).
    // Row 0: M=0, D=NEG_INF, Ins=NEG_INF (no prior column to open insertion from).
    // Rows 1..n: all NEG_INF (pattern chars not yet consumed).
    col_a[0] = ScoreCell {
        m: 0,
        d: NEG_INF,
        ins: NEG_INF,
    };
    // col_a[1..=n] already INIT (NEG_INF)

    // We rotate: prev2 <- prev <- cur after each column.
    // Start with col_a = "column j=0", col_b = scratch for j=1, col_c = scratch for j=2, etc.
    let mut prev2: &mut [ScoreCell; COLMAJOR_MAX_N + 1] = &mut col_c;
    let mut prev: &mut [ScoreCell; COLMAJOR_MAX_N + 1] = &mut col_a;
    let mut cur: &mut [ScoreCell; COLMAJOR_MAX_N + 1] = &mut col_b;

    let mut global_best: Score = NEG_INF;
    let mut global_best_j: usize = 0;

    // Track the best of the previous column's row-n cell for the final answer.
    // Row 0 at j=0: best = M=0 (semi-global: can start anywhere).
    // Row n at j=0: best = NEG_INF (no pattern chars matched yet).
    // We track best across all columns' row-n cells.
    {
        let s = prev[n].best();
        if s > global_best {
            global_best = s;
            global_best_j = 0;
        }
    }

    for j in 1..=m {
        let cj = cho[j - 1];
        let bonus_j = bonuses[j - 1];

        // Row 0 of current column: semi-global start — M=0, Ins from prev col.
        // Insertion gap in row 0: we can skip choice chars freely.
        // prev[0].m = 0 always, so ins_from_m = 0 - INS_OPEN = -8
        // prev[0].ins was NEG_INF for j=0, then propagates.
        let ins_open_r0 = sat_sub(prev[0].m, INS_OPEN);
        let ins_ext_r0 = sat_sub(prev[0].ins, INS_EXTEND);
        cur[0] = ScoreCell {
            m: 0,
            d: NEG_INF,
            ins: ins_open_r0.max(ins_ext_r0),
        };

        for i in 1..=n {
            let pi = pat[i - 1];

            // --- D: deletion gap (skip pattern char) ---
            let cur_d = if ALLOW_TYPOS {
                let d_open = sat_sub(cur[i - 1].m, DEL_OPEN);
                let d_ext = sat_sub(cur[i - 1].d, DEL_EXTEND);
                d_open.max(d_ext)
            } else {
                NEG_INF
            };

            // --- Insertion gap (skip choice char) ---
            // Depends on same row (i), previous column (j-1) = prev[i]
            let ins_from_m = sat_sub(prev[i].m, INS_OPEN);
            let ins_ext = sat_sub(prev[i].ins, INS_EXTEND);
            let cur_ins = ins_from_m.max(ins_ext);

            // --- M: align pi with cj ---
            // Diagonal = prev[i-1] (previous column, previous row)
            let diag = &prev[i - 1];
            let pred_m = diag.m;
            let pred_d = if ALLOW_TYPOS { diag.d } else { NEG_INF };
            let pred_ins = diag.ins;
            let best_pred = pred_m.max(pred_d).max(pred_ins);

            let mut cur_m = if ALLOW_TYPOS {
                // With typos, match_type always returns +2, +1, or -1 (never 0).
                // We skip the `best_pred > NEG_INF` guard: when best_pred is
                // extremely negative the result is still extremely negative and
                // gets beaten by any legitimate path later.
                let mt = match_type(pi, cj, respect_case, true);
                if mt > 0 {
                    let from_m = pred_m >= pred_d && pred_m >= pred_ins;
                    let bonus = context_bonus_precomputed(bonus_j, from_m, i == 1);
                    let case_pen = if mt == 1 { CASE_MISMATCH_PENALTY } else { 0 };
                    best_pred + MATCH_BONUS + bonus - case_pen
                } else {
                    // mt == -1 (mismatch)
                    best_pred - MISMATCH_PENALTY
                }
            } else if best_pred > NEG_INF {
                let mt = match_type(pi, cj, respect_case, false);
                if mt > 0 {
                    let from_m = pred_m >= pred_ins;
                    let bonus = context_bonus_precomputed(bonus_j, from_m, i == 1);
                    let case_pen = if mt == 1 { CASE_MISMATCH_PENALTY } else { 0 };
                    best_pred + MATCH_BONUS + bonus - case_pen
                } else {
                    NEG_INF
                }
            } else {
                NEG_INF
            };

            // --- Transposition (typos only) ---
            if ALLOW_TYPOS && i >= 2 && j >= 2 {
                let t1 = eq_byte(pi, cho[j - 2], respect_case);
                let t2 = eq_byte(pat[i - 2], cj, respect_case);
                if t1 && t2 {
                    let ts = prev2[i - 2].best() - TRANSPOSITION_PENALTY;
                    cur_m = cur_m.max(ts);
                }
            }

            cur[i] = ScoreCell {
                m: cur_m,
                d: cur_d,
                ins: cur_ins,
            };
        }

        // Track best score at row n (all pattern chars consumed)
        {
            let s = cur[n].best();
            if s > global_best {
                global_best = s;
                global_best_j = j;
            }
        }

        // Rotate columns: prev2 <- prev <- cur, reuse prev2 as next cur
        let tmp = prev2;
        prev2 = prev;
        prev = cur;
        cur = tmp;
    }

    if global_best > 0 {
        Some((global_best, global_best_j))
    } else {
        None
    }
}

/// Row-major score-only DP fallback for longer patterns (n > COLMAJOR_MAX_N).
fn score_only_bytes_rowmajor<const ALLOW_TYPOS: bool>(
    cho: &[u8],
    pat: &[u8],
    bonuses: &[Score],
    respect_case: bool,
    score_buf_tls: &ThreadLocal<RefCell<Vec<ScoreCell>>>,
) -> Option<(Score, usize)> {
    let n = pat.len();
    let m = cho.len();
    let cols = m + 1;

    let mut buf = score_buf_tls.get_or(|| RefCell::new(Vec::new())).borrow_mut();
    let needed = cols * 3;
    if buf.len() < needed {
        buf.resize(needed, ScoreCell::INIT);
    }

    #[inline(always)]
    fn row_off(logical: usize, cols: usize) -> usize {
        (logical % 3) * cols
    }

    // Base case: row 0
    {
        let off = row_off(0, cols);
        buf[off] = ScoreCell {
            m: 0,
            d: NEG_INF,
            ins: NEG_INF,
        };
        for j in 1..cols {
            buf[off + j] = ScoreCell {
                m: 0,
                d: NEG_INF,
                ins: 0,
            };
        }
    }

    for i in 1..=n {
        let cur = row_off(i, cols);
        let prev = row_off(i - 1, cols);
        let prev2 = if ALLOW_TYPOS {
            row_off(i.wrapping_sub(2), cols)
        } else {
            0
        };

        // Reset current row
        for j in 0..cols {
            buf[cur + j] = ScoreCell::INIT;
        }

        let is_first = i == 1;
        let pi = pat[i - 1];

        let mut prev_cell = buf[prev];
        let mut cur_prev_m = NEG_INF;
        let mut cur_prev_ins = NEG_INF;

        for j in 1..=m {
            let cj = cho[j - 1];

            let prev_above = buf[prev + j];
            let cur_d = if ALLOW_TYPOS {
                let d_open = sat_sub(prev_above.m, DEL_OPEN);
                let d_ext = sat_sub(prev_above.d, DEL_EXTEND);
                d_open.max(d_ext)
            } else {
                NEG_INF
            };

            let ins_from_m = sat_sub(cur_prev_m, INS_OPEN);
            let ins_ext = sat_sub(cur_prev_ins, INS_EXTEND);
            let cur_ins = ins_from_m.max(ins_ext);

            let pred_m = prev_cell.m;
            let pred_d = if ALLOW_TYPOS { prev_cell.d } else { NEG_INF };
            let pred_ins = prev_cell.ins;
            let best_pred = pred_m.max(pred_d).max(pred_ins);

            let cur_m = if ALLOW_TYPOS {
                let mt = match_type(pi, cj, respect_case, true);
                if mt > 0 {
                    let from_m = pred_m >= pred_d && pred_m >= pred_ins;
                    let bonus = context_bonus_precomputed(bonuses[j - 1], from_m, is_first);
                    let case_pen = if mt == 1 { CASE_MISMATCH_PENALTY } else { 0 };
                    best_pred + MATCH_BONUS + bonus - case_pen
                } else {
                    best_pred - MISMATCH_PENALTY
                }
            } else if best_pred > NEG_INF {
                let mt = match_type(pi, cj, respect_case, false);
                if mt > 0 {
                    let from_m = pred_m >= pred_ins;
                    let bonus = context_bonus_precomputed(bonuses[j - 1], from_m, is_first);
                    let case_pen = if mt == 1 { CASE_MISMATCH_PENALTY } else { 0 };
                    best_pred + MATCH_BONUS + bonus - case_pen
                } else {
                    NEG_INF
                }
            } else {
                NEG_INF
            };

            let cur_m = if ALLOW_TYPOS && i >= 2 && j >= 2 {
                let t1 = eq_byte(pi, cho[j - 2], respect_case);
                let t2 = eq_byte(pat[i - 2], cj, respect_case);
                if t1 && t2 {
                    let ts = buf[prev2 + j - 2].best() - TRANSPOSITION_PENALTY;
                    cur_m.max(ts)
                } else {
                    cur_m
                }
            } else {
                cur_m
            };

            buf[cur + j].m = cur_m;
            buf[cur + j].d = cur_d;
            buf[cur + j].ins = cur_ins;

            prev_cell = prev_above;
            cur_prev_m = cur_m;
            cur_prev_ins = cur_ins;
        }
    }

    let last = row_off(n, cols);
    let mut best = NEG_INF;
    let mut best_j = 0usize;
    for j in 0..cols {
        let s = buf[last + j].best();
        if s > best {
            best = s;
            best_j = j;
        }
    }

    if best > 0 { Some((best, best_j)) } else { None }
}

/// Score-only DP kernel for char slices, const-generic on ALLOW_TYPOS.
fn score_only_chars_inner<const ALLOW_TYPOS: bool>(
    cho: &[char],
    pat: &[char],
    bonuses: &[Score],
    respect_case: bool,
    score_buf_tls: &ThreadLocal<RefCell<Vec<ScoreCell>>>,
) -> Option<(Score, usize)> {
    let n = pat.len();
    if n <= COLMAJOR_MAX_N {
        return score_only_chars_colmajor::<ALLOW_TYPOS>(cho, pat, bonuses, respect_case);
    }
    score_only_chars_rowmajor::<ALLOW_TYPOS>(cho, pat, bonuses, respect_case, score_buf_tls)
}

/// Column-major score-only DP for char slices.
fn score_only_chars_colmajor<const ALLOW_TYPOS: bool>(
    cho: &[char],
    pat: &[char],
    bonuses: &[Score],
    respect_case: bool,
) -> Option<(Score, usize)> {
    let n = pat.len();
    let m = cho.len();

    let mut col_a = [ScoreCell::INIT; COLMAJOR_MAX_N + 1];
    let mut col_b = [ScoreCell::INIT; COLMAJOR_MAX_N + 1];
    let mut col_c = [ScoreCell::INIT; COLMAJOR_MAX_N + 1];

    col_a[0] = ScoreCell {
        m: 0,
        d: NEG_INF,
        ins: NEG_INF,
    };

    let mut prev2: &mut [ScoreCell; COLMAJOR_MAX_N + 1] = &mut col_c;
    let mut prev: &mut [ScoreCell; COLMAJOR_MAX_N + 1] = &mut col_a;
    let mut cur: &mut [ScoreCell; COLMAJOR_MAX_N + 1] = &mut col_b;

    let mut global_best: Score = NEG_INF;
    let mut global_best_j: usize = 0;
    {
        let s = prev[n].best();
        if s > global_best {
            global_best = s;
            global_best_j = 0;
        }
    }

    for j in 1..=m {
        let cj = cho[j - 1];
        let bonus_j = bonuses[j - 1];

        let ins_open_r0 = sat_sub(prev[0].m, INS_OPEN);
        let ins_ext_r0 = sat_sub(prev[0].ins, INS_EXTEND);
        cur[0] = ScoreCell {
            m: 0,
            d: NEG_INF,
            ins: ins_open_r0.max(ins_ext_r0),
        };

        for i in 1..=n {
            let pi = pat[i - 1];

            let cur_d = if ALLOW_TYPOS {
                let d_open = sat_sub(cur[i - 1].m, DEL_OPEN);
                let d_ext = sat_sub(cur[i - 1].d, DEL_EXTEND);
                d_open.max(d_ext)
            } else {
                NEG_INF
            };

            let ins_from_m = sat_sub(prev[i].m, INS_OPEN);
            let ins_ext = sat_sub(prev[i].ins, INS_EXTEND);
            let cur_ins = ins_from_m.max(ins_ext);

            let diag = &prev[i - 1];
            let pred_m = diag.m;
            let pred_d = if ALLOW_TYPOS { diag.d } else { NEG_INF };
            let pred_ins = diag.ins;
            let best_pred = pred_m.max(pred_d).max(pred_ins);

            let mut cur_m = if ALLOW_TYPOS {
                let is_match = eq_char(pi, cj, respect_case);
                if is_match {
                    let from_m = pred_m >= pred_d && pred_m >= pred_ins;
                    let bonus = context_bonus_precomputed(bonus_j, from_m, i == 1);
                    let case_pen = if !respect_case && pi != cj {
                        CASE_MISMATCH_PENALTY
                    } else {
                        0
                    };
                    best_pred + MATCH_BONUS + bonus - case_pen
                } else {
                    best_pred - MISMATCH_PENALTY
                }
            } else if best_pred > NEG_INF {
                let is_match = eq_char(pi, cj, respect_case);
                if is_match {
                    let from_m = pred_m >= pred_ins;
                    let bonus = context_bonus_precomputed(bonus_j, from_m, i == 1);
                    let case_pen = if !respect_case && pi != cj {
                        CASE_MISMATCH_PENALTY
                    } else {
                        0
                    };
                    best_pred + MATCH_BONUS + bonus - case_pen
                } else {
                    NEG_INF
                }
            } else {
                NEG_INF
            };

            if ALLOW_TYPOS && i >= 2 && j >= 2 {
                let t1 = eq_char(pi, cho[j - 2], respect_case);
                let t2 = eq_char(pat[i - 2], cj, respect_case);
                if t1 && t2 {
                    let ts = prev2[i - 2].best() - TRANSPOSITION_PENALTY;
                    cur_m = cur_m.max(ts);
                }
            }

            cur[i] = ScoreCell {
                m: cur_m,
                d: cur_d,
                ins: cur_ins,
            };
        }

        {
            let s = cur[n].best();
            if s > global_best {
                global_best = s;
                global_best_j = j;
            }
        }

        let tmp = prev2;
        prev2 = prev;
        prev = cur;
        cur = tmp;
    }

    if global_best > 0 {
        Some((global_best, global_best_j))
    } else {
        None
    }
}

/// Row-major score-only DP fallback for char slices (n > COLMAJOR_MAX_N).
fn score_only_chars_rowmajor<const ALLOW_TYPOS: bool>(
    cho: &[char],
    pat: &[char],
    bonuses: &[Score],
    respect_case: bool,
    score_buf_tls: &ThreadLocal<RefCell<Vec<ScoreCell>>>,
) -> Option<(Score, usize)> {
    let n = pat.len();
    let m = cho.len();
    let cols = m + 1;

    let mut buf = score_buf_tls.get_or(|| RefCell::new(Vec::new())).borrow_mut();
    let needed = cols * 3;
    if buf.len() < needed {
        buf.resize(needed, ScoreCell::INIT);
    }

    #[inline(always)]
    fn row_off(logical: usize, cols: usize) -> usize {
        (logical % 3) * cols
    }

    {
        let off = row_off(0, cols);
        buf[off] = ScoreCell {
            m: 0,
            d: NEG_INF,
            ins: NEG_INF,
        };
        for j in 1..cols {
            buf[off + j] = ScoreCell {
                m: 0,
                d: NEG_INF,
                ins: 0,
            };
        }
    }

    for i in 1..=n {
        let cur = row_off(i, cols);
        let prev = row_off(i - 1, cols);
        let prev2 = if ALLOW_TYPOS {
            row_off(i.wrapping_sub(2), cols)
        } else {
            0
        };

        for j in 0..cols {
            buf[cur + j] = ScoreCell::INIT;
        }

        let is_first = i == 1;
        let pi = pat[i - 1];

        let mut prev_cell = buf[prev];
        let mut cur_prev_m = NEG_INF;
        let mut cur_prev_ins = NEG_INF;

        for j in 1..=m {
            let cj = cho[j - 1];

            let prev_above = buf[prev + j];
            let cur_d = if ALLOW_TYPOS {
                let d_open = sat_sub(prev_above.m, DEL_OPEN);
                let d_ext = sat_sub(prev_above.d, DEL_EXTEND);
                d_open.max(d_ext)
            } else {
                NEG_INF
            };

            let ins_from_m = sat_sub(cur_prev_m, INS_OPEN);
            let ins_ext = sat_sub(cur_prev_ins, INS_EXTEND);
            let cur_ins = ins_from_m.max(ins_ext);

            let pred_m = prev_cell.m;
            let pred_d = if ALLOW_TYPOS { prev_cell.d } else { NEG_INF };
            let pred_ins = prev_cell.ins;
            let best_pred = pred_m.max(pred_d).max(pred_ins);

            let cur_m = if ALLOW_TYPOS {
                let is_match = eq_char(pi, cj, respect_case);
                if is_match {
                    let from_m = pred_m >= pred_d && pred_m >= pred_ins;
                    let bonus = context_bonus_precomputed(bonuses[j - 1], from_m, is_first);
                    let case_pen = if !respect_case && pi != cj {
                        CASE_MISMATCH_PENALTY
                    } else {
                        0
                    };
                    best_pred + MATCH_BONUS + bonus - case_pen
                } else {
                    best_pred - MISMATCH_PENALTY
                }
            } else if best_pred > NEG_INF {
                let is_match = eq_char(pi, cj, respect_case);
                if is_match {
                    let from_m = pred_m >= pred_ins;
                    let bonus = context_bonus_precomputed(bonuses[j - 1], from_m, is_first);
                    let case_pen = if !respect_case && pi != cj {
                        CASE_MISMATCH_PENALTY
                    } else {
                        0
                    };
                    best_pred + MATCH_BONUS + bonus - case_pen
                } else {
                    NEG_INF
                }
            } else {
                NEG_INF
            };

            let cur_m = if ALLOW_TYPOS && i >= 2 && j >= 2 {
                let t1 = eq_char(pi, cho[j - 2], respect_case);
                let t2 = eq_char(pat[i - 2], cj, respect_case);
                if t1 && t2 {
                    let ts = buf[prev2 + j - 2].best() - TRANSPOSITION_PENALTY;
                    cur_m.max(ts)
                } else {
                    cur_m
                }
            } else {
                cur_m
            };

            buf[cur + j].m = cur_m;
            buf[cur + j].d = cur_d;
            buf[cur + j].ins = cur_ins;

            prev_cell = prev_above;
            cur_prev_m = cur_m;
            cur_prev_ins = cur_ins;
        }
    }

    let last = row_off(n, cols);
    let mut best = NEG_INF;
    let mut best_j = 0usize;
    for j in 0..cols {
        let s = buf[last + j].best();
        if s > best {
            best = s;
            best_j = j;
        }
    }

    if best > 0 { Some((best, best_j)) } else { None }
}

// ---------------------------------------------------------------------------
// SIMD batched score-only DP (8 items at once using wide::i32x8)
// ---------------------------------------------------------------------------

/// Number of SIMD lanes (AVX2 = 8 × i32).
const SIMD_LANES: usize = 8;

/// Maximum choice string length for the batched SIMD path.
/// Strings longer than this fall back to scalar.
const SIMD_MAX_CHOICE_LEN: usize = 128;

use wide::{CmpEq, CmpGt, i32x8};

/// SIMD-parallel score-only DP: processes up to 8 ASCII choice strings
/// simultaneously against the same pattern.
///
/// Each lane of the `i32x8` vectors holds the DP state for a different
/// choice string. The outer loop iterates over choice positions (columns),
/// the inner loop over pattern positions (rows).
///
/// `batch_count` is the number of valid items in the batch (1..=8).
/// Items at indices `>= batch_count` are ignored.
///
/// Returns an array of 8 `Option<Score>` values (one per lane).
///
/// # Arguments
/// - `choices`: up to 8 byte slices (choice strings). Unused lanes should
///   point to an empty slice.
/// - `choice_lens`: lengths of each choice string.
/// - `bonuses`: up to 8 precomputed bonus arrays. Unused lanes should point
///   to an empty slice.
/// - `pat`: the pattern bytes.
/// - `respect_case`: whether matching is case-sensitive.
/// - `batch_count`: number of valid items (1..=8).
fn batch_score_only_bytes_simd<const ALLOW_TYPOS: bool>(
    choices: &[&[u8]; SIMD_LANES],
    choice_lens: &[usize; SIMD_LANES],
    bonuses: &[&[Score]; SIMD_LANES],
    pat: &[u8],
    respect_case: bool,
    batch_count: usize,
) -> [Option<Score>; SIMD_LANES] {
    let n = pat.len();
    debug_assert!(n <= COLMAJOR_MAX_N, "pattern too long for SIMD batch path");
    debug_assert!(batch_count > 0 && batch_count <= SIMD_LANES);

    let neg_inf_v = i32x8::splat(NEG_INF);
    let zero_v = i32x8::splat(0);
    let match_bonus_v = i32x8::splat(MATCH_BONUS);
    let mismatch_penalty_v = i32x8::splat(MISMATCH_PENALTY);
    let case_mismatch_penalty_v = i32x8::splat(CASE_MISMATCH_PENALTY);
    let ins_open_v = i32x8::splat(INS_OPEN);
    let ins_extend_v = i32x8::splat(INS_EXTEND);
    let del_open_v = i32x8::splat(DEL_OPEN);
    let del_extend_v = i32x8::splat(DEL_EXTEND);
    let trans_penalty_v = i32x8::splat(TRANSPOSITION_PENALTY);
    let consec_bonus_v = i32x8::splat(CONSECUTIVE_BONUS);

    // Max choice length across the batch
    let max_m = *choice_lens.iter().take(batch_count).max().unwrap_or(&0);

    // DP state: 3 columns (prev2, prev, cur) × (n+1) rows, each row has
    // m, d, ins as i32x8.
    // We use stack arrays sized to COLMAJOR_MAX_N+1.
    // Column state: m, d, ins per row
    struct ColState {
        m: [i32x8; COLMAJOR_MAX_N + 1],
        d: [i32x8; COLMAJOR_MAX_N + 1],
        ins: [i32x8; COLMAJOR_MAX_N + 1],
    }

    impl ColState {
        fn new() -> Self {
            Self {
                m: [i32x8::splat(NEG_INF); COLMAJOR_MAX_N + 1],
                d: [i32x8::splat(NEG_INF); COLMAJOR_MAX_N + 1],
                ins: [i32x8::splat(NEG_INF); COLMAJOR_MAX_N + 1],
            }
        }
    }

    let mut col_a = ColState::new();
    let mut col_b = ColState::new();
    let mut col_c = ColState::new();

    // Base case: column 0 (j=0)
    // Row 0: m=0, d=NEG_INF, ins=NEG_INF
    col_a.m[0] = zero_v;
    // Rows 1..=n stay NEG_INF

    let mut prev2 = &mut col_c;
    let mut prev = &mut col_a;
    let mut cur = &mut col_b;

    let mut global_best = neg_inf_v;

    // Track best at row n from column 0
    global_best = global_best.max(prev.m[n].max(prev.d[n]).max(prev.ins[n]));

    // Build lane-validity mask: lanes >= batch_count or with zero-length
    // choices are "inactive". We'll mask their contributions.
    let _lane_active = {
        let mut arr = [0i32; SIMD_LANES];
        for lane in 0..batch_count {
            if choice_lens[lane] > 0 {
                arr[lane] = -1; // all bits set = true mask
            }
        }
        i32x8::new(arr)
    };

    for j in 1..=max_m {
        // Build per-lane mask: is this lane still within its choice string?
        let j_valid = {
            let mut arr = [0i32; SIMD_LANES];
            for lane in 0..batch_count {
                if j <= choice_lens[lane] {
                    arr[lane] = -1;
                }
            }
            i32x8::new(arr)
        };

        // Gather choice characters at position j-1 for each lane
        let cj_bytes = {
            let mut arr = [0i32; SIMD_LANES];
            for lane in 0..batch_count {
                if j <= choice_lens[lane] {
                    arr[lane] = choices[lane][j - 1] as i32;
                }
            }
            i32x8::new(arr)
        };

        // Gather bonuses at position j-1
        let bonus_j = {
            let mut arr = [0i32; SIMD_LANES];
            for lane in 0..batch_count {
                if j <= choice_lens[lane] {
                    arr[lane] = bonuses[lane][j - 1];
                }
            }
            i32x8::new(arr)
        };

        // Row 0 of current column
        let ins_open_r0 = prev.m[0] - ins_open_v;
        let ins_ext_r0 = prev.ins[0] - ins_extend_v;
        cur.m[0] = zero_v;
        cur.d[0] = neg_inf_v;
        cur.ins[0] = ins_open_r0.max(ins_ext_r0);

        // Gather previous column's cho[j-2] for transposition check
        let prev_cj_bytes = if ALLOW_TYPOS && j >= 2 {
            let mut arr = [0i32; SIMD_LANES];
            for lane in 0..batch_count {
                if j - 1 <= choice_lens[lane] {
                    arr[lane] = choices[lane][j - 2] as i32;
                }
            }
            i32x8::new(arr)
        } else {
            zero_v
        };

        for i in 1..=n {
            let pi = pat[i - 1];
            let pi_v = i32x8::splat(pi as i32);

            // --- D: deletion gap (skip pattern char) ---
            let cur_d = if ALLOW_TYPOS {
                let d_open = cur.m[i - 1] - del_open_v;
                let d_ext = cur.d[i - 1] - del_extend_v;
                d_open.max(d_ext)
            } else {
                neg_inf_v
            };

            // --- Insertion gap (skip choice char) ---
            let ins_from_m = prev.m[i] - ins_open_v;
            let ins_ext = prev.ins[i] - ins_extend_v;
            let cur_ins = ins_from_m.max(ins_ext);

            // --- M: align pi with cj ---
            let diag_m = prev.m[i - 1];
            let diag_d = if ALLOW_TYPOS { prev.d[i - 1] } else { neg_inf_v };
            let diag_ins = prev.ins[i - 1];
            let best_pred = diag_m.max(diag_d).max(diag_ins);

            let cur_m = if ALLOW_TYPOS {
                // Check match: compare pi with cj per lane
                let is_exact_match = if respect_case {
                    pi_v.simd_eq(cj_bytes)
                } else {
                    // Case-insensitive: compare lowercased
                    let pi_lower = i32x8::splat((pi | 0x20) as i32);
                    let cj_lower = cj_bytes | i32x8::splat(0x20);
                    // But only apply lowering for ASCII letters
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

                // Case mismatch: matched but cases differ
                let cases_differ = if !respect_case {
                    is_exact_match & !pi_v.simd_eq(cj_bytes)
                } else {
                    zero_v // no case mismatch possible in respect_case mode
                };

                // Bonus computation
                let from_m = {
                    let m_ge_d = diag_m.simd_gt(diag_d) | diag_m.simd_eq(diag_d);
                    let m_ge_ins = diag_m.simd_gt(diag_ins) | diag_m.simd_eq(diag_ins);
                    m_ge_d & m_ge_ins
                };

                let mut bonus = bonus_j;
                // Add consecutive bonus where from_m is true
                bonus += from_m & consec_bonus_v;
                // Multiply by 2 for first pattern char
                if i == 1 {
                    bonus = bonus + bonus; // bonus *= 2
                }

                let match_score = best_pred + match_bonus_v + bonus - (cases_differ & case_mismatch_penalty_v);
                let mismatch_score = best_pred - mismatch_penalty_v;

                // Select: if matched → match_score, else → mismatch_score
                is_exact_match.blend(match_score, mismatch_score)
            } else {
                // No typos: only score matches
                let is_match = if respect_case {
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
                    is_match & !pi_v.simd_eq(cj_bytes)
                } else {
                    zero_v
                };

                // Guard: only compute when best_pred > NEG_INF
                let pred_valid = best_pred.simd_gt(neg_inf_v);
                let valid_match = is_match & pred_valid;

                let from_m = { diag_m.simd_gt(diag_ins) | diag_m.simd_eq(diag_ins) };

                let mut bonus = bonus_j;
                bonus += from_m & consec_bonus_v;
                if i == 1 {
                    bonus = bonus + bonus;
                }

                let match_score = best_pred + match_bonus_v + bonus - (cases_differ & case_mismatch_penalty_v);

                // Select: if valid_match → match_score, else → NEG_INF
                valid_match.blend(match_score, neg_inf_v)
            };

            // --- Transposition (typos only) ---
            let cur_m = if ALLOW_TYPOS && i >= 2 && j >= 2 {
                let pi_prev = pat[i - 2];
                let pi_v_prev = i32x8::splat(pi_prev as i32);

                // Check: pat[i-1] == cho[j-2] and pat[i-2] == cho[j-1]
                let t1 = if respect_case {
                    pi_v.simd_eq(prev_cj_bytes)
                } else {
                    // Case-insensitive comparison for pi vs cho[j-2]
                    let a_lower = i32x8::splat((pi | 0x20) as i32);
                    let b_lower = prev_cj_bytes | i32x8::splat(0x20);
                    let a_alpha = if pi.is_ascii_alphabetic() {
                        i32x8::splat(-1)
                    } else {
                        zero_v
                    };
                    let b_alpha = {
                        let ge_a = prev_cj_bytes.simd_gt(i32x8::splat(b'A' as i32 - 1));
                        let le_z = (i32x8::splat(b'z' as i32 + 1)).simd_gt(prev_cj_bytes);
                        let ge_la = prev_cj_bytes.simd_gt(i32x8::splat(b'a' as i32 - 1));
                        let upper = ge_a & (i32x8::splat(b'Z' as i32 + 1)).simd_gt(prev_cj_bytes);
                        upper | ge_la & le_z
                    };
                    let ci = (a_lower.simd_eq(b_lower)) & a_alpha & b_alpha;
                    ci | pi_v.simd_eq(prev_cj_bytes)
                };

                let t2 = if respect_case {
                    pi_v_prev.simd_eq(cj_bytes)
                } else {
                    let a_lower = i32x8::splat((pi_prev | 0x20) as i32);
                    let b_lower = cj_bytes | i32x8::splat(0x20);
                    let a_alpha = if pi_prev.is_ascii_alphabetic() {
                        i32x8::splat(-1)
                    } else {
                        zero_v
                    };
                    let b_alpha = {
                        let ge_a = cj_bytes.simd_gt(i32x8::splat(b'A' as i32 - 1));
                        let le_z = (i32x8::splat(b'z' as i32 + 1)).simd_gt(cj_bytes);
                        let ge_la = cj_bytes.simd_gt(i32x8::splat(b'a' as i32 - 1));
                        let upper = ge_a & (i32x8::splat(b'Z' as i32 + 1)).simd_gt(cj_bytes);
                        upper | ge_la & le_z
                    };
                    let ci = (a_lower.simd_eq(b_lower)) & a_alpha & b_alpha;
                    ci | pi_v_prev.simd_eq(cj_bytes)
                };

                let trans_valid = t1 & t2;

                // prev2 column, row i-2: best of m, d, ins
                let p2_best = prev2.m[i - 2].max(prev2.d[i - 2]).max(prev2.ins[i - 2]);
                let ts = p2_best - trans_penalty_v;
                let better = ts.simd_gt(cur_m);
                let use_trans = trans_valid & better;
                use_trans.blend(ts, cur_m)
            } else {
                cur_m
            };

            // Mask: for lanes past their choice length, keep NEG_INF
            cur.m[i] = j_valid.blend(cur_m, neg_inf_v);
            cur.d[i] = j_valid.blend(cur_d, neg_inf_v);
            cur.ins[i] = j_valid.blend(cur_ins, neg_inf_v);
        }

        // Track best score at row n
        let row_n_best = cur.m[n].max(cur.d[n]).max(cur.ins[n]);
        global_best = global_best.max(row_n_best);

        // Rotate columns
        let tmp = prev2;
        prev2 = prev;
        prev = cur;
        cur = tmp;
    }

    // Extract results
    let scores = global_best.to_array();
    let mut result = [None; SIMD_LANES];
    for lane in 0..batch_count {
        if scores[lane] > 0 {
            result[lane] = Some(scores[lane]);
        }
    }
    result
}

/// Public batch scoring API for the microbenchmark and internal use.
///
/// Runs prefilter + SIMD-batched score-only DP on up to 8 ASCII choice strings.
/// Non-ASCII strings or strings longer than `SIMD_MAX_CHOICE_LEN` fall back to scalar.
///
/// Returns an array of 8 `Option<Score>`.
impl SkimV3Matcher {
    /// Batch score-only fuzzy matching for multiple byte-string items using SIMD.
    ///
    /// Processes items in groups of 8 using `i32x8` SIMD lanes. Items that fail
    /// prefiltering, are non-ASCII, or exceed `SIMD_MAX_CHOICE_LEN` fall back to scalar.
    pub fn batch_fuzzy_match_bytes(&self, items: &[&[u8]], pattern: &[u8], respect_case: bool) -> Vec<Option<Score>> {
        let n = pattern.len();
        if n == 0 {
            return items.iter().map(|_| Some(0)).collect();
        }
        if n > COLMAJOR_MAX_N {
            // Fall back to scalar for long patterns
            return items
                .iter()
                .map(|cho| {
                    let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
                    precompute_bonuses_bytes(cho, &mut bonus_buf);
                    self.score_only_bytes(cho, pattern, &bonus_buf, respect_case)
                        .map(|(s, _)| s)
                })
                .collect();
        }

        let mut results: Vec<Option<Score>> = Vec::with_capacity(items.len());
        let empty: &[u8] = &[];
        let empty_bonus: &[Score] = &[];

        // Process in batches of 8
        let mut bonus_storage: Vec<Vec<Score>> = Vec::with_capacity(SIMD_LANES);
        for _ in 0..SIMD_LANES {
            bonus_storage.push(Vec::new());
        }

        let mut idx = 0;
        while idx < items.len() {
            let batch_count = (items.len() - idx).min(SIMD_LANES);

            let mut choices: [&[u8]; SIMD_LANES] = [empty; SIMD_LANES];
            let mut choice_lens: [usize; SIMD_LANES] = [0; SIMD_LANES];
            // Track which lanes should use scalar fallback
            let mut scalar_fallback: [bool; SIMD_LANES] = [false; SIMD_LANES];

            // Prepare batch: run prefilter and compute bonuses
            for lane in 0..batch_count {
                let cho = items[idx + lane];

                // Prefilter
                let pass = if self.allow_typos {
                    cheap_typo_prefilter_bytes(pattern, cho, respect_case)
                } else {
                    is_subsequence_bytes(pattern, cho, respect_case)
                };

                if !pass {
                    // This lane will produce None
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

            // Build bonus refs after the mutable borrow loop is done
            let mut bonus_refs: [&[Score]; SIMD_LANES] = [empty_bonus; SIMD_LANES];
            for lane in 0..batch_count {
                if !scalar_fallback[lane] {
                    bonus_refs[lane] = &bonus_storage[lane];
                }
            }

            // Count how many lanes are active for SIMD
            let active_count = (0..batch_count).filter(|&l| !scalar_fallback[l]).count();

            if active_count == 0 {
                // All filtered out or scalar fallback
                for lane in 0..batch_count {
                    if scalar_fallback[lane] && items[idx + lane].len() > SIMD_MAX_CHOICE_LEN {
                        // Scalar fallback for long strings that passed prefilter
                        let cho = items[idx + lane];
                        let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
                        precompute_bonuses_bytes(cho, &mut bonus_buf);
                        results.push(
                            self.score_only_bytes(cho, pattern, &bonus_buf, respect_case)
                                .map(|(s, _)| s),
                        );
                    } else {
                        results.push(None);
                    }
                }
            } else {
                // Run SIMD batch
                let batch_results = if self.allow_typos {
                    batch_score_only_bytes_simd::<true>(
                        &choices,
                        &choice_lens,
                        &bonus_refs,
                        pattern,
                        respect_case,
                        SIMD_LANES, // always pass all 8 lanes (inactive ones have len=0)
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
                        results.push(
                            self.score_only_bytes(cho, pattern, &bonus_buf, respect_case)
                                .map(|(s, _)| s),
                        );
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
        // Ensure non-ASCII falls back to char path and still works
        let m = matcher();
        assert!(m.fuzzy_match("café", "café").is_some());
        assert!(m.fuzzy_match("naïve", "naive").is_none()); // ï ≠ i (respect case default smart)
    }

    // ----- SIMD batch matching -----

    /// Convert FuzzyMatcher's Option<i64> to Option<i32> for comparison with batch results
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
            b"tset", // transposition
            b"tast", // substitution
            b"testing",
            b"the_test_file",
            b"xxxx", // no match
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
            b"extra1.rs", // 9th item - second batch
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
        let pattern = b"";
        let items: Vec<&[u8]> = vec![b"foo", b"bar"];

        let batch_results = m.batch_fuzzy_match_bytes(&items, pattern, false);
        assert_eq!(batch_results.len(), 2);
        // Empty pattern should match everything with score 0
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
            b"t",                                                                      // too short
            b"te",                                                                     // too short
            b"tes",                                          // too short for no-typo, maybe typo
            b"test",                                         // exact
            b"this_is_a_fairly_long_path/to/some/test/file", // medium
            b"a/very/deeply/nested/directory/structure/with/test/somewhere/in/it.txt", // long
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
        // Test with a variety of patterns and generated items to catch subtle SIMD bugs
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
