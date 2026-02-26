//! Banding utils
//! Banding is the process of calculating the pertinent parts of the matrix to our specific
//! computation to avoid computing every cell

use super::atom::Atom;
use super::constants::*;
use super::helpers::{compute_last_match_cols, compute_row_col_bounds, find_first_char};

/// Precomputed banding information shared by both score-only and full DP.
pub(super) struct BandingInfo {
    /// Per-row column bounds (only present in exact mode).
    pub(super) row_bounds: Option<([usize; MAX_PAT_LEN], [usize; MAX_PAT_LEN])>,
    /// 1-indexed column of the first match of `pat[0]` in `cho`.
    pub(super) j_first: usize,
    /// Bandwidth for typo-mode diagonal banding (0 in exact mode).
    pub(super) bandwidth: usize,
    /// Minimum number of true (non-substitution) matches to accept.
    pub(super) min_true_matches: usize,
}

/// Compute banding information for the DP. Returns `None` if the pattern
/// cannot possibly match (e.g. a pattern character has no occurrence).
pub(super) fn compute_banding<const ALLOW_TYPOS: bool, C: Atom>(
    pat: &[C],
    cho: &[C],
    respect_case: bool,
) -> Option<BandingInfo> {
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

/// Row-major V-shaped band: compute column bounds at row `i`.
///
/// The result is an upper triangle starting at the diagonal (j ~ i + j_first - 1)
#[inline(always)]
pub(super) fn typo_vband_row(i: usize, m: usize, bandwidth: usize, j_first: usize) -> (usize, usize) {
    let j = i + j_first - 1;
    let lo = j.saturating_sub(bandwidth).max(j_first);

    (lo, m)
}

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
