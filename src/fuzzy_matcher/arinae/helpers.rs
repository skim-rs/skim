//! &[dyn Atom] manipulation helpers

use super::Atom;
use super::constants::*;

/// Find the 1-indexed column of the first occurrence of `pat[0]` in `cho`.
///
/// Returns `None` if `pat[0]` is not found anywhere (caller should return
/// `None`). The position defines the start of the V-shaped banding envelope.
/// Uses SIMD-backed `find_first_in` for `u8` slices.
#[inline]
pub(super) fn find_first_char<C: Atom>(pat: &[C], cho: &[C], respect_case: bool) -> Option<usize> {
    pat[0].find_first_in(cho, respect_case).map(|idx| idx + 1) // 1-indexed
}

/// Compute the last column (1-indexed) at which each pattern character can be
/// matched, scanning from the end. Used to tighten the diagonal upper bound.
pub(super) fn compute_last_match_cols<C: Atom>(
    pat: &[C],
    cho: &[C],
    respect_case: bool,
) -> Option<[usize; MAX_PAT_LEN]> {
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
pub(super) fn compute_row_col_bounds(
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
