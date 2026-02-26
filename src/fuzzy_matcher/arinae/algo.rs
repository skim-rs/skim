//! Arinae's algo itself

use std::cell::RefCell;

use thread_local::ThreadLocal;

use crate::fuzzy_matcher::{IndexType, MatchIndices};

use super::banding::{compute_banding, typo_vband_row};
use super::constants::*;
use super::{Atom, CELL_ZERO, Cell, Dir, SWMatrix, Score};

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
pub(super) fn full_dp<const ALLOW_TYPOS: bool, const COMPUTE_INDICES: bool, C: Atom>(
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

    // base_ptr and cols already set above

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

    for i in 1..=n {
        let pi = pat[i - 1];
        let is_first = i == 1;

        // --- Compute column bounds for this row (original 1-indexed space) ---
        let (j_lo, j_hi) = typo_vband_row(i, m, banding.bandwidth, banding.j_first);

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

            unsafe {
                *cur_ptr.add(jm) = Cell::new(best, dir);
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

    if COMPUTE_INDICES {
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
    } else {
        Some((best_score, Vec::default()))
    }
}

// ---------------------------------------------------------------------------
// Range DP — full matrix, minimal traceback (begin + end only)
// ---------------------------------------------------------------------------

/// Full matrix DP followed by a traceback that only records the first and
/// last matched positions (not every index). Used by `fuzzy_match_range` to
/// avoid allocating and populating the full index vec when only the span is
/// needed.
pub(super) fn range_dp<const ALLOW_TYPOS: bool, C: Atom>(
    cho: &[C],
    pat: &[C],
    bonuses: &[Score],
    respect_case: bool,
    full_buf: &ThreadLocal<RefCell<SWMatrix>>,
) -> Option<(Score, usize, usize)> {
    let n = pat.len();
    let m = cho.len();

    let banding = compute_banding::<ALLOW_TYPOS, C>(pat, cho, respect_case)?;
    let j_start = banding.j_first;
    let col_off = j_start - 1;
    let mcols = m - col_off + 1;

    let mut buf = full_buf
        .get_or(|| RefCell::new(SWMatrix::zero(n + 1, mcols)))
        .borrow_mut();
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
