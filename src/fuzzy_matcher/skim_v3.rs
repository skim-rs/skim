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
//!
//!
//! ## Pruning strategies
//!
//! - **Row-range banding**: each DP cell is only computed when the row/column
//!   pair falls within the feasible alignment band. In exact mode the band is
//!   derived from precomputed first/last match columns for each pattern
//!   character; in typo mode a diagonal ± bandwidth envelope is used.
//! - **Interpair max-score pruning**: after processing a column (score-only)
//!   or row (full DP), if all cells are zero for several consecutive
//!   iterations, the alignment is dead and we terminate early.

use std::cell::RefCell;

use thread_local::ThreadLocal;

use crate::{
    CaseMatching,
    fuzzy_matcher::{FuzzyMatcher, IndexType, MatchIndices, ScoreType},
};

// ---------------------------------------------------------------------------
// Scoring constants
// ---------------------------------------------------------------------------

type Score = i16;

/// Points awarded for each correctly matched character.
const MATCH_BONUS: Score = 18;

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

/// Cost to open a gap (skip characters in choice).
const GAP_OPEN: Score = 6;

/// Cost to extend a gap by one more character.
const GAP_EXTEND: Score = 2;

const TYPO_PENALTY: Score = 4;

/// Penalty for aligning a pattern char to a different choice char (typos only).
const MISMATCH_PENALTY: Score = 12;

/// Extra penalty applied per consecutive Up (typo skip) move.
/// The 2nd and next typos will be penalized by TYPO_PENALTY << CONSECUTIVE_TYPO_FACT
const CONSECUTIVE_TYPO_FACT: Score = 4;

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
    pub fn resize(&mut self, rows: usize, cols: usize) {
        let needed = rows * cols;
        if needed > self.data.len() {
            self.data.resize(needed, CELL_ZERO);
        }
        self.rows = rows;
        self.cols = cols;
    }
}

/// Bitmask lookup for ASCII word separators: ' ' (32), '-' (45), '.' (46), '/' (47),
/// '\\' (92), '_' (95).  Bits 0-63 cover codepoints 0-63; bits 64-127 cover 64-127.
const SEPARATOR_MASK_LO: u64 = (1u64 << 32) | (1u64 << 45) | (1u64 << 46) | (1u64 << 47);
const SEPARATOR_MASK_HI: u64 = (1u64 << (92 - 64)) | (1u64 << (95 - 64));

/// Check if a character is a word separator for bonus computation.
#[inline(always)]
fn is_separator<C: Atom>(c: C) -> bool {
    let ch = c.into() as u32;
    if ch < 64 {
        SEPARATOR_MASK_LO & (1u64 << ch) != 0
    } else if ch < 128 {
        SEPARATOR_MASK_HI & (1u64 << (ch - 64)) != 0
    } else {
        false
    }
}

fn precompute_bonuses<C: Atom>(cho: &[C], buf: &mut Vec<Score>) {
    if cho.is_empty() {
        return;
    }
    buf.reserve(cho.len().saturating_sub(buf.len()));
    let buf_ptr = buf.as_mut_ptr();

    // First character always gets START_OF_STRING_BONUS.
    // SAFETY: We reserved enough capacity above
    unsafe {
        *buf_ptr = START_OF_STRING_BONUS;
    }
    // Remaining characters: look at previous character for word boundary / camelCase.
    for j in 1..cho.len() {
        let prev = cho[j - 1];
        let ch = cho[j];
        let mut bonus: Score = 0;
        if is_separator(prev) {
            bonus += START_OF_WORD_BONUS;
        }
        if prev.is_lowercase() && !ch.is_lowercase() {
            bonus += CAMEL_CASE_BONUS;
        }
        unsafe {
            *buf_ptr.add(j) = bonus;
        }
    }
    // SAFETY: We overwrote all elements of the buf in the previous loop
    unsafe {
        buf.set_len(cho.len());
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

/// Packed cell stored as a `u32`: bits [15:0] = score (as u16 bitcast from
/// i16), bits [17:16] = direction tag.  This gives 4 bytes per cell with no
/// padding and enables branchless direction extraction via bitmask.
#[derive(Copy, Clone)]
struct Cell(u32);

const CELL_ZERO: Cell = Cell::new(0, Dir::None);

impl std::fmt::Debug for Cell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cell")
            .field("score", &self.score())
            .field("dir", &self.dir())
            .finish()
    }
}

impl Cell {
    #[inline(always)]
    pub const fn new(score: Score, dir: Dir) -> Cell {
        // Store score as u16 bits in low 16 bits, dir in bits 16-17.
        Cell((score as u16 as u32) | ((dir as u32) << 16))
    }
    #[inline(always)]
    pub fn score(self) -> Score {
        self.0 as u16 as i16
    }
    #[inline(always)]
    fn dir(self) -> Dir {
        // SAFETY: Dir has repr(u8) with values 0..=3 and we only ever store
        // valid Dir values in bits 16-17.
        unsafe { std::mem::transmute((self.0 >> 16) as u8 & 0x3) }
    }
    /// Branchless check: true when dir == Diag (tag 0).
    #[inline(always)]
    fn is_diag(self) -> bool {
        (self.0 >> 16) & 0x3 == 0
    }
    /// Branchless check: true when dir == Up (tag 1).
    #[inline(always)]
    fn is_up(self) -> bool {
        (self.0 >> 16) & 0x3 == 1
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
    indices_buf: ThreadLocal<RefCell<MatchIndices>>,
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

/// Cheap prefilter for typo-tolerant matching.
///
/// Rejects choices that clearly cannot produce a positive score in the DP.
/// The prefilter is intentionally lenient — false positives are fine (the DP
/// will reject them), but false negatives lose valid matches.
///
/// Strategy: the first pattern character must appear somewhere in the choice
/// (anchoring the alignment). Of the remaining `n - 1` pattern characters,
/// at least `floor((n - 1) / 2)` must also appear (unordered) in the choice.
fn cheap_typo_prefilter<C: Atom>(pattern: &[C], choice: &[C], respect_case: bool) -> bool {
    let n = pattern.len();
    let m = choice.len();

    // A pattern much longer than the choice cannot match.
    if n > m + 2 {
        return false;
    }

    // The first pattern character must be present in the choice.
    let first = pattern[0];
    let mut found_first = false;
    for &c in choice {
        if first.eq(c, respect_case) {
            found_first = true;
            break;
        }
    }
    if !found_first {
        return false;
    }

    if n == 1 {
        return true;
    }

    // Of the remaining n-1 pattern characters, require at least
    // floor((n - 1) / 2) to appear as an in-order subsequence in the choice.
    // We scan the tail pattern chars against the choice left-to-right,
    // counting ordered matches and stopping as soon as we hit the threshold.
    let min_tail = (n - 1) / 2;
    if min_tail == 0 {
        return true;
    }

    let mut matched = 0usize;
    let mut ci = 0usize; // cursor into choice
    for &pi in &pattern[1..] {
        let ci_save = ci;
        let mut found = false;
        while ci < m {
            if pi.eq(choice[ci], respect_case) {
                matched += 1;
                ci += 1;
                found = true;
                break;
            }
            ci += 1;
        }
        // If this pattern char wasn't found, restore the cursor so
        // subsequent pattern chars (the typo-tolerant case) can still
        // scan from the same position.
        if !found {
            ci = ci_save;
        }
        if matched >= min_tail {
            return true;
        }
    }

    false
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
// Banding helpers
// ---------------------------------------------------------------------------

/// For exact (non-typo) mode, compute the earliest column (1-indexed) at which
/// each pattern character can first be matched. This tightens the diagonal
/// lower bound so we never compute cells that cannot participate in a valid
/// alignment.
///
/// Returns `None` if any pattern character has no match in the choice (the
/// subsequence check should have caught this, but we guard anyway).
fn compute_first_match_cols<C: Atom>(pat: &[C], cho: &[C], respect_case: bool) -> Option<[usize; COLMAJOR_MAX_N]> {
    let n = pat.len();
    let mut first = [0usize; COLMAJOR_MAX_N];
    let mut start = 0usize; // search from this choice index onward
    for i in 0..n {
        let found = cho[start..].iter().position(|&c| pat[i].eq(c, respect_case));
        match found {
            Some(pos) => {
                first[i] = start + pos + 1; // 1-indexed column
                start = start + pos + 1; // next char must be strictly after
            }
            None => return None,
        }
    }
    Some(first)
}

/// Compute the last column (1-indexed) at which each pattern character can be
/// matched, scanning from the end. Used to tighten the diagonal upper bound.
fn compute_last_match_cols<C: Atom>(pat: &[C], cho: &[C], respect_case: bool) -> Option<[usize; COLMAJOR_MAX_N]> {
    let n = pat.len();
    let m = cho.len();
    let mut last = [0usize; COLMAJOR_MAX_N];
    let mut end = m; // search up to this choice index (exclusive)
    for i in (0..n).rev() {
        let found = cho[..end].iter().rposition(|&c| pat[i].eq(c, respect_case));
        match found {
            Some(pos) => {
                last[i] = pos + 1; // 1-indexed column
                end = pos; // previous char must be strictly before
            }
            None => return None,
        }
    }
    Some(last)
}

/// For the **row-major** full DP (outer loop over rows), compute per-row
/// column bounds `(j_lo, j_hi)` accounting for cross-row Diag reads.
///
/// Row `i` (1-indexed) matches pattern char `i-1`. The Diag move at
/// `(i, j)` reads `buf[i-1][j-1]`, so row `i-1` must have computed
/// column `j-1`. We expand each row's upper bound to satisfy the next
/// row's lower-bound Diag dependency, and each row's lower bound to
/// satisfy the previous row's upper-bound Diag dependency.
fn compute_row_col_bounds(
    n: usize,
    m: usize,
    first_match: &[usize; COLMAJOR_MAX_N],
    last_match: &[usize; COLMAJOR_MAX_N],
) -> ([usize; COLMAJOR_MAX_N], [usize; COLMAJOR_MAX_N]) {
    let mut lo = [0usize; COLMAJOR_MAX_N];
    let mut hi = [0usize; COLMAJOR_MAX_N];

    // Start with the raw first/last match bounds.
    lo[..n].copy_from_slice(&first_match[..n]);
    hi[..n].copy_from_slice(&last_match[..n]);

    // Forward pass: row i's upper bound must extend so that row i+1 can
    // read Diag at (i+1, j_lo[i+1]) → needs buf[i][j_lo[i+1]-1].
    // Also, LEFT propagation within row i+1 starts at j_lo[i+1], but
    // score flows from row i via Diag, so row i must reach j_lo[i+1]-1.
    for i in 0..n.saturating_sub(1) {
        let next_lo = lo[i + 1];
        if next_lo > 1 {
            hi[i] = hi[i].max(next_lo - 1);
        }
    }

    // Backward pass: row i's lower bound can't be later than row i-1's
    // upper bound + 1 (Diag from (i-1, hi[i-1]) can reach (i, hi[i-1]+1)).
    // This is rarely binding but ensures consistency.
    for i in 1..n {
        lo[i] = lo[i].min(hi[i - 1] + 1);
    }

    // Clamp to valid range.
    for i in 0..n {
        lo[i] = lo[i].max(1).min(m);
        hi[i] = hi[i].max(lo[i]).min(m);
    }

    (lo, hi)
}

/// For the **column-major** score-only DP (outer loop over columns),
/// compute per-column row bounds `(i_lo(j), i_hi(j))` on the fly from
/// per-row column bounds.
///
/// Given per-row bounds `(row_lo[i], row_hi[i])` for i in 0..n (0-indexed,
/// representing rows 1..=n in 1-indexed DP coords), this returns the row
/// range that should be active at column `j` (1-indexed):
///   i_lo = min {i+1 : row_lo[i] <= j}  (first row active at column j)
///   i_hi = max {i+1 : row_hi[i] >= j}  (last row active at column j)
#[inline]
fn col_row_bounds_at(
    j: usize,
    n: usize,
    row_lo: &[usize; COLMAJOR_MAX_N],
    row_hi: &[usize; COLMAJOR_MAX_N],
) -> (usize, usize) {
    // Find first active row (scan forward) and last active row (scan backward).
    let mut i_lo = n + 1;
    for idx in 0..n {
        if row_lo[idx] <= j {
            // this row might be active; confirm row_hi
            if row_hi[idx] >= j {
                i_lo = idx + 1;
                break;
            }
        }
    }

    let mut i_hi = 0usize;
    for idx in (0..n).rev() {
        if row_hi[idx] >= j && row_lo[idx] <= j {
            i_hi = idx + 1;
            break;
        }
    }

    (i_lo, i_hi)
}

/// Bandwidth for typo-mode banding. In typo mode we allow diagonal moves
/// (match/mismatch) plus UP (skip pattern char) and LEFT (skip choice char),
/// so the optimal path can wander off the main diagonal. A bandwidth of
/// `n + TYPO_BAND_SLACK` columns around the diagonal is generous enough
/// to capture all viable alignments while still pruning far-off cells.
const TYPO_BAND_SLACK: usize = 4;

// ---------------------------------------------------------------------------
// Shared DP helpers
// ---------------------------------------------------------------------------

/// Precomputed banding information shared by both score-only and full DP.
struct BandingInfo {
    /// Per-row column bounds (only present in exact mode).
    row_bounds: Option<([usize; COLMAJOR_MAX_N], [usize; COLMAJOR_MAX_N])>,
    /// 1-indexed column of the first match of `pat[0]` in `cho`.
    j_first: usize,
    /// Bandwidth for typo-mode diagonal banding (0 in exact mode).
    bandwidth: usize,
    /// Minimum number of true (non-substitution) matches to accept.
    min_true_matches: usize,
}

/// Compute banding information for the DP. Returns `None` if the pattern
/// cannot possibly match (e.g. a pattern character has no occurrence).
fn compute_banding<const ALLOW_TYPOS: bool, C: Atom>(pat: &[C], cho: &[C], respect_case: bool) -> Option<BandingInfo> {
    let n = pat.len();
    let m = cho.len();
    let row_bounds;
    let j_first;

    if !ALLOW_TYPOS {
        let fm = compute_first_match_cols(pat, cho, respect_case)?;
        let lm = compute_last_match_cols(pat, cho, respect_case)?;
        j_first = fm[0];
        row_bounds = Some(compute_row_col_bounds(n, m, &fm, &lm));
    } else {
        j_first = find_first_char(pat, cho, respect_case)?;
        row_bounds = None;
    }

    let bandwidth = if ALLOW_TYPOS { n + TYPO_BAND_SLACK } else { 0 };
    let min_true_matches = if ALLOW_TYPOS { n.div_ceil(2) } else { 0 };

    Some(BandingInfo {
        row_bounds,
        j_first,
        bandwidth,
        min_true_matches,
    })
}

/// Core cell scoring kernel shared by both score-only and full DP.
///
/// Computes the best score and direction for a single DP cell from its
/// three neighbours (diagonal, up, left). The caller is responsible for
/// fetching the neighbour values from whatever storage layout it uses.
///
/// Returns `(best_score, direction)`. The direction is `Dir::None` when
/// `best_score <= 0`.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn compute_cell<const ALLOW_TYPOS: bool>(
    is_match: bool,
    is_first: bool,
    bonus_j: Score,
    diag_score: Score,
    diag_was_diag: bool,
    up_score: Score,
    up_was_up: bool,
    left_score: Score,
    left_was_diag: bool,
) -> (Score, Dir) {
    // --- Bonus ---
    let mut bonus = bonus_j;
    if diag_was_diag {
        bonus += CONSECUTIVE_BONUS;
    }
    if is_first {
        bonus *= FIRST_CHAR_BONUS_MULTIPLIER;
    }

    // --- DIAGONAL ---
    let diag_val = if is_match {
        diag_score + MATCH_BONUS + bonus
    } else if ALLOW_TYPOS {
        diag_score - MISMATCH_PENALTY
    } else {
        0
    };

    // --- UP (skip pattern char, typos only) ---
    let up_val = if ALLOW_TYPOS {
        let pen = if up_was_up {
            TYPO_PENALTY << CONSECUTIVE_TYPO_FACT
        } else {
            TYPO_PENALTY
        };
        up_score - pen
    } else {
        0
    };

    // --- LEFT (skip choice char) ---
    let left_val = {
        let pen = if left_was_diag { GAP_OPEN } else { GAP_EXTEND };
        left_score - pen
    };

    let best = diag_val.max(up_val).max(left_val);

    let dir = if best <= 0 {
        Dir::None
    } else if diag_val >= up_val && diag_val >= left_val && (is_match || ALLOW_TYPOS) {
        Dir::Diag
    } else if ALLOW_TYPOS && up_val >= left_val {
        Dir::Up
    } else {
        Dir::Left
    };

    (best, dir)
}

/// Find the first and last 1-indexed columns where `pat[0]` matches in `cho`.
///
/// Returns `None` if `pat[0]` is not found anywhere (caller should return
/// `None`). The two positions define the V-shaped banding envelope.
#[inline]
fn find_first_char<C: Atom>(pat: &[C], cho: &[C], respect_case: bool) -> Option<usize> {
    let first = pat[0];
    let mut j_first = 0usize;
    for (idx, &c) in cho.iter().enumerate() {
        if first.eq(c, respect_case) {
            let col = idx + 1; // 1-indexed
            j_first = col;
            break;
        }
    }
    if j_first > 0 { Some(j_first) } else { None }
}

/// Compute typo-mode row bounds at column `j` using a V-shaped band
/// (column-major DP variant).
///
/// The result is an upper triangle starting at the diagonal (j ~ i + j_first - 1)
#[inline(always)]
fn typo_vband(j: usize, n: usize, bandwidth: usize, j_first: usize) -> (usize, usize) {
    // Band 1: diagonal anchored at j_first → i = j - j_first + 1
    let i = (j + 1).saturating_sub(j_first);
    let lo = i.saturating_sub(bandwidth).max(1);

    // Union of the two bands
    (lo, n)
}

/// Row-major variant of V-shaped band: compute column bounds at row `i`.
///
/// The result is an upper triangle starting at the diagonal (j ~ i + j_first - 1)
#[inline(always)]
fn typo_vband_row(i: usize, m: usize, bandwidth: usize, j_first: usize) -> (usize, usize) {
    let j = i + j_first - 1;
    let lo = j.saturating_sub(bandwidth).max(1);

    (lo, m)
}

// ---------------------------------------------------------------------------
// Smith-Waterman DP — Score-only (column-major, stack arrays)
// ---------------------------------------------------------------------------

const COLMAJOR_MAX_N: usize = 16;

/// Column-major score-only for byte slices (n <= 16, stack allocated).
///
/// Implements two pruning strategies:
///
/// 1. **Row-range banding** – for each column `j` only compute rows
///    `i_lo..=i_hi` that can participate in a valid alignment.
///    - Exact mode: bounded by precomputed first/last match columns.
///    - Typo mode: bounded by diagonal ± bandwidth.
///
/// 2. **Interpair max-score pruning** – after processing a column, if every
///    row's score has dropped to 0, all active alignments are dead and we
///    can restart cheaply (reset the "last column with life" counter). If
///    the last row (`i == n`) hasn't seen improvement for many consecutive
///    columns, we can also stop early.
fn score_only_colmajor<const ALLOW_TYPOS: bool, C: Atom>(
    cho: &[C],
    pat: &[C],
    bonuses: &[Score],
    respect_case: bool,
) -> Option<(Score, usize)> {
    let n = pat.len();
    let m = cho.len();

    let banding = compute_banding::<ALLOW_TYPOS, C>(pat, cho, respect_case)?;
    let j_start = banding.j_first; // earliest match — skip columns before this

    // Two columns + direction tracking + true-match count
    let mut score_a: [Score; _] = [0; COLMAJOR_MAX_N + 1];
    let mut score_b: [Score; _] = [0; COLMAJOR_MAX_N + 1];
    let mut diag_a = [false; COLMAJOR_MAX_N + 1];
    let mut diag_b = [false; COLMAJOR_MAX_N + 1];
    // Whether this cell took Up (typo skip) — tracked per-column for
    // consecutive-Up penalty. Only cur_u is read (same column, prior row).
    let mut up_a = [false; COLMAJOR_MAX_N + 1];
    let mut up_b = [false; COLMAJOR_MAX_N + 1];
    // Number of true (non-substitution) matches on the best path to each cell.
    let mut true_a = [0u8; COLMAJOR_MAX_N + 1];
    let mut true_b = [0u8; COLMAJOR_MAX_N + 1];

    let mut prev_s = &mut score_a;
    let mut cur_s = &mut score_b;
    let mut prev_d = &mut diag_a;
    let mut cur_d = &mut diag_b;
    let mut cur_u = &mut up_a;
    let mut spare_u = &mut up_b;
    let mut prev_t = &mut true_a;
    let mut cur_t = &mut true_b;

    let mut global_best: Score = 0;
    let mut global_best_j: usize = 0;
    let mut global_best_true: u8 = 0;

    for j in j_start..=m {
        let cj = cho[j - 1];
        let bonus_j = bonuses[j - 1];

        // --- Compute row bounds for this column ---
        let (i_lo, i_hi) = if ALLOW_TYPOS {
            typo_vband(j, n, banding.bandwidth, banding.j_first)
        } else {
            let (row_lo, row_hi) = banding.row_bounds.as_ref().unwrap();
            col_row_bounds_at(j, n, row_lo, row_hi)
        };

        // Zero out cells outside the band for this column, so they don't
        // carry stale data from a previous column swap.
        cur_s[0] = 0;
        cur_d[0] = false;
        cur_u[0] = false;
        cur_t[0] = 0;

        // Zero cells below the band (rows < i_lo) that UP moves might read.
        for i in 1..i_lo.min(n + 1) {
            cur_s[i] = 0;
            cur_d[i] = false;
            cur_u[i] = false;
            cur_t[i] = 0;
        }
        // Zero cells above the band (rows > i_hi).
        for i in (i_hi + 1)..=n {
            cur_s[i] = 0;
            cur_d[i] = false;
            cur_u[i] = false;
            cur_t[i] = 0;
        }

        if i_lo > i_hi || i_lo > n {
            // No active rows this column — swap and continue.
            std::mem::swap(&mut prev_s, &mut cur_s);
            std::mem::swap(&mut prev_d, &mut cur_d);
            std::mem::swap(&mut cur_u, &mut spare_u);
            std::mem::swap(&mut prev_t, &mut cur_t);
            continue;
        }

        for i in i_lo..=i_hi {
            let pi = pat[i - 1];
            let is_match = pi.eq(cj, respect_case);
            let is_first = i == 1;

            let diag_score = prev_s[i - 1];
            let diag_was_diag = prev_d[i - 1];
            let up_score = cur_s[i - 1]; // same col, prev row (already computed)
            let up_was_up = cur_u[i - 1]; // same col, prev row
            let left_score = prev_s[i]; // prev col, same row
            let left_was_diag = prev_d[i];

            let (best, dir) = compute_cell::<ALLOW_TYPOS>(
                is_match,
                is_first,
                bonus_j,
                diag_score,
                diag_was_diag,
                up_score,
                up_was_up,
                left_score,
                left_was_diag,
            );

            let took_diag = dir == Dir::Diag;
            cur_s[i] = best;
            cur_d[i] = took_diag;
            cur_u[i] = dir == Dir::Up;
            cur_t[i] = if best <= 0 {
                0
            } else if took_diag {
                prev_t[i - 1] + is_match as u8
            } else if ALLOW_TYPOS && dir == Dir::Up {
                cur_t[i - 1]
            } else {
                prev_t[i]
            };

            if i == n && best > global_best {
                global_best = best;
                global_best_j = j;
                global_best_true = cur_t[i];
            }
        }

        std::mem::swap(&mut prev_s, &mut cur_s);
        std::mem::swap(&mut prev_d, &mut cur_d);
        std::mem::swap(&mut cur_u, &mut spare_u);
        std::mem::swap(&mut prev_t, &mut cur_t);
    }

    if global_best > 0 && global_best_true >= banding.min_true_matches as u8 {
        Some((global_best, global_best_j))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Full DP with traceback — packed Cell (u32 = score + dir)
// ---------------------------------------------------------------------------

/// Full DP for byte slices using packed cells.
///
/// Implements two pruning strategies:
///
/// 1. **Row-range banding** – for each row `i` only compute columns
///    `j_lo..=j_hi` that can participate in a valid alignment.
///    - Exact mode: bounded by precomputed first/last match columns.
///    - Typo mode: bounded by diagonal ± bandwidth.
///
/// 2. **Interpair max-score pruning** – after processing a row, if no
///    column produced a non-zero score, all active alignments for this
///    and subsequent rows are dead (since UP/LEFT can only propagate
///    existing scores). We track this and allow early termination.
fn full_dp<const ALLOW_TYPOS: bool, C: Atom>(
    cho: &[C],
    pat: &[C],
    bonuses: &[Score],
    respect_case: bool,
    full_buf: &ThreadLocal<RefCell<SWMatrix>>,
    indices_buf: &ThreadLocal<RefCell<MatchIndices>>,
) -> Option<(Score, MatchIndices)> {
    let n = pat.len();
    let m = cho.len();

    let banding = compute_banding::<ALLOW_TYPOS, C>(pat, cho, respect_case)?;
    let j_start = banding.j_first; // earliest match — skip columns before this

    // Column offset: the matrix stores only columns from j_start onward.
    // Matrix column 0 is the left wall (all zeros); matrix column `jm`
    // corresponds to original 1-indexed column `j = jm + j_start - 1`.
    let col_off = j_start - 1; // subtract from original j to get matrix col
    let mcols = m - col_off + 1; // matrix columns: 0 ..= (m - col_off)

    let mut buf = full_buf
        .get_or(|| RefCell::new(SWMatrix::zero(n + 1, mcols)))
        .borrow_mut();
    buf.resize(n + 1, mcols);

    // Hoist pointer and stride before initialization to use raw access.
    let base_ptr = buf.data.as_mut_ptr();
    let cols = buf.cols;

    // Initialize row 0 and column 0 using raw pointer access (fewer borrows).
    unsafe {
        let row0 = base_ptr;
        for c in 0..mcols {
            *row0.add(c) = CELL_ZERO;
        }
        for i in 1..=n {
            *base_ptr.add(i * cols) = CELL_ZERO;
        }
    }

    let mut best_score = 0;
    let mut best_j = 0usize; // stored in original 1-indexed space

    // base_ptr and cols already set above

    for i in 1..=n {
        let pi = pat[i - 1];
        let is_first = i == 1;

        // --- Compute column bounds for this row (original 1-indexed space) ---
        let (j_lo, j_hi) = if ALLOW_TYPOS {
            typo_vband_row(i, m, banding.bandwidth, banding.j_first)
        } else {
            let (row_lo, row_hi) = banding.row_bounds.as_ref().unwrap();
            (row_lo[i - 1], row_hi[i - 1])
        };
        // No alignment can start before the first occurrence of pat[0].
        let j_lo = j_lo.max(j_start);

        if j_lo > j_hi || j_lo > m {
            // Entire row is outside the band. Only zero the cells the next
            // row's Diag (reads [i][jm-1]) and Up (reads [i][jm]) will touch.
            // Peek at the next row's bounds to limit work.
            if i < n {
                let (nj_lo, nj_hi) = if ALLOW_TYPOS {
                    typo_vband_row(i + 1, m, banding.bandwidth, banding.j_first)
                } else {
                    let (row_lo, row_hi) = banding.row_bounds.as_ref().unwrap();
                    (row_lo[i], row_hi[i])
                };
                let nj_lo = nj_lo.max(j_start);
                if nj_lo <= nj_hi && nj_lo <= m {
                    let njm_lo = nj_lo - col_off;
                    let njm_hi = (nj_hi - col_off).min(mcols - 1);
                    // Diag reads jm-1, Up reads jm → need [njm_lo-1 .. njm_hi].
                    let zero_lo = njm_lo.saturating_sub(1);
                    let zero_hi = njm_hi.min(mcols - 1);
                    // SAFETY: row i is within the allocated matrix.
                    unsafe {
                        let row_ptr = base_ptr.add(i * cols);
                        for k in zero_lo..=zero_hi {
                            *row_ptr.add(k) = CELL_ZERO;
                        }
                    }
                }
            }
            continue;
        }

        // Convert to matrix-local column indices (safe: j_lo >= j_start here).
        let jm_lo = j_lo - col_off;
        let jm_hi = j_hi - col_off;
        let jm_max = mcols - 1; // last valid matrix column

        // Zero only the boundary cells that Diag/Left/Up moves will read:
        // - Cell at jm_lo-1: read by Left at jm_lo and Diag from next row.
        // - Cell at jm_hi+1: read by Up from next row at jm_hi+1 (if in next band).
        // SAFETY: indices are within the row's allocation.
        unsafe {
            let row_ptr = base_ptr.add(i * cols);
            if jm_lo > 1 {
                *row_ptr.add(jm_lo - 1) = CELL_ZERO;
            }
            if jm_hi < jm_max {
                *row_ptr.add(jm_hi + 1) = CELL_ZERO;
            }
        }

        // Get prev_row as immutable slice, cur_row as mutable slice.
        // SAFETY: i >= 1 so rows i-1 and i are distinct; each row is
        // cols-aligned inside the contiguous data vec. base_ptr/cols are
        // hoisted outside the loop.
        let (prev_row, cur_row) = unsafe {
            let pr = std::slice::from_raw_parts(base_ptr.add((i - 1) * cols), cols);
            let cr = std::slice::from_raw_parts_mut(base_ptr.add(i * cols), cols);
            (pr, cr)
        };

        // Hoist raw pointers for unchecked access inside the hot loop.
        let cho_ptr = cho.as_ptr();
        let bonuses_ptr = bonuses.as_ptr();
        let prev_ptr = prev_row.as_ptr();
        let cur_ptr = cur_row.as_mut_ptr();

        for j in j_lo..=j_hi {
            let jm = j - col_off; // matrix column
            // SAFETY: j and jm are inside the band and within array bounds.
            let cj = unsafe { *cho_ptr.add(j - 1) };
            let is_match = pi.eq(cj, respect_case);

            // Fetch neighbour values from the matrix.
            let diag_cell = unsafe { *prev_ptr.add(jm - 1) };
            let (up_score, up_was_up) = if ALLOW_TYPOS {
                let up_cell = unsafe { *prev_ptr.add(jm) };
                (up_cell.score(), up_cell.is_up())
            } else {
                (0, false)
            };
            let left_cell = unsafe { *cur_ptr.add(jm - 1) };

            let (best, dir) = compute_cell::<ALLOW_TYPOS>(
                is_match,
                is_first,
                unsafe { *bonuses_ptr.add(j - 1) },
                diag_cell.score(),
                diag_cell.is_diag(),
                up_score,
                up_was_up,
                left_cell.score(),
                left_cell.is_diag(),
            );

            unsafe {
                *cur_ptr.add(jm) = Cell::new(best, dir);
            }

            if i == n && best > best_score {
                best_score = best;
                best_j = j; // keep in original space
            }
        }
    }

    if best_score <= 0 {
        return None;
    }

    // Traceback — j walks in original 1-indexed space, convert to matrix
    // column for buf access; output indices in original 0-indexed space.
    // Reuse a thread-local Vec to avoid per-call allocation.
    let indices_ref_cell = indices_buf.get_or(|| RefCell::new(Vec::new()));
    let mut indices_ref = indices_ref_cell.borrow_mut();
    indices_ref.clear();
    let mut i = n;
    let mut j = best_j;
    let mut true_matches = 0usize;

    while i > 0 && j >= j_start {
        let jm = j - col_off;
        // SAFETY: jm and i are within the matrix bounds established above.
        let c = unsafe { *base_ptr.add(i * cols).add(jm) };
        match c.dir() {
            Dir::Diag => {
                if pat[i - 1].eq(cho[j - 1], respect_case) {
                    indices_ref.push((j - 1) as IndexType);
                    true_matches += 1;
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

    if true_matches < banding.min_true_matches {
        return None;
    }

    // Traceback produces indices in reverse order; reverse is O(n)
    // vs sort_unstable's O(n log n).
    indices_ref.reverse();

    // Move ownership out of the thread-local buffer by cloning the vec's
    // contents into a fresh Vec (cheap since MatchIndices is Vec<usize>),
    // but avoid an extra clone by using `to_vec()` which reallocates once.
    let out = indices_ref.to_vec();
    Some((best_score, out))
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
                full_dp::<true, _>(cho, pat, &bonus_buf, respect_case, &self.full_buf, &self.indices_buf)
            } else {
                full_dp::<false, _>(cho, pat, &bonus_buf, respect_case, &self.full_buf, &self.indices_buf)
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
            full_dp::<true, _>(
                cho_buf,
                pat_buf,
                &bonus_buf,
                respect_case,
                &self.full_buf,
                &self.indices_buf,
            )
        } else {
            full_dp::<false, _>(
                cho_buf,
                pat_buf,
                &bonus_buf,
                respect_case,
                &self.full_buf,
                &self.indices_buf,
            )
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
        let flat = score("foobar", "fb").unwrap();
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

    // Regression test: all valid subsequences must be returned in --no-typos mode.
    // grep '.*t.*e.*s.*t' should give the same results as skim_v3 with pattern 'test'.
    #[test]
    fn all_subsequences_must_match() {
        let m = matcher();
        let cases = [
            // Bug 1: full_dp tracked best_j across all rows instead of only the
            // last row, so traceback started at the wrong cell.
            "audio/audio/bin/temp/usr/uploads/mnt/cache/media_3445258",
            "audio/audio/audio/docs/cache/temp/downloads/backup/shared/data_9591740",
            // Bug 2: min_true_matches was enforced in exact mode, but the true-count
            // bookkeeping is corrupted by tiebreaking when a character coincidentally
            // matches at a column where diag_score=0 (fresh local alignment start).
            // In exact mode every row increment requires a true match, so score > 0
            // at row n already guarantees n true matches; the threshold is not needed.
            "audio/audio/audio/opt/media/sys/sys/backup/etc_744357",
            "audio/audio/audio/temp/shared/uploads/downloads/config/home/mnt_9037278",
            "audio/audio/opt/cache/usr/usr/var/temp_1579492",
        ];
        for choice in &cases {
            assert!(
                m.fuzzy_match(choice, "test").is_some(),
                "fuzzy_match should match subsequence 'test' in {:?}",
                choice
            );
            assert!(
                m.fuzzy_indices(choice, "test").is_some(),
                "fuzzy_indices should match subsequence 'test' in {:?}",
                choice
            );
        }
    }
}
