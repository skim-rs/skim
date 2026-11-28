//! Arinae's algo itself

use std::cell::RefCell;

use thread_local::ThreadLocal;

use crate::fuzzy_matcher::{IndexType, MatchIndices};

use super::banding::{BandingInfo, typo_vband_row};
use super::constants::{
    CONSECUTIVE_BONUS, GAP_EXTEND, GAP_OPEN, MATCH_BONUS, MAX_PAT_LEN, MISMATCH_PENALTY, TYPO_PENALTY,
};
use super::matrix::RankRows;
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
#[allow(clippy::fn_params_excessive_bools)]
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
    let bonus = (bonus_j + CONSECUTIVE_BONUS * Score::from(diag_was_diag)) * (1 + Score::from(is_first));

    // --- DIAGONAL (branchless) ---
    // Match path: diag_score + MATCH_BONUS + bonus, masked by is_match.
    // Mismatch path (typos only): diag_score - MISMATCH_PENALTY, masked by !is_match.
    let match_val = (diag_score + MATCH_BONUS + bonus) * Score::from(is_match);
    let mismatch_val = if ALLOW_TYPOS {
        (diag_score - MISMATCH_PENALTY) * Score::from(!is_match)
    } else {
        0
    };
    let diag_val = match_val + mismatch_val;

    // --- UP (skip pattern char, typos only — const-generic elides entirely) ---
    let up_val = if ALLOW_TYPOS { up_score - TYPO_PENALTY } else { 0 };

    // --- LEFT (skip choice char, branchless gap penalty) ---
    // GAP_OPEN when left_was_diag, GAP_EXTEND otherwise.
    // pen = GAP_EXTEND + (GAP_OPEN - GAP_EXTEND) * left_was_diag
    let left_val = left_score - (GAP_EXTEND + (GAP_OPEN - GAP_EXTEND) * Score::from(left_was_diag));

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
    let dir_bits: u8 = Dir::Left as u8 - u8::from(up_wins) - u8::from(diag_wins) * 2;
    // If best <= 0, force Dir::None (0) — achieved by ANDing with all-zeros.
    let positive = best > 0;
    // When positive: dir_bits; when not: 0 (Dir::None).
    let dir_val = dir_bits & u8::from(positive).wrapping_neg();

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
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
pub(super) fn full_dp<const ALLOW_TYPOS: bool, const COMPUTE_INDICES: bool, C: Atom>(
    cho: &[C],
    pat: &[C],
    bonuses: &[Score],
    respect_case: bool,
    full_buf: &ThreadLocal<RefCell<SWMatrix>>,
    indices_buf: &ThreadLocal<RefCell<MatchIndices>>,
    use_last_match: bool,
    banding: &BandingInfo,
) -> Option<(Score, MatchIndices)> {
    let n = pat.len();
    let m = cho.len();

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
    let (row_lo_arr, row_hi_arr) = if ALLOW_TYPOS {
        ([0usize; MAX_PAT_LEN], [0usize; MAX_PAT_LEN])
    } else {
        let (lo, hi) = banding.row_bounds.as_ref().unwrap();
        (*lo, *hi)
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
                    let next_mat_lo = nj_lo - col_off;
                    let next_mat_hi = (nj_hi - col_off).min(mcols - 1);
                    // Diag reads jm-1, Up reads jm → need [next_mat_lo-1 .. next_mat_hi].
                    let zero_lo = next_mat_lo.saturating_sub(1);
                    let zero_hi = next_mat_hi.min(mcols - 1);
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
        let mat_col_lo = j_lo - col_off;
        let mat_col_hi = j_hi - col_off;
        let jm_max = mcols - 1; // last valid matrix column

        // Zero only the boundary cells that Diag/Left/Up moves will read:
        // - Cell at mat_col_lo-1: read by Left at mat_col_lo and Diag from next row.
        // - Cell at mat_col_hi+1: read by Up from next row at mat_col_hi+1 (if in next band).
        // SAFETY: indices are within the row's allocation.
        unsafe {
            let row_ptr = base_ptr.add(i * cols);
            if mat_col_lo > 1 {
                *row_ptr.add(mat_col_lo - 1) = CELL_ZERO;
            }
            if mat_col_hi < jm_max {
                *row_ptr.add(mat_col_hi + 1) = CELL_ZERO;
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
        let (last_j_lo_raw, last_j_hi) = if ALLOW_TYPOS {
            typo_vband_row(n, m, banding.bandwidth, banding.j_first)
        } else {
            (row_lo_arr[n - 1], row_hi_arr[n - 1])
        };
        let last_j_lo = last_j_lo_raw.max(j_start);
        let last_row_ptr = unsafe { base_ptr.add(n * cols) };
        if use_last_match {
            // Iterate left-to-right with `>=` so that every column tying the
            // running best overwrites best_j — the last (rightmost) tie wins.
            for j in last_j_lo..=last_j_hi {
                let jm = j - col_off;
                let s = unsafe { (*last_row_ptr.add(jm)).score() };
                let better = s >= best_score && s > 0;
                best_score = if better { s } else { best_score };
                best_j = if better { j } else { best_j };
            }
        } else {
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
            let cell_val = unsafe { *base_ptr.add(i * cols).add(jm) };
            match cell_val.dir() {
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
// Range DP — two rolling score rows + a direction matrix
// ---------------------------------------------------------------------------

/// Same DP and same result as `full_dp` + traceback, but faster: it keeps
/// only two rows of scores (the left neighbour stays in a register), and only
/// the 1-byte directions in a matrix for the traceback. Used by
/// `fuzzy_match_range` and `fuzzy_match`, i.e. for every item.
///
/// It computes the band of `full_dp` (V-band), made tighter only where the
/// skipped cells are provably zero or never read (exact mode only):
/// - left: in exact mode no cell is negative, and a cell is zero until the
///   first occurrence of its pattern char in the band;
/// - right: row `i` is only read by row `i + 1` up to its own right bound - 1,
///   and the last row is only scanned up to `row_hi[n - 1]`.
///
/// Cells outside a row's band are zero (`Dir::None`), as in `full_dp`.
#[allow(clippy::too_many_lines)]
pub(super) fn range_dp<const ALLOW_TYPOS: bool, C: Atom>(
    cho: &[C],
    pat: &[C],
    bonuses: &[Score],
    respect_case: bool,
    rows_buf: &ThreadLocal<RefCell<RankRows>>,
    use_last_match: bool,
    banding: &BandingInfo,
) -> Option<(Score, usize, usize)> {
    let n = pat.len();
    let m = cho.len();

    let j_start = banding.j_first;
    let col_off = j_start - 1;
    let mcols = m - col_off + 1;

    let mut buf = rows_buf.get_or(|| RefCell::new(RankRows::default())).borrow_mut();
    let RankRows {
        rows: [row_a, row_b],
        dirs,
        bands,
    } = &mut *buf;
    // The score rows are all zero between calls (we zero what we wrote at the
    // end): this is row 0 and the left wall (column 0). Per-call setup cost
    // matters, most choices are short.
    for row in [&mut *row_a, &mut *row_b] {
        if row.len() < mcols {
            row.resize(mcols, CELL_ZERO);
        }
    }
    // Directions are only read inside the bands, no need to clear them.
    if dirs.len() < n * mcols {
        dirs.resize(n * mcols, 0);
    }
    bands.clear();

    let mut prev: &mut [Cell] = &mut row_a[..mcols];
    let mut cur: &mut [Cell] = &mut row_b[..mcols];
    // Matrix column range written in each buffer, to zero it before reuse.
    let mut prev_written = (1usize, 0usize);
    let mut cur_written = (1usize, 0usize);

    // Bounds of the last-row scan (same as `full_dp`).
    let (last_j_lo, last_j_hi) = if ALLOW_TYPOS {
        typo_vband_row(n, m, banding.bandwidth, banding.j_first)
    } else {
        let (lo, hi) = banding.row_bounds.as_ref().unwrap();
        (lo[n - 1], hi[n - 1])
    };

    for i in 1..=n {
        let pi = pat[i - 1];
        let is_first = i == 1;

        let (j_lo, _) = typo_vband_row(i, m, banding.bandwidth, banding.j_first);
        // Right bound, see the doc comment.
        let j_hi = if ALLOW_TYPOS {
            m
        } else {
            last_j_hi.min(m).saturating_sub(n - i)
        };
        let j_lo = if ALLOW_TYPOS || j_lo > m {
            j_lo
        } else {
            // No occurrence: the whole row is zero.
            pi.find_first_in(&cho[j_lo - 1..], respect_case)
                .map_or(m + 1, |k| j_lo + k)
        };
        let computed = j_lo <= j_hi && j_lo <= m;

        // Row i-2 used this buffer: zero what it wrote outside this row's band,
        // so every cell outside the band reads as zero.
        let (new_lo, new_hi) = if computed {
            (j_lo - col_off, j_hi - col_off)
        } else {
            (1, 0)
        };
        let (old_lo, old_hi) = cur_written;
        if new_lo > new_hi {
            if old_lo <= old_hi {
                cur[old_lo..=old_hi].fill(CELL_ZERO);
            }
        } else {
            let below_hi = old_hi.min(new_lo - 1);
            if old_lo <= below_hi {
                cur[old_lo..=below_hi].fill(CELL_ZERO);
            }
            let above_lo = old_lo.max(new_hi + 1);
            if above_lo <= old_hi {
                cur[above_lo..=old_hi].fill(CELL_ZERO);
            }
        }
        cur_written = (new_lo, new_hi);
        bands.push((new_lo, new_hi));

        if computed {
            let dir_row = &mut dirs[(i - 1) * mcols..i * mcols];
            assert!(j_hi <= m && j_lo > col_off && bonuses.len() >= m);
            let prev_ptr = prev.as_ptr();
            let cur_ptr = cur.as_mut_ptr();
            let dir_ptr = dir_row.as_mut_ptr();
            let cho_ptr = cho.as_ptr();
            let bonuses_ptr = bonuses.as_ptr();
            // SAFETY (all raw accesses in this loop): col_off < j_lo <= j <= j_hi <= m,
            // so 1 <= jm <= m - col_off = mcols - 1 (rows and dir_row have mcols
            // cells), and j - 1 < m = cho.len() <= bonuses.len().
            // The left neighbour stays in registers (no store-to-load latency).
            let left = unsafe { *cur_ptr.add(j_lo - col_off - 1) };
            let mut left_score = left.score();
            let mut left_is_diag = left.is_diag();
            for j in j_lo..=j_hi {
                let jm = j - col_off;
                let is_match = pi.eq(unsafe { *cho_ptr.add(j - 1) }, respect_case);
                let diag = unsafe { *prev_ptr.add(jm - 1) };
                let up_score = if ALLOW_TYPOS {
                    unsafe { (*prev_ptr.add(jm)).score() }
                } else {
                    0
                };

                let (best, dir) = compute_cell::<ALLOW_TYPOS>(
                    is_match,
                    is_first,
                    unsafe { *bonuses_ptr.add(j - 1) },
                    diag.score(),
                    diag.is_diag(),
                    up_score,
                    left_score,
                    left_is_diag,
                );

                unsafe {
                    *cur_ptr.add(jm) = Cell::new(best, dir);
                    *dir_ptr.add(jm) = dir as u8;
                }
                left_score = best;
                left_is_diag = dir == Dir::Diag;
            }
        }

        std::mem::swap(&mut prev, &mut cur);
        std::mem::swap(&mut prev_written, &mut cur_written);
    }

    // Find best score in the last row (now in `prev`), same scan as `full_dp`.
    let mut best_score: Score = 0;
    let mut best_j = 0usize;
    let last_j_lo = last_j_lo.max(j_start);
    if last_j_lo <= last_j_hi && last_j_lo <= m {
        for j in last_j_lo..=last_j_hi {
            let s = prev[j - col_off].score();
            // When use_last_match: `>=` keeps overwriting best_j so the
            // rightmost tied column wins. Otherwise strict `>` means the
            // leftmost (first) column with the best score wins.
            let better = if use_last_match {
                s >= best_score && s > 0
            } else {
                s > best_score
            };
            best_score = if better { s } else { best_score };
            best_j = if better { j } else { best_j };
        }
    }

    // Restore the all-zero rows for the next call.
    for (row, (lo, hi)) in [(prev, prev_written), (cur, cur_written)] {
        if lo <= hi {
            row[lo..=hi].fill(CELL_ZERO);
        }
    }

    if best_score <= 0 {
        return None;
    }

    // Traceback of `full_dp`, on the directions only. A cell outside its
    // row's band has no direction.
    let mut i = n;
    let mut j = best_j;
    let mut matched = TracebackMatches::default();
    while i > 0 && j >= j_start {
        let jm = j - col_off;
        let (lo, hi) = bands[i - 1];
        if jm < lo || jm > hi {
            break;
        }
        match dirs[(i - 1) * mcols + jm] {
            1 => {
                // Dir::Diag
                if pat[i - 1].eq(cho[j - 1], respect_case) {
                    matched.push(j - 1);
                }
                i -= 1;
                j -= 1;
            }
            2 => i -= 1, // Dir::Up
            3 => j -= 1, // Dir::Left
            _ => break,  // Dir::None
        }
    }
    matched.finish(best_score, banding.min_true_matches)
}

/// What `full_dp`'s traceback records, without the index list: the number of
/// true matches and the first / last matched char index.
#[derive(Default)]
pub(super) struct TracebackMatches {
    count: usize,
    first: usize,
    last: usize,
}

impl TracebackMatches {
    /// Records a true match at 0-indexed column `col` (the traceback walks
    /// right to left).
    #[inline]
    pub(super) fn push(&mut self, col: usize) {
        if self.count == 0 {
            self.last = col;
        }
        self.first = col;
        self.count += 1;
    }

    /// `(score, begin, end)` as `fuzzy_indices` + first/last index would give
    /// (0 for an empty index list), or `None` with too few true matches.
    pub(super) fn finish(self, score: Score, min_true_matches: usize) -> Option<(Score, usize, usize)> {
        (self.count >= min_true_matches).then_some((score, self.first, self.last))
    }
}
