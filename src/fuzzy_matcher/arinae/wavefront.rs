//! Anti-diagonal ("wavefront") version of the range DP, for ASCII patterns of
//! at most 8 chars on `x86_64` (SSE2 is part of the `x86_64` baseline).
//!
//! Cell `(i, j)` only depends on `(i-1, j-1)`, `(i-1, j)` and `(i, j-1)`, so all
//! the cells of an anti-diagonal (`i + j` constant) are independent. With one
//! `i16` lane per pattern row, one SSE2 step computes a whole anti-diagonal:
//! about `m + n` steps instead of `n * m` cells. Each lane does exactly the
//! arithmetic of `compute_cell` (wrapping `i16` ops, like a release build) on
//! the band of `full_dp`, so the result is the same as `full_dp` + traceback.

#![cfg(target_arch = "x86_64")]

use std::arch::x86_64::{
    __m128i, _mm_add_epi16, _mm_and_si128, _mm_andnot_si128, _mm_cmpeq_epi16, _mm_cmpgt_epi16, _mm_cvtsi32_si128,
    _mm_max_epi16, _mm_or_si128, _mm_packus_epi16, _mm_set1_epi16, _mm_setr_epi16, _mm_setzero_si128, _mm_slli_si128,
    _mm_storel_epi64, _mm_storeu_si128, _mm_sub_epi16, _mm_xor_si128,
};
use std::cell::RefCell;

use thread_local::ThreadLocal;

use super::Score;
use super::algo::TracebackMatches;
use super::banding::{BandingInfo, typo_vband_row};
use super::constants::{CONSECUTIVE_BONUS, GAP_EXTEND, GAP_OPEN, MATCH_BONUS, MISMATCH_PENALTY, TYPO_PENALTY};

/// Number of lanes, i.e. the longest pattern this kernel handles.
pub(super) const LANES: usize = 8;

/// Scratch buffers: directions per (anti-diagonal, lane), last-row scores,
/// and the case-folded choice.
#[derive(Default, Debug)]
pub(super) struct WaveBufs {
    dirs: Vec<u8>,
    last_row: Vec<Score>,
    folded: Vec<u8>,
}

/// Range DP on anti-diagonals. Needs `1 <= pat.len() <= LANES`. Returns
/// `(score, begin, end)` like `range_dp`, or `Err(())` when the choice is too
/// long for `i16` column indices (the caller then uses `range_dp`).
#[allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::result_unit_err,
    clippy::many_single_char_names
)]
pub(super) fn wavefront_range_dp<const ALLOW_TYPOS: bool>(
    cho: &[u8],
    pat: &[u8],
    bonuses: &[Score],
    respect_case: bool,
    bufs: &ThreadLocal<RefCell<WaveBufs>>,
    use_last_match: bool,
    banding: &BandingInfo,
) -> Result<Option<(Score, usize, usize)>, ()> {
    let n = pat.len();
    let m = cho.len();
    assert!((1..=LANES).contains(&n) && bonuses.len() >= m);

    let j_start = banding.j_first;
    let col_off = j_start - 1;
    // Matrix columns jm = 1..=cols are choice columns j = jm + col_off.
    let cols = m - col_off;
    let steps = n + cols + 1; // anti-diagonals d = i + jm, from 2 to n + cols
    // Lane column indices are i16.
    i16::try_from(steps).map_err(|_| ())?;
    let cols_i16 = i16::try_from(cols).map_err(|_| ())?;

    let mut guard = bufs.get_or(|| RefCell::new(WaveBufs::default())).borrow_mut();
    let WaveBufs { dirs, last_row, folded } = &mut *guard;
    dirs.clear();
    dirs.resize(steps * LANES, 0);
    last_row.clear();
    last_row.resize(cols + 1, 0);
    let window = &cho[col_off..];
    let window: &[u8] = if respect_case {
        window
    } else {
        folded.clear();
        folded.extend(window.iter().map(u8::to_ascii_lowercase));
        folded
    };
    let bonus_window = &bonuses[col_off..m];

    // Per-lane constants. Lane l is pattern row i = l + 1.
    let mut pat_lanes = [-1i16; LANES]; // -1 never equals a byte
    for (l, &p) in pat.iter().enumerate() {
        pat_lanes[l] = i16::from(if respect_case { p } else { p.to_ascii_lowercase() });
    }
    // In exact mode the band of row i starts at jm = i (V-band of full_dp);
    // in typo mode at jm = 1 (the band is wider than the pattern).
    let lane_lo: [i16; LANES] = if ALLOW_TYPOS {
        [1; LANES]
    } else {
        [1, 2, 3, 4, 5, 6, 7, 8]
    };

    // SAFETY: SSE2 is always available on x86_64. All loads/stores below are
    // on in-bounds slices (`dirs` has `steps * LANES` bytes, `out` 8 lanes).
    unsafe {
        let p = |a: [i16; LANES]| _mm_setr_epi16(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]);
        let pat_v = p(pat_lanes);
        let lo_minus_1 = _mm_sub_epi16(p(lane_lo), _mm_set1_epi16(1));
        let cols_plus_1 = _mm_set1_epi16(cols_i16 + 1);
        // jm of each lane is (d - 1) - l.
        let lane_idx = _mm_setr_epi16(0, 1, 2, 3, 4, 5, 6, 7);
        let lane0 = _mm_setr_epi16(-1, 0, 0, 0, 0, 0, 0, 0);
        let ones = _mm_set1_epi16(-1);
        let zero = _mm_setzero_si128();
        let consecutive = _mm_set1_epi16(CONSECUTIVE_BONUS);
        let match_bonus = _mm_set1_epi16(MATCH_BONUS);
        let mismatch = _mm_set1_epi16(MISMATCH_PENALTY);
        let typo = _mm_set1_epi16(TYPO_PENALTY);
        let gap_extend = _mm_set1_epi16(GAP_EXTEND);
        let gap_extra = _mm_set1_epi16(GAP_OPEN - GAP_EXTEND);
        let one = _mm_set1_epi16(1);
        let two = _mm_set1_epi16(2);
        let three = _mm_set1_epi16(3);

        // Rolling anti-diagonals d-1 and d-2: scores and "dir is Diag" masks.
        let (mut s1, mut g1, mut s2, mut g2) = (zero, zero, zero, zero);
        // Sliding windows: lane l holds the choice char / bonus at jm = d - 1 - l.
        let (mut chars, mut bon) = (zero, zero);
        let mut out = [0i16; LANES];
        let last_lane = n - 1;

        for d in 2..steps {
            // Lane 0 gets jm = d - 1; past the end it gets junk (out of band).
            let new_jm = d - 1;
            let (c, b) = if new_jm <= cols {
                (i32::from(window[new_jm - 1]), i32::from(bonus_window[new_jm - 1]))
            } else {
                (-1, 0)
            };
            chars = _mm_or_si128(_mm_slli_si128(chars, 2), _mm_and_si128(_mm_cvtsi32_si128(c), lane0));
            bon = _mm_or_si128(_mm_slli_si128(bon, 2), _mm_and_si128(_mm_cvtsi32_si128(b), lane0));
            // Row 0 is zero: lane 0 reads zeros from "lane -1".
            let diag = _mm_slli_si128(s2, 2);
            let diag_was_diag = _mm_slli_si128(g2, 2);
            let up = _mm_slli_si128(s1, 2);
            let left = s1;
            let left_was_diag = g1;

            // Truncation is fine: d < steps fits in i16 (checked above).
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let jm = _mm_sub_epi16(_mm_set1_epi16((d - 1) as i16), lane_idx);
            let in_band = _mm_and_si128(_mm_cmpgt_epi16(jm, lo_minus_1), _mm_cmpgt_epi16(cols_plus_1, jm));
            let is_match = _mm_cmpeq_epi16(chars, pat_v);

            // --- Same arithmetic as `compute_cell` ---
            // bonus = (bonus_j + CONSECUTIVE * diag_was_diag) * (1 + is_first)
            let bonus = _mm_add_epi16(bon, _mm_and_si128(consecutive, diag_was_diag));
            let bonus = _mm_add_epi16(bonus, _mm_and_si128(bonus, lane0));
            let match_val = _mm_and_si128(_mm_add_epi16(_mm_add_epi16(diag, match_bonus), bonus), is_match);
            let diag_val = if ALLOW_TYPOS {
                _mm_or_si128(match_val, _mm_andnot_si128(is_match, _mm_sub_epi16(diag, mismatch)))
            } else {
                match_val
            };
            let up_val = if ALLOW_TYPOS { _mm_sub_epi16(up, typo) } else { zero };
            let left_val = _mm_sub_epi16(left, _mm_add_epi16(gap_extend, _mm_and_si128(gap_extra, left_was_diag)));
            let best = _mm_max_epi16(_mm_max_epi16(diag_val, up_val), left_val);
            // a >= b  <=>  !(b > a)
            let ge = |a: __m128i, b: __m128i| _mm_xor_si128(_mm_cmpgt_epi16(b, a), ones);
            let diag_wins = if ALLOW_TYPOS {
                _mm_and_si128(ge(diag_val, up_val), ge(diag_val, left_val))
            } else {
                _mm_and_si128(is_match, ge(diag_val, left_val))
            };
            let up_wins = if ALLOW_TYPOS {
                _mm_andnot_si128(diag_wins, ge(up_val, left_val))
            } else {
                zero
            };
            // dir = Left(3) - up_wins - 2 * diag_wins; None (0) unless best > 0.
            let dir = _mm_sub_epi16(
                _mm_sub_epi16(three, _mm_and_si128(up_wins, one)),
                _mm_and_si128(diag_wins, two),
            );
            let keep = _mm_and_si128(_mm_cmpgt_epi16(best, zero), in_band);
            let dir = _mm_and_si128(dir, keep);
            // Cells outside the band are zero with no direction, as in full_dp.
            let s0 = _mm_and_si128(best, in_band);
            let g0 = _mm_cmpeq_epi16(dir, one);

            // Both stores are unaligned (`storel` / `storeu`), the casts are fine.
            #[allow(clippy::cast_ptr_alignment)]
            _mm_storel_epi64(
                dirs.as_mut_ptr().add(d * LANES).cast::<__m128i>(),
                _mm_packus_epi16(dir, dir),
            );
            // Lane n - 1 is the last row, at jm = d - n.
            if let Some(jm) = d.checked_sub(n)
                && (1..=cols).contains(&jm)
            {
                #[allow(clippy::cast_ptr_alignment)]
                _mm_storeu_si128(out.as_mut_ptr().cast::<__m128i>(), s0);
                last_row[jm] = out[last_lane];
            }

            s2 = s1;
            g2 = g1;
            s1 = s0;
            g1 = g0;
        }
    }

    // Find best score in the last row, same scan as `full_dp`.
    let (last_j_lo, last_j_hi) = if ALLOW_TYPOS {
        typo_vband_row(n, m, banding.bandwidth, banding.j_first)
    } else {
        let (lo, hi) = banding.row_bounds.as_ref().ok_or(())?;
        (lo[n - 1], hi[n - 1])
    };
    let last_j_lo = last_j_lo.max(j_start);
    let mut best_score: Score = 0;
    let mut best_j = 0usize;
    if last_j_lo <= last_j_hi && last_j_lo <= m {
        for j in last_j_lo..=last_j_hi {
            let s = last_row[j - col_off];
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
    if best_score <= 0 {
        return Ok(None);
    }

    // Traceback of `full_dp` on the directions.
    let mut i = n;
    let mut j = best_j;
    let mut matched = TracebackMatches::default();
    while i > 0 && j >= j_start {
        let jm = j - col_off;
        match dirs[(i + jm) * LANES + i - 1] {
            1 => {
                // Dir::Diag
                let (a, b) = (pat[i - 1], cho[j - 1]);
                if a == b || (!respect_case && a.eq_ignore_ascii_case(&b)) {
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
    Ok(matched.finish(best_score, banding.min_true_matches))
}
