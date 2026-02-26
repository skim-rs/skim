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

use std::cell::UnsafeCell;

use memchr::memchr;
use thread_local::ThreadLocal;

/// A newtype wrapping `UnsafeCell<T>` that is `Send`.
///
/// # Safety
/// This is safe to mark `Send` only because the surrounding `ThreadLocal<T>`
/// guarantees that each instance is accessed from at most one thread at a time.
/// Never use this outside of `ThreadLocal` contexts.
struct TLCell<T>(UnsafeCell<T>);
// SAFETY: ThreadLocal ensures single-thread access; we never send an active
// reference across threads.
unsafe impl<T: Send> Send for TLCell<T> {}

impl<T: Default> Default for TLCell<T> {
    fn default() -> Self {
        TLCell(UnsafeCell::new(T::default()))
    }
}

impl<T> std::fmt::Debug for TLCell<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TLCell(...)")
    }
}

/// Helper to get a `&mut T` from a `ThreadLocal<TLCell<T>>`.
///
/// # Safety
/// This is safe as long as:
/// 1. Only one mutable reference to the cell is live at a time (no re-entrant
///    access to the same thread-local on the same call stack).
/// 2. The `ThreadLocal` wrapper ensures distinct per-thread storage, so no
///    concurrent access from other threads is possible.
///
/// All callers in this module satisfy both conditions: each thread-local is
/// borrowed for a short lexical scope, and no two borrows of the same
/// thread-local overlap in any call path.
#[inline(always)]
#[allow(clippy::mut_from_ref)]
// `mut_from_ref` is intentional here: `ThreadLocal` guarantees per-thread
// isolation so the returned `&mut T` cannot alias any reference held by another
// thread, and single-thread aliasing is prevented by the documented call-site
// invariant that no two `tl_get_mut` calls on the same TL overlap.
unsafe fn tl_get_mut<T: Default + Send>(tl: &ThreadLocal<TLCell<T>>) -> &mut T {
    // SAFETY: caller guarantees no aliasing mutable reference is alive.
    unsafe { &mut *tl.get_or_default().0.get() }
}

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

/// Cost to open a gap (skip characters in choice).
const GAP_OPEN: Score = 6;

/// Cost to extend a gap by one more character.
const GAP_EXTEND: Score = 2;

const TYPO_PENALTY: Score = 4;

/// Penalty for aligning a pattern char to a different choice char (typos only).
const MISMATCH_PENALTY: Score = 12;

/// Maximum pattern length supported by the banding arrays (stack-allocated).
const MAX_PAT_LEN: usize = 16;

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

    /// Return the index of the first occurrence of `self` in `haystack`,
    /// or `None` if not found.
    ///
    /// Implementations may override this with a SIMD-backed search (e.g.
    /// `memchr` for `u8` in case-sensitive mode).
    #[inline]
    fn find_first_in(self, haystack: &[Self], respect_case: bool) -> Option<usize> {
        haystack.iter().position(|&c| self.eq(c, respect_case))
    }
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

    /// Case-sensitive search uses SIMD-backed `memchr`; case-insensitive
    /// falls back to the generic scalar loop.
    #[inline]
    fn find_first_in(self, haystack: &[Self], respect_case: bool) -> Option<usize> {
        if respect_case {
            // SAFETY: `self` is a u8 and memchr searches for it in a byte slice.
            memchr(self, haystack)
        } else {
            // Case-insensitive: compare lowercase. Also try the uppercase variant
            // so a single `memchr` can be used for each case variant.
            let lo = self.to_ascii_lowercase();
            let hi = self.to_ascii_uppercase();
            if lo == hi {
                // No case distinction for this byte (digit, symbol, etc.).
                memchr(lo, haystack)
            } else {
                // Check both variants and return the earliest occurrence.
                let p_lo = memchr(lo, haystack);
                let p_hi = memchr(hi, haystack);
                match (p_lo, p_hi) {
                    (None, x) | (x, None) => x,
                    (Some(a), Some(b)) => Some(a.min(b)),
                }
            }
        }
    }
}
impl Atom for char {
    #[inline(always)]
    fn eq_ignore_case(self, b: Self) -> bool {
        // Fast path for ASCII (the common case in filenames and code).
        // eq_ignore_ascii_case is a single comparison vs. the ToLowercase
        // iterator that to_lowercase() requires.
        if self.is_ascii() && b.is_ascii() {
            self.eq_ignore_ascii_case(&b)
        } else {
            self.to_lowercase().eq(b.to_lowercase())
        }
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
    pub fn resize(&mut self, rows: usize, cols: usize) {
        let needed = rows * cols;
        if needed > self.data.len() {
            self.data.resize(needed, CELL_ZERO);
        }
        self.rows = rows;
        self.cols = cols;
    }
}

/// 128-byte lookup table for separator detection. A byte is 1 if the
/// corresponding ASCII codepoint is a word separator, 0 otherwise.
/// Non-ASCII (>= 128) is handled by the bounds check in `is_separator`.
static SEPARATOR_TABLE: [u8; 128] = {
    let mut t = [0u8; 128];
    t[b' ' as usize] = 1;
    t[b'-' as usize] = 1;
    t[b'.' as usize] = 1;
    t[b'/' as usize] = 1;
    t[b'\\' as usize] = 1;
    t[b'_' as usize] = 1;
    t
};

/// Check if a character is a word separator for bonus computation.
/// Uses a table lookup — a single bounds check replaces three branches.
#[inline(always)]
fn is_separator<C: Atom>(c: C) -> bool {
    let ch = c.into() as u32;
    // For ch < 128 we do a table lookup; for ch >= 128 we return false.
    // The `get` returns None for out-of-range, and `copied().unwrap_or(0)` is
    // typically compiled as a conditional move (branchless).
    SEPARATOR_TABLE.get(ch as usize).copied().unwrap_or(0) != 0
}

fn precompute_bonuses<C: Atom>(cho: &[C], buf: &mut Vec<Score>) {
    // Reset length (O(1), no deallocation) then fill with fresh values.
    buf.clear();
    // The first character always gets START_OF_STRING_BONUS.
    // Subsequent characters get a bonus based on the previous character:
    //   - START_OF_WORD_BONUS when the previous char is a separator, or
    //   - CAMEL_CASE_BONUS when transitioning from lowercase to non-lowercase.
    // Using a safe iterator lets the compiler auto-vectorise the loop.
    let bonus_iter = std::iter::once(START_OF_STRING_BONUS).chain(cho.windows(2).map(|w| {
        let prev = w[0];
        let cur = w[1];
        START_OF_WORD_BONUS * (is_separator(prev) as Score)
            + CAMEL_CASE_BONUS * ((prev.is_lowercase() && !cur.is_lowercase()) as Score)
    }));
    buf.extend(bonus_iter);
}

// ---------------------------------------------------------------------------
// Packed cell for full DP: score (u16) + direction (u8) in u32
// ---------------------------------------------------------------------------

/// Direction the optimal path took to reach a cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[allow(dead_code)] // variants are constructed via transmute from bits
enum Dir {
    /// No valid path (score == 0).
    ///
    /// Assigned tag 0 so that `Cell::new(0, Dir::None)` encodes as all-zero
    /// bits, allowing boundary rows/columns to be bulk-zeroed with
    /// `write_bytes(0)` instead of a scalar loop.
    None = 0,
    /// Diagonal: match or mismatch (came from [i-1][j-1])
    Diag = 1,
    /// Up: gap in choice (came from [i-1][j], skip pattern char)
    Up = 2,
    /// Left: gap in pattern (came from [i][j-1], skip choice char)
    Left = 3,
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
    /// Branchless check: true when dir == Diag (tag 1).
    #[inline(always)]
    fn is_diag(self) -> bool {
        (self.0 >> 16) & 0x3 == 1
    }
}

// ---------------------------------------------------------------------------
// SkimV3Matcher
// ---------------------------------------------------------------------------

/// SkimV3 fuzzy matcher: Smith-Waterman local alignment with affine gap
/// penalties and context-sensitive bonuses.
///
/// Thread-local buffers use `TLCell` (an `UnsafeCell` newtype) instead of
/// `RefCell` to avoid the runtime borrow-check overhead. Safety is maintained
/// by the single-writer invariant described on `tl_get_mut`.
#[derive(Debug, Default)]
pub struct SkimV3Matcher {
    pub(crate) case: CaseMatching,
    pub(crate) allow_typos: bool,
    full_buf: ThreadLocal<TLCell<SWMatrix>>,
    /// Two-row rolling buffer used by the score-only DP path (no traceback).
    /// Stores rows as a flat vec: row 0 occupies [0..mcols], row 1 [mcols..2*mcols].
    score_buf: ThreadLocal<TLCell<Vec<Cell>>>,
    indices_buf: ThreadLocal<TLCell<MatchIndices>>,
    #[allow(clippy::type_complexity)]
    char_buf: ThreadLocal<TLCell<(Vec<char>, Vec<char>)>>,
    bonus_buf: ThreadLocal<TLCell<Vec<Score>>>,
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

    /// Dispatch to the appropriate DP with the appropriate const generics.
    /// Assumes prefilters and bonuses have already been computed.
    ///
    /// When `compute_indices` is false we use a 2-row rolling buffer (O(m)
    /// memory instead of O(n×m)) since we never need to traceback through the
    /// full matrix.
    fn dispatch_dp<C: Atom>(
        &self,
        cho: &[C],
        pat: &[C],
        bonuses: &[Score],
        respect_case: bool,
        compute_indices: bool,
    ) -> Option<(ScoreType, MatchIndices)> {
        let res = if compute_indices {
            // Full matrix needed for traceback.
            if self.allow_typos {
                full_dp::<true, _>(cho, pat, bonuses, respect_case, &self.full_buf, &self.indices_buf)
            } else {
                full_dp::<false, _>(cho, pat, bonuses, respect_case, &self.full_buf, &self.indices_buf)
            }
        } else {
            // Score-only: 2-row rolling buffer, no traceback.
            let score = if self.allow_typos {
                score_only_dp::<true, _>(cho, pat, bonuses, respect_case, &self.score_buf)
            } else {
                score_only_dp::<false, _>(cho, pat, bonuses, respect_case, &self.score_buf)
            };
            score.map(|s| (s, MatchIndices::default()))
        };
        res.map(|(s, idx)| (s as ScoreType, idx))
    }

    /// Generic helper: run full DP over slices of Atom.
    /// If `compute_indices` is true, returns the matched indices; otherwise
    /// returns a single-element vec containing the 1-indexed end column.
    fn match_slices<C: Atom>(&self, cho: &[C], pat: &[C], compute_indices: bool) -> Option<(ScoreType, MatchIndices)> {
        if pat.is_empty() {
            return Some((0, MatchIndices::new()));
        }
        if cho.is_empty() {
            return None;
        }

        let respect_case = self.respect_case(pat);

        // Prefilter for typo mode.
        // In exact mode (non-typo) we skip is_subsequence here: compute_banding
        // calls compute_first_match_cols which already validates the subsequence
        // and returns None if any pattern character is absent — no redundant scan.
        if self.allow_typos && !cheap_typo_prefilter(pat, cho, respect_case) {
            return None;
        }

        // Prepare bonuses.
        // SAFETY: bonus_buf is not accessed elsewhere on this call stack.
        let bonus_buf = unsafe { tl_get_mut(&self.bonus_buf) };
        precompute_bonuses(cho, bonus_buf);

        self.dispatch_dp(cho, pat, bonus_buf, respect_case, compute_indices)
    }

    /// Generic helper: run range DP over slices of Atom.
    ///
    /// Shared by the ASCII and non-ASCII paths of `run_range`.  Mirrors
    /// `match_slices` but calls `range_dp` instead of `dispatch_dp`.
    fn match_slices_range<C: Atom>(&self, cho: &[C], pat: &[C]) -> Option<(ScoreType, usize, usize)> {
        if pat.is_empty() {
            return Some((0, 0, 0));
        }
        if cho.is_empty() {
            return None;
        }

        let respect_case = self.respect_case(pat);

        // Prefilter for typo mode; compute_banding handles the exact-mode
        // subsequence check implicitly.
        if self.allow_typos && !cheap_typo_prefilter(pat, cho, respect_case) {
            return None;
        }

        // SAFETY: bonus_buf is not accessed elsewhere on this call stack.
        let bonus_buf = unsafe { tl_get_mut(&self.bonus_buf) };
        precompute_bonuses(cho, bonus_buf);

        let res = if self.allow_typos {
            range_dp::<true, _>(cho, pat, bonus_buf, respect_case, &self.full_buf)
        } else {
            range_dp::<false, _>(cho, pat, bonus_buf, respect_case, &self.full_buf)
        };
        res.map(|(s, b, e)| (s as ScoreType, b, e))
    }

    fn run(&self, choice: &str, pattern: &str, compute_indices: bool) -> Option<(ScoreType, MatchIndices)> {
        if pattern.is_empty() {
            return Some((0, MatchIndices::new()));
        }
        if choice.is_empty() {
            return None;
        }

        // Fast path for ASCII matching
        if choice.is_ascii() && pattern.is_ascii() {
            let cho = choice.as_bytes();
            let pat = pattern.as_bytes();
            return self.match_slices(cho, pat, compute_indices);
        }

        // SAFETY: char_buf is not accessed elsewhere on this call stack.
        let bufs = unsafe { tl_get_mut(&self.char_buf) };
        let (ref mut pat_buf, ref mut cho_buf) = *bufs;
        pat_buf.clear();
        pat_buf.extend(pattern.chars());
        cho_buf.clear();
        cho_buf.extend(choice.chars());

        let respect_case = self.respect_case(pat_buf);

        // Prefilter for typo mode only (see match_slices for rationale).
        if self.allow_typos && !cheap_typo_prefilter(pat_buf, cho_buf, respect_case) {
            return None;
        }

        // SAFETY: bonus_buf is not accessed elsewhere on this call stack.
        // char_buf and bonus_buf are distinct thread-locals; no aliasing.
        let bonus_buf = unsafe { tl_get_mut(&self.bonus_buf) };
        precompute_bonuses(cho_buf, bonus_buf);

        // Call dispatch_dp directly to avoid re-borrowing bonus_buf.
        self.dispatch_dp(cho_buf, pat_buf, bonus_buf, respect_case, compute_indices)
    }

    /// Run the DP and return `(score, begin, end)` without collecting all indices.
    ///
    /// Uses the full matrix (for traceback) but only records the first and last
    /// matched columns instead of the full index list. Avoids the allocation and
    /// work of `fuzzy_indices` when only the range is needed.
    fn run_range(&self, choice: &str, pattern: &str) -> Option<(ScoreType, usize, usize)> {
        if pattern.is_empty() {
            return Some((0, 0, 0));
        }
        if choice.is_empty() {
            return None;
        }

        if choice.is_ascii() && pattern.is_ascii() {
            return self.match_slices_range(choice.as_bytes(), pattern.as_bytes());
        }

        // Non-ASCII: convert to char slices in the thread-local buffer, then
        // call the generic helper.
        // SAFETY: char_buf is not accessed elsewhere on this call stack.
        let bufs = unsafe { tl_get_mut(&self.char_buf) };
        let (ref mut pat_buf, ref mut cho_buf) = *bufs;
        pat_buf.clear();
        pat_buf.extend(pattern.chars());
        cho_buf.clear();
        cho_buf.extend(choice.chars());

        // Temporarily reborrow as slices to satisfy the borrow checker; the
        // char_buf borrow ends before match_slices_range borrows bonus_buf.
        let (pat_slice, cho_slice): (&[char], &[char]) = (pat_buf, cho_buf);

        // SAFETY: bonus_buf and char_buf are distinct thread-locals.
        let respect_case = self.respect_case(pat_slice);
        if self.allow_typos && !cheap_typo_prefilter(pat_slice, cho_slice, respect_case) {
            return None;
        }
        let bonus_buf = unsafe { tl_get_mut(&self.bonus_buf) };
        precompute_bonuses(cho_slice, bonus_buf);
        let res = if self.allow_typos {
            range_dp::<true, _>(cho_slice, pat_slice, bonus_buf, respect_case, &self.full_buf)
        } else {
            range_dp::<false, _>(cho_slice, pat_slice, bonus_buf, respect_case, &self.full_buf)
        };
        res.map(|(s, b, e)| (s as ScoreType, b, e))
    }
}

// ---------------------------------------------------------------------------
// Prefilters
// ---------------------------------------------------------------------------

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
    // Use the SIMD-backed find_first_in (memchr for u8, scalar for char).
    let first = pattern[0];
    if first.find_first_in(choice, respect_case).is_none() {
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
fn compute_first_match_cols<C: Atom>(pat: &[C], cho: &[C], respect_case: bool) -> Option<[usize; MAX_PAT_LEN]> {
    let n = pat.len();
    // Patterns longer than MAX_PAT_LEN cannot be handled by the stack-allocated
    // banding arrays.  Return None so the caller skips this choice gracefully.
    if n > MAX_PAT_LEN {
        return None;
    }
    let mut first = [0usize; MAX_PAT_LEN];
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
fn compute_last_match_cols<C: Atom>(pat: &[C], cho: &[C], respect_case: bool) -> Option<[usize; MAX_PAT_LEN]> {
    let n = pat.len();
    // Patterns longer than MAX_PAT_LEN cannot be handled by the stack-allocated
    // banding arrays.  Return None so the caller skips this choice gracefully.
    if n > MAX_PAT_LEN {
        return None;
    }
    let m = cho.len();
    let mut last = [0usize; MAX_PAT_LEN];
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
    first_match: &[usize; MAX_PAT_LEN],
    last_match: &[usize; MAX_PAT_LEN],
) -> ([usize; MAX_PAT_LEN], [usize; MAX_PAT_LEN]) {
    let mut lo = [0usize; MAX_PAT_LEN];
    let mut hi = [0usize; MAX_PAT_LEN];

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
    row_bounds: Option<([usize; MAX_PAT_LEN], [usize; MAX_PAT_LEN])>,
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
///
/// This function is written in a branchless style: all scoring arithmetic
/// uses `bool as Score` multipliers and `max` instead of if/else, and the
/// final direction is selected via a branchless cascade of conditional moves.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn compute_cell<const ALLOW_TYPOS: bool>(
    is_match: bool,
    is_first: bool,
    bonus_j: Score,
    diag_score: Score,
    diag_was_diag: bool,
    up_score: Score,
    left_score: Score,
    left_was_diag: bool,
) -> (Score, Dir) {
    // --- Bonus (branchless) ---
    // consecutive bonus added when diag_was_diag, first-char multiplier doubles the bonus.
    // `bool as Score` is 0 or 1 — no branch.
    let bonus = (bonus_j + CONSECUTIVE_BONUS * (diag_was_diag as Score)) * (1 + is_first as Score);

    // --- DIAGONAL (branchless) ---
    // Match path: diag_score + MATCH_BONUS + bonus, masked by is_match.
    // Mismatch path (typos only): diag_score - MISMATCH_PENALTY, masked by !is_match.
    let match_val = (diag_score + MATCH_BONUS + bonus) * (is_match as Score);
    let mismatch_val = if ALLOW_TYPOS {
        (diag_score - MISMATCH_PENALTY) * (!is_match as Score)
    } else {
        0
    };
    let diag_val = match_val + mismatch_val;

    // --- UP (skip pattern char, typos only — const-generic elides entirely) ---
    let up_val = if ALLOW_TYPOS { up_score - TYPO_PENALTY } else { 0 };

    // --- LEFT (skip choice char, branchless gap penalty) ---
    // GAP_OPEN when left_was_diag, GAP_EXTEND otherwise.
    // pen = GAP_EXTEND + (GAP_OPEN - GAP_EXTEND) * left_was_diag
    let left_val = left_score - (GAP_EXTEND + (GAP_OPEN - GAP_EXTEND) * (left_was_diag as Score));

    // --- Best score (branchless max chain) ---
    let best = diag_val.max(up_val).max(left_val);

    // --- Direction (branchless select) ---
    // We encode direction as a u8 and build it without branches.
    // Priority: Diag > Up > Left > None (when best <= 0).
    //
    // Start with Left (2), override with Up if up wins, override with Diag
    // if diag wins, override with None if best <= 0.
    // For exact mode (ALLOW_TYPOS=false), Diag is only valid when is_match.
    let diag_wins = if ALLOW_TYPOS {
        diag_val >= up_val && diag_val >= left_val
    } else {
        is_match && diag_val >= left_val
    };
    let up_wins = ALLOW_TYPOS && !diag_wins && up_val >= left_val;

    // Branchless cascade: select dir as integer.
    // Dir encoding: None=0, Diag=1, Up=2, Left=3.
    // Base is Left(3); subtract 1 if Up wins, subtract 2 if Diag wins.
    let dir_bits: u8 = Dir::Left as u8 - (up_wins as u8) - (diag_wins as u8) * 2;
    // If best <= 0, force Dir::None (0) — achieved by ANDing with all-zeros.
    let positive = best > 0;
    // When positive: dir_bits; when not: 0 (Dir::None).
    let dir_val = dir_bits & (positive as u8).wrapping_neg();

    // SAFETY: dir_val is in 0..=3 because of the construction above.
    let dir: Dir = unsafe { std::mem::transmute(dir_val) };

    (best, dir)
}

/// Find the 1-indexed column of the first occurrence of `pat[0]` in `cho`.
///
/// Returns `None` if `pat[0]` is not found anywhere (caller should return
/// `None`). The position defines the start of the V-shaped banding envelope.
/// Uses SIMD-backed `find_first_in` for `u8` slices.
#[inline]
fn find_first_char<C: Atom>(pat: &[C], cho: &[C], respect_case: bool) -> Option<usize> {
    pat[0].find_first_in(cho, respect_case).map(|idx| idx + 1) // 1-indexed
}

/// Row-major V-shaped band: compute column bounds at row `i`.
///
/// Lower bound is tightened around the diagonal; the upper bound is left at
/// `m` so that alignments that skip many choice characters (large LEFT runs)
/// are never pruned — the affine gap penalty keeps them from winning anyway.
#[inline(always)]
fn typo_vband_row(i: usize, m: usize, bandwidth: usize, j_first: usize) -> (usize, usize) {
    let j = i + j_first - 1;
    let lo = j.saturating_sub(bandwidth).max(1);

    (lo, m)
}

// ---------------------------------------------------------------------------
// Score-only DP — 2-row rolling buffer (no traceback)
// ---------------------------------------------------------------------------

/// Score-only DP using a 2-row rolling buffer.
///
/// Since we never need to traceback through the full matrix when only the
/// score is requested, we store only two rows at a time: the previous row
/// (`prev`) and the current row (`cur`). After processing each row we swap
/// the two buffers. This reduces memory from O(n×m) to O(m) and dramatically
/// improves cache utilization for large choice strings.
///
/// **Early termination**: after processing a row, if no cell had a positive
/// score we increment a dead-row counter. Once the counter reaches 2, we
/// terminate — no subsequent row can produce a positive score since gap
/// penalties can only decrease an existing score.
///
/// The banding and cell computation logic is identical to `full_dp`.
fn score_only_dp<const ALLOW_TYPOS: bool, C: Atom>(
    cho: &[C],
    pat: &[C],
    bonuses: &[Score],
    respect_case: bool,
    score_buf: &ThreadLocal<TLCell<Vec<Cell>>>,
) -> Option<Score> {
    let n = pat.len();
    let m = cho.len();

    let banding = compute_banding::<ALLOW_TYPOS, C>(pat, cho, respect_case)?;
    let j_start = banding.j_first;

    let col_off = j_start - 1;
    let mcols = m - col_off + 1; // columns: 0 ..= (m - col_off)

    // Allocate/reuse a flat buffer for two rows: [prev_row | cur_row].
    let needed = 2 * mcols;
    // SAFETY: score_buf is not accessed elsewhere on this call stack.
    let score_buf_ref = unsafe { tl_get_mut(score_buf) };
    if score_buf_ref.len() < needed {
        score_buf_ref.resize(needed, CELL_ZERO);
    }
    let buf_ptr = score_buf_ref.as_mut_ptr();

    // Initialize both rows to CELL_ZERO (all-zero bytes: score=0, dir=None=0).
    // SAFETY: buf_ptr points to `needed` valid Cell slots; Cell is u32-aligned.
    unsafe {
        std::ptr::write_bytes(buf_ptr, 0, needed);
    }

    // Pre-extract row bounds.
    let (row_lo_arr, row_hi_arr) = if !ALLOW_TYPOS {
        let (lo, hi) = banding.row_bounds.as_ref().unwrap();
        (*lo, *hi)
    } else {
        ([0usize; MAX_PAT_LEN], [0usize; MAX_PAT_LEN])
    };

    let cho_ptr = cho.as_ptr();
    let bonuses_ptr = bonuses.as_ptr();

    // `cur_half` toggles between 0 and 1 selecting which half of the buffer
    // is the "current" row. The other half is the "previous" row.
    let mut cur_half = 0usize;
    // Consecutive all-zero-row counter for early termination.
    let mut dead_rows = 0u32;

    for i in 1..=n {
        let pi = pat[i - 1];
        let is_first = i == 1;

        let (j_lo, j_hi) = if ALLOW_TYPOS {
            typo_vband_row(i, m, banding.bandwidth, banding.j_first)
        } else {
            (row_lo_arr[i - 1], row_hi_arr[i - 1])
        };
        let j_lo = j_lo.max(j_start);

        // Swap: cur_half becomes prev, the new cur_half is the other half.
        let prev_half = cur_half;
        cur_half = 1 - cur_half;

        let prev_ptr = unsafe { buf_ptr.add(prev_half * mcols) };
        let cur_ptr = unsafe { buf_ptr.add(cur_half * mcols) };

        if j_lo > j_hi || j_lo > m {
            // Row outside band: zero the new current row so the next row
            // reads clean values.
            unsafe {
                for k in 0..mcols {
                    *cur_ptr.add(k) = CELL_ZERO;
                }
            }
            dead_rows += 1;
            if dead_rows >= 2 {
                return None;
            }
            continue;
        }

        let jm_lo = j_lo - col_off;
        let jm_hi = j_hi - col_off;
        let jm_max = mcols - 1;

        // Zero the boundary sentinels and cells outside the band.
        unsafe {
            // Zero everything before jm_lo (including the left sentinel at jm_lo-1).
            for k in 0..jm_lo {
                *cur_ptr.add(k) = CELL_ZERO;
            }
            // Zero everything after jm_hi (including the right sentinel at jm_hi+1).
            for k in (jm_hi + 1)..=jm_max {
                *cur_ptr.add(k) = CELL_ZERO;
            }
        }

        let mut row_positive = false;
        for j in j_lo..=j_hi {
            let jm = j - col_off;
            let cj = unsafe { *cho_ptr.add(j - 1) };
            let is_match = pi.eq(cj, respect_case);

            let diag_cell = unsafe { *prev_ptr.add(jm - 1) };
            let up_score = if ALLOW_TYPOS {
                unsafe { (*prev_ptr.add(jm)).score() }
            } else {
                0
            };
            let left_cell = unsafe { *cur_ptr.add(jm - 1) };

            let (best, dir) = compute_cell::<ALLOW_TYPOS>(
                is_match,
                is_first,
                unsafe { *bonuses_ptr.add(j - 1) },
                diag_cell.score(),
                diag_cell.is_diag(),
                up_score,
                left_cell.score(),
                left_cell.is_diag(),
            );

            row_positive |= best > 0;
            unsafe {
                *cur_ptr.add(jm) = Cell::new(best, dir);
            }
        }

        // Early termination: if this row had no positive score, no downstream
        // row can produce one either (gap penalties only decrease scores).
        if row_positive {
            dead_rows = 0;
        } else {
            dead_rows += 1;
            if dead_rows >= 2 {
                return None;
            }
        }
    }

    // Scan the last row (cur_half) for the best score.
    let (last_j_lo, last_j_hi) = if ALLOW_TYPOS {
        typo_vband_row(n, m, banding.bandwidth, banding.j_first)
    } else {
        (row_lo_arr[n - 1], row_hi_arr[n - 1])
    };
    let last_j_lo = last_j_lo.max(j_start);

    let mut best_score: Score = 0;
    if last_j_lo <= last_j_hi && last_j_lo <= m {
        let last_row_ptr = unsafe { buf_ptr.add(cur_half * mcols) };
        for j in last_j_lo..=last_j_hi {
            let jm = j - col_off;
            let s = unsafe { (*last_row_ptr.add(jm)).score() };
            let better = s > best_score;
            best_score = if better { s } else { best_score };
        }
    }

    if best_score > 0 { Some(best_score) } else { None }
}

// ---------------------------------------------------------------------------
// Full DP with traceback — packed Cell (u32 = score + dir)
// ---------------------------------------------------------------------------

/// Full DP for byte slices using packed cells, with traceback support.
///
/// Allocates the full `(n+1) × mcols` matrix so that traceback can follow
/// direction pointers back to the source. Use `score_only_dp` instead when
/// only the score is needed — it uses a O(m) rolling buffer.
///
/// Implements row-range banding:
///
/// - Exact mode: bounded by precomputed first/last match columns.
/// - Typo mode: bounded by diagonal ± bandwidth.
fn full_dp<const ALLOW_TYPOS: bool, C: Atom>(
    cho: &[C],
    pat: &[C],
    bonuses: &[Score],
    respect_case: bool,
    full_buf: &ThreadLocal<TLCell<SWMatrix>>,
    indices_buf: &ThreadLocal<TLCell<MatchIndices>>,
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

    // SAFETY: full_buf is not accessed elsewhere on this call stack.
    let buf = unsafe { tl_get_mut(full_buf) };
    buf.resize(n + 1, mcols);

    // Hoist pointer and stride before initialization to use raw access.
    let base_ptr = buf.data.as_mut_ptr();
    let cols = buf.cols;

    // Initialize row 0 to CELL_ZERO (all-zero bytes: score=0, dir=None=0).
    // Column 0 of each subsequent row is also CELL_ZERO.
    // SAFETY: base_ptr points to a valid allocation of (n+1)*cols Cells.
    unsafe {
        // Row 0: mcols contiguous Cells starting at base_ptr.
        std::ptr::write_bytes(base_ptr, 0, mcols);
        // Column 0 of rows 1..=n: one Cell per row, stride = cols.
        for i in 1..=n {
            *base_ptr.add(i * cols) = CELL_ZERO;
        }
    }

    // Pre-extract row bounds once (avoids repeated unwrap inside the loop).
    // For exact mode we copy the arrays out; for typo mode these are unused.
    let (row_lo_arr, row_hi_arr) = if !ALLOW_TYPOS {
        let (lo, hi) = banding.row_bounds.as_ref().unwrap();
        (*lo, *hi)
    } else {
        ([0usize; MAX_PAT_LEN], [0usize; MAX_PAT_LEN])
    };

    // Hoist invariant pointers outside the row loop.
    let cho_ptr = cho.as_ptr();
    let bonuses_ptr = bonuses.as_ptr();
    // Consecutive all-zero-row counter for early termination.
    let mut dead_rows = 0u32;

    for i in 1..=n {
        let pi = pat[i - 1];
        let is_first = i == 1;

        // --- Compute column bounds for this row (original 1-indexed space) ---
        let (j_lo, j_hi) = if ALLOW_TYPOS {
            typo_vband_row(i, m, banding.bandwidth, banding.j_first)
        } else {
            (row_lo_arr[i - 1], row_hi_arr[i - 1])
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
                    (row_lo_arr[i], row_hi_arr[i])
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
            dead_rows += 1;
            if dead_rows >= 2 {
                return None;
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
        let prev_ptr = prev_row.as_ptr();
        let cur_ptr = cur_row.as_mut_ptr();

        let mut row_positive = false;
        for j in j_lo..=j_hi {
            let jm = j - col_off; // matrix column
            // SAFETY: j and jm are inside the band and within array bounds.
            let cj = unsafe { *cho_ptr.add(j - 1) };
            let is_match = pi.eq(cj, respect_case);

            // Fetch neighbour values from the matrix.
            let diag_cell = unsafe { *prev_ptr.add(jm - 1) };
            let up_score = if ALLOW_TYPOS {
                let up_cell = unsafe { *prev_ptr.add(jm) };
                up_cell.score()
            } else {
                0
            };
            let left_cell = unsafe { *cur_ptr.add(jm - 1) };

            let (best, dir) = compute_cell::<ALLOW_TYPOS>(
                is_match,
                is_first,
                unsafe { *bonuses_ptr.add(j - 1) },
                diag_cell.score(),
                diag_cell.is_diag(),
                up_score,
                left_cell.score(),
                left_cell.is_diag(),
            );

            row_positive |= best > 0;
            unsafe {
                *cur_ptr.add(jm) = Cell::new(best, dir);
            }
        }

        // Early termination: if this row had no positive score, no downstream
        // row can produce one either.
        if row_positive {
            dead_rows = 0;
        } else {
            dead_rows += 1;
            if dead_rows >= 2 {
                return None;
            }
        }
    }

    // --- Find best score in the last row (row n) ---
    // Moved out of the inner loop to eliminate the `i == n` branch per cell.
    let mut best_score: Score = 0;
    let mut best_j = 0usize; // stored in original 1-indexed space
    {
        let (last_j_lo, last_j_hi) = if ALLOW_TYPOS {
            typo_vband_row(n, m, banding.bandwidth, banding.j_first)
        } else {
            (row_lo_arr[n - 1], row_hi_arr[n - 1])
        };
        let last_j_lo = last_j_lo.max(j_start);
        if last_j_lo <= last_j_hi && last_j_lo <= m {
            let last_row_ptr = unsafe { base_ptr.add(n * cols) };
            for j in last_j_lo..=last_j_hi {
                let jm = j - col_off;
                let s = unsafe { (*last_row_ptr.add(jm)).score() };
                // Branchless max: update best_score and best_j together.
                let better = s > best_score;
                // Use conditional moves instead of a branch.
                best_score = if better { s } else { best_score };
                best_j = if better { j } else { best_j };
            }
        }
    }

    if best_score <= 0 {
        return None;
    }

    // Traceback — j walks in original 1-indexed space, convert to matrix
    // column for buf access; output indices in original 0-indexed space.
    // SAFETY: indices_buf is not accessed elsewhere on this call stack.
    let indices_ref = unsafe { tl_get_mut(indices_buf) };
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

    // Move ownership out of the thread-local buffer without cloning: swap it
    // with an empty Vec so we return the populated Vec directly. The thread-local
    // then starts the next call with a zero-capacity Vec and will reallocate on
    // first push — a small one-time cost traded for zero-copy return here.
    let out = std::mem::take(indices_ref);
    Some((best_score, out))
}

// ---------------------------------------------------------------------------
// Range DP — full matrix, minimal traceback (begin + end only)
// ---------------------------------------------------------------------------

/// Full matrix DP followed by a traceback that only records the first and
/// last matched positions (not every index). Used by `fuzzy_match_range` to
/// avoid allocating and populating the full index vec when only the span is
/// needed.
fn range_dp<const ALLOW_TYPOS: bool, C: Atom>(
    cho: &[C],
    pat: &[C],
    bonuses: &[Score],
    respect_case: bool,
    full_buf: &ThreadLocal<TLCell<SWMatrix>>,
) -> Option<(Score, usize, usize)> {
    let n = pat.len();
    let m = cho.len();

    let banding = compute_banding::<ALLOW_TYPOS, C>(pat, cho, respect_case)?;
    let j_start = banding.j_first;
    let col_off = j_start - 1;
    let mcols = m - col_off + 1;

    // SAFETY: full_buf is not accessed elsewhere on this call stack.
    let buf = unsafe { tl_get_mut(full_buf) };
    buf.resize(n + 1, mcols);

    let base_ptr = buf.data.as_mut_ptr();
    let cols = buf.cols;

    // Initialize row 0 to CELL_ZERO (all-zero bytes: score=0, dir=None=0).
    // Column 0 of each subsequent row is also CELL_ZERO.
    // SAFETY: base_ptr points to a valid allocation of (n+1)*cols Cells.
    unsafe {
        std::ptr::write_bytes(base_ptr, 0, mcols);
        for i in 1..=n {
            *base_ptr.add(i * cols) = CELL_ZERO;
        }
    }

    let (row_lo_arr, row_hi_arr) = if !ALLOW_TYPOS {
        let (lo, hi) = banding.row_bounds.as_ref().unwrap();
        (*lo, *hi)
    } else {
        ([0usize; MAX_PAT_LEN], [0usize; MAX_PAT_LEN])
    };

    let cho_ptr = cho.as_ptr();
    let bonuses_ptr = bonuses.as_ptr();
    let mut dead_rows = 0u32;

    for i in 1..=n {
        let pi = pat[i - 1];
        let is_first = i == 1;

        let (j_lo, j_hi) = if ALLOW_TYPOS {
            typo_vband_row(i, m, banding.bandwidth, banding.j_first)
        } else {
            (row_lo_arr[i - 1], row_hi_arr[i - 1])
        };
        let j_lo = j_lo.max(j_start);

        if j_lo > j_hi || j_lo > m {
            if i < n {
                let (nj_lo, nj_hi) = if ALLOW_TYPOS {
                    typo_vband_row(i + 1, m, banding.bandwidth, banding.j_first)
                } else {
                    (row_lo_arr[i], row_hi_arr[i])
                };
                let nj_lo = nj_lo.max(j_start);
                if nj_lo <= nj_hi && nj_lo <= m {
                    let njm_lo = nj_lo - col_off;
                    let njm_hi = (nj_hi - col_off).min(mcols - 1);
                    let zero_lo = njm_lo.saturating_sub(1);
                    let zero_hi = njm_hi.min(mcols - 1);
                    unsafe {
                        let row_ptr = base_ptr.add(i * cols);
                        for k in zero_lo..=zero_hi {
                            *row_ptr.add(k) = CELL_ZERO;
                        }
                    }
                }
            }
            dead_rows += 1;
            if dead_rows >= 2 {
                return None;
            }
            continue;
        }

        let jm_lo = j_lo - col_off;
        let jm_hi = j_hi - col_off;
        let jm_max = mcols - 1;

        unsafe {
            let row_ptr = base_ptr.add(i * cols);
            if jm_lo > 1 {
                *row_ptr.add(jm_lo - 1) = CELL_ZERO;
            }
            if jm_hi < jm_max {
                *row_ptr.add(jm_hi + 1) = CELL_ZERO;
            }
        }

        let (prev_row, cur_row) = unsafe {
            let pr = std::slice::from_raw_parts(base_ptr.add((i - 1) * cols), cols);
            let cr = std::slice::from_raw_parts_mut(base_ptr.add(i * cols), cols);
            (pr, cr)
        };

        let prev_ptr = prev_row.as_ptr();
        let cur_ptr = cur_row.as_mut_ptr();

        let mut row_positive = false;
        for j in j_lo..=j_hi {
            let jm = j - col_off;
            let cj = unsafe { *cho_ptr.add(j - 1) };
            let is_match = pi.eq(cj, respect_case);

            let diag_cell = unsafe { *prev_ptr.add(jm - 1) };
            let up_score = if ALLOW_TYPOS {
                let up_cell = unsafe { *prev_ptr.add(jm) };
                up_cell.score()
            } else {
                0
            };
            let left_cell = unsafe { *cur_ptr.add(jm - 1) };

            let (best, dir) = compute_cell::<ALLOW_TYPOS>(
                is_match,
                is_first,
                unsafe { *bonuses_ptr.add(j - 1) },
                diag_cell.score(),
                diag_cell.is_diag(),
                up_score,
                left_cell.score(),
                left_cell.is_diag(),
            );

            row_positive |= best > 0;
            unsafe {
                *cur_ptr.add(jm) = Cell::new(best, dir);
            }
        }

        if row_positive {
            dead_rows = 0;
        } else {
            dead_rows += 1;
            if dead_rows >= 2 {
                return None;
            }
        }
    }

    // Find best score in the last row.
    let mut best_score: Score = 0;
    let mut best_j = 0usize;
    {
        let (last_j_lo, last_j_hi) = if ALLOW_TYPOS {
            typo_vband_row(n, m, banding.bandwidth, banding.j_first)
        } else {
            (row_lo_arr[n - 1], row_hi_arr[n - 1])
        };
        let last_j_lo = last_j_lo.max(j_start);
        if last_j_lo <= last_j_hi && last_j_lo <= m {
            let last_row_ptr = unsafe { base_ptr.add(n * cols) };
            for j in last_j_lo..=last_j_hi {
                let jm = j - col_off;
                let s = unsafe { (*last_row_ptr.add(jm)).score() };
                let better = s > best_score;
                best_score = if better { s } else { best_score };
                best_j = if better { j } else { best_j };
            }
        }
    }

    if best_score <= 0 {
        return None;
    }

    // Minimal traceback: walk back until we can go no further, recording
    // only the final j (which becomes `begin`). `end` is best_j - 1.
    let end_0 = best_j - 1; // 0-indexed end
    let mut i = n;
    let mut j = best_j;
    let mut true_matches = 0usize;

    while i > 0 && j >= j_start {
        let jm = j - col_off;
        let c = unsafe { *base_ptr.add(i * cols).add(jm) };
        match c.dir() {
            Dir::Diag => {
                if pat[i - 1].eq(cho[j - 1], respect_case) {
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

    // `j` after traceback is one step before the first matched column;
    // the first match is at `j` (0-indexed: `j` since j is 1-indexed here
    // but we stepped past it). We need the earliest index that was recorded.
    // After the loop, j points to the column just before the alignment start,
    // so begin = j (0-indexed) because the first Diag step decremented j before
    // breaking. Re-scan the last row of the traceback to find begin precisely:
    // We track the last diagonal j we visited.
    let begin_0 = j; // j is 1-indexed after the last decrement; 0-indexed = j

    Some((best_score, begin_0, end_0))
}

// ---------------------------------------------------------------------------
// FuzzyMatcher trait implementation
// ---------------------------------------------------------------------------

impl FuzzyMatcher for SkimV3Matcher {
    fn fuzzy_match(&self, choice: &str, pattern: &str) -> Option<ScoreType> {
        let result = self.run(choice, pattern, false);
        result.map(|x| x.0)
    }

    fn fuzzy_match_range(&self, choice: &str, pattern: &str) -> Option<(ScoreType, usize, usize)> {
        self.run_range(choice, pattern)
    }

    fn fuzzy_indices(&self, choice: &str, pattern: &str) -> Option<(ScoreType, MatchIndices)> {
        self.run(choice, pattern, true)
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
    #[test]
    fn score_and_full_dp_same() {
        let cases = [("dist-workspace.toml", "tst")];
        let m = matcher_typos();
        for (choice, pat) in cases {
            assert_eq!(
                m.fuzzy_indices(choice, pat).map(|(s, _)| s),
                m.fuzzy_match_range(choice, pat).map(|(s, _, _)| s)
            )
        }
    }

    // Verify that fuzzy_match_range returns scores consistent with fuzzy_indices
    // and that begin/end are within the span of the full index list.
    #[test]
    fn range_consistent_with_indices() {
        let cases = [
            ("hello", "hello"),
            ("axbycz", "abc"),
            ("src/reader.rs", "reader"),
            ("FooBar", "fb"),
            ("dist-workspace.toml", "tst"),
        ];
        let matchers = [matcher(), matcher_typos()];
        for m in &matchers {
            for &(choice, pattern) in &cases {
                let range = m.fuzzy_match_range(choice, pattern);
                let full = m.fuzzy_indices(choice, pattern);
                match (range, full) {
                    (None, None) => {}
                    (Some((rs, rb, re)), Some((fs, fidx))) => {
                        assert_eq!(rs, fs, "score mismatch for ({choice}, {pattern})");
                        let fbegin = fidx.first().copied().unwrap_or_default();
                        let fend = fidx.last().copied().unwrap_or_default();
                        assert_eq!(
                            rb, fbegin,
                            "begin mismatch for ({choice}, {pattern}): range={rb} indices={fbegin}"
                        );
                        assert_eq!(
                            re, fend,
                            "end mismatch for ({choice}, {pattern}): range={re} indices={fend}"
                        );
                    }
                    _ => panic!("range/indices disagreement for ({choice}, {pattern})"),
                }
            }
        }
    }

    // Temporary debug test to reproduce the mismatch between full_dp and
    // score-only on the failing case. Prints the two scores so we can inspect
    // differences while iterating. Remove or disable once the root cause is
    // fixed.
    #[test]
    fn debug_score_vs_full() {
        let choice = "dist-workspace.toml";
        let pat = "tst";
        let m = matcher_typos();
        let full_idx = m.fuzzy_indices(choice, pat);
        let full_score = full_idx.as_ref().map(|(s, _)| *s);
        let score_range = m.fuzzy_match_range(choice, pat);
        let score_only_score = score_range.map(|(s, _, _)| s);
        println!("full_idx: {:?}, score_range: {:?}", full_idx, score_range);
        if let Some((_, idx)) = full_idx.as_ref() {
            println!("full indices: {:?}", idx);
        }
        if let Some((s, b, e)) = score_range {
            println!("score_only range: score={}, begin={}, end={}", s, b, e);
        }
        // keep the assertion to reflect intended equality of scores
        assert_eq!(full_score, score_only_score);
    }
}
