//! Fuzzy matching algorithm based on fzy by John Hawthorn.
//! https://github.com/jhawthorn/fzy
//!
//! This implements fzy's scoring algorithm which treats fuzzy matching as a
//! modified edit-distance problem using dynamic programming (Needleman-Wunsch style).
//! It uses two DP matrices:
//! - `M[i][j]`: the best possible score using the first `i` chars of the needle
//!   and the first `j` chars of the haystack.
//! - `D[i][j]`: the best score that *ends with a match* at position `(i, j)`.
//!
//! This separation enables affine gap penalties: a constant cost to open a gap
//! and a linear cost for extending it, plus a bonus for consecutive matches.
//!
//! All scoring uses integer arithmetic with a ×200 scaling factor for performance.
//! The original fzy float constants map as follows:
//! - -0.005 → -1
//! - -0.01  → -2
//! - 0.6    → 120
//! - 0.7    → 140
//! - 0.8    → 160
//! - 0.9    → 180
//! - 1.0    → 200
//! - -1.5   → -300
//!
//! # Example:
//! ```
//! use skim::fuzzy_matcher::FuzzyMatcher;
//! use skim::fuzzy_matcher::fzy::FzyMatcher;
//!
//! let matcher = FzyMatcher::default();
//!
//! assert_eq!(None, matcher.fuzzy_match("abc", "abx"));
//! assert!(matcher.fuzzy_match("axbycz", "abc").is_some());
//!
//! let (score, indices) = matcher.fuzzy_indices("axbycz", "abc").unwrap();
//! assert_eq!(indices, [0, 2, 4]);
//! ```

use std::cell::RefCell;

use thread_local::ThreadLocal;

use crate::fuzzy_matcher::util::{char_equal, cheap_matches};
use crate::fuzzy_matcher::{FuzzyMatcher, IndexType, MatchIndices, ScoreType};

// ---------------------------------------------------------------------------
// Score constants (from fzy's config.def.h, scaled ×200 to integer)
// ---------------------------------------------------------------------------

/// Sentinel for "impossible" / uninitialized DP cells.
/// Uses `i64::MIN / 2` so that adding a penalty never overflows.
const SCORE_MIN: i64 = i64::MIN / 2;

/// Score for an exact-length match (`needle.len()` == `haystack.len()`).
const SCORE_MAX: i64 = i64::MAX / 2;

const SCORE_GAP_LEADING: i64 = -1; // -0.005 × 200
const SCORE_GAP_TRAILING: i64 = -1; // -0.005 × 200
const SCORE_GAP_INNER: i64 = -2; // -0.01  × 200

const SCORE_MATCH_CONSECUTIVE: i64 = 200; // 1.0 × 200
const SCORE_MATCH_SLASH: i64 = 180; // 0.9 × 200
const SCORE_MATCH_WORD: i64 = 160; // 0.8 × 200
const SCORE_MATCH_CAPITAL: i64 = 140; // 0.7 × 200
const SCORE_MATCH_DOT: i64 = 120; // 0.6 × 200

/// Penalty applied when a typo is used (substitution or needle-char deletion).
const SCORE_TYPO: i64 = -300; // -1.5 × 200

/// Maximum haystack length we will score.
const MATCH_MAX_LEN: usize = 1024;

/// Conversion factor from internal ×200 scores to skim's ×1000 convention.
/// `internal_score` × `SCORE_TO_SKIM` = `skim_score`
const SCORE_TO_SKIM: i64 = 5; // 1000 / 200

// ---------------------------------------------------------------------------
// Bonus computation
// ---------------------------------------------------------------------------

#[inline]
fn bonus_index(ch: char) -> usize {
    if ch.is_ascii_uppercase() {
        2
    } else {
        usize::from(ch.is_ascii_lowercase() || ch.is_ascii_digit())
    }
}

#[inline]
fn compute_bonus(prev_ch: char, ch: char) -> i64 {
    match bonus_index(ch) {
        0 => 0,
        1 => match prev_ch {
            '/' => SCORE_MATCH_SLASH,
            '-' | '_' | ' ' => SCORE_MATCH_WORD,
            '.' => SCORE_MATCH_DOT,
            _ => 0,
        },
        2 => match prev_ch {
            '/' => SCORE_MATCH_SLASH,
            '-' | '_' | ' ' => SCORE_MATCH_WORD,
            '.' => SCORE_MATCH_DOT,
            c if c.is_ascii_lowercase() => SCORE_MATCH_CAPITAL,
            _ => 0,
        },
        _ => unreachable!(),
    }
}

// ---------------------------------------------------------------------------
// Core DP matching (no typos)
// ---------------------------------------------------------------------------

fn precompute_bonus(haystack: &[char]) -> Vec<i64> {
    let mut bonuses = Vec::with_capacity(haystack.len());
    let mut prev = '/';
    for &ch in haystack {
        bonuses.push(compute_bonus(prev, ch));
        prev = ch;
    }
    bonuses
}

#[inline]
fn is_match(needle: &[char], haystack: &[char], case_sensitive: bool, i: usize, j: usize) -> bool {
    char_equal(needle[i], haystack[j], case_sensitive)
}

/// Core fzy scoring without typos.
#[allow(clippy::too_many_lines)]
fn fzy_score(
    needle: &[char],
    haystack: &[char],
    case_sensitive: bool,
    positions: Option<&mut Vec<IndexType>>,
) -> Option<i64> {
    let n = needle.len();
    let m = haystack.len();

    if n == 0 || m > MATCH_MAX_LEN || n > m {
        return None;
    }

    if n == m {
        if let Some(pos) = positions {
            pos.clear();
            pos.extend(0..n);
        }
        return Some(SCORE_MAX);
    }

    let match_bonus = precompute_bonus(haystack);

    if positions.is_some() {
        // Full matrix for position recovery
        let mut d_matrix: Vec<Vec<i64>> = vec![vec![SCORE_MIN; m]; n];
        let mut m_matrix: Vec<Vec<i64>> = vec![vec![SCORE_MIN; m]; n];

        // Row 0
        {
            let mut prev_score = SCORE_MIN;
            let gap = if n == 1 { SCORE_GAP_TRAILING } else { SCORE_GAP_INNER };
            for j in 0..m {
                if is_match(needle, haystack, case_sensitive, 0, j) {
                    let score = i64::try_from(j).unwrap_or(i64::MAX) * SCORE_GAP_LEADING + match_bonus[j];
                    d_matrix[0][j] = score;
                    prev_score = score;
                    m_matrix[0][j] = score;
                } else {
                    prev_score += gap;
                    m_matrix[0][j] = prev_score;
                }
            }
        }

        // Rows 1..n-1
        for i in 1..n {
            let mut prev_score = SCORE_MIN;
            let gap_score = if i == n - 1 {
                SCORE_GAP_TRAILING
            } else {
                SCORE_GAP_INNER
            };
            for j in 0..m {
                if is_match(needle, haystack, case_sensitive, i, j) {
                    let mut score = SCORE_MIN;
                    if j > 0 {
                        let prev_m = m_matrix[i - 1][j - 1];
                        let prev_d = d_matrix[i - 1][j - 1];
                        score = i64::max(prev_m + match_bonus[j], prev_d + SCORE_MATCH_CONSECUTIVE);
                    }
                    d_matrix[i][j] = score;
                    prev_score = i64::max(score, prev_score + gap_score);
                    m_matrix[i][j] = prev_score;
                } else {
                    prev_score += gap_score;
                    m_matrix[i][j] = prev_score;
                }
            }
        }

        let final_score = m_matrix[n - 1][m - 1];

        // Backtrace
        if let Some(pos) = positions {
            pos.clear();
            pos.resize(n, 0);
            let mut match_required = false;
            let mut j = m - 1;
            for i in (0..n).rev() {
                loop {
                    if d_matrix[i][j] != SCORE_MIN && (match_required || d_matrix[i][j] == m_matrix[i][j]) {
                        match_required =
                            i > 0 && j > 0 && m_matrix[i][j] == d_matrix[i - 1][j - 1] + SCORE_MATCH_CONSECUTIVE;
                        pos[i] = j;
                        j = j.saturating_sub(1);
                        break;
                    }
                    if j == 0 {
                        break;
                    }
                    j -= 1;
                }
            }
        }

        Some(final_score)
    } else {
        // Score-only: rolling single row
        let mut d_row = vec![SCORE_MIN; m];
        let mut m_row = vec![SCORE_MIN; m];

        for i in 0..n {
            let mut prev_score = SCORE_MIN;
            let gap_score = if i == n - 1 {
                SCORE_GAP_TRAILING
            } else {
                SCORE_GAP_INNER
            };
            let mut prev_d = SCORE_MIN;
            let mut prev_m = SCORE_MIN;

            for j in 0..m {
                let old_d = d_row[j];
                let old_m = m_row[j];

                if is_match(needle, haystack, case_sensitive, i, j) {
                    let score = if i == 0 {
                        i64::try_from(j).unwrap_or(i64::MAX) * SCORE_GAP_LEADING + match_bonus[j]
                    } else if j > 0 {
                        i64::max(prev_m + match_bonus[j], prev_d + SCORE_MATCH_CONSECUTIVE)
                    } else {
                        SCORE_MIN
                    };
                    d_row[j] = score;
                    prev_score = i64::max(score, prev_score + gap_score);
                    m_row[j] = prev_score;
                } else {
                    d_row[j] = SCORE_MIN;
                    prev_score += gap_score;
                    m_row[j] = prev_score;
                }

                prev_d = old_d;
                prev_m = old_m;
            }
        }

        Some(m_row[m - 1])
    }
}

// ---------------------------------------------------------------------------
// Typo-tolerant pre-filter (allocation-free)
// ---------------------------------------------------------------------------

/// Fast subsequence check allowing up to `max_typos` needle characters to be
/// unmatched. Returns `true` if the needle can plausibly match the haystack.
///
/// Uses a greedy forward scan: for each needle char, try to find it in the
/// remaining haystack. If not found, consume a typo. If we exceed `max_typos`,
/// return false.
///
/// This is a heuristic pre-filter — it may return true for some candidates
/// that the full DP will reject, but it will never return false for a valid
/// match. The key property is that it's O(n + m) with zero allocations.
///
/// `lower_pattern` should be the pre-lowercased pattern (avoids repeated
/// lowercasing of the same pattern across calls). Haystack chars are
/// lowercased inline.
#[inline]
fn can_match_with_typos(
    choice: &[char],
    pattern: &[char],
    lower_pattern: &[char],
    case_sensitive: bool,
    max_typos: usize,
) -> bool {
    let n = pattern.len();
    let m = choice.len();

    // Quick length check: we need at least (n - max_typos) haystack chars
    if n > m + max_typos {
        return false;
    }

    let mut typos_used = 0;
    let mut j = 0; // position in haystack

    for i in 0..n {
        // Try to find pattern[i] in remaining haystack
        let saved_j = j;
        let mut found = false;
        while j < m {
            let matches = if case_sensitive {
                pattern[i] == choice[j]
            } else {
                lower_pattern[i] == choice[j].to_ascii_lowercase()
            };
            if matches {
                j += 1;
                found = true;
                break;
            }
            j += 1;
        }
        if !found {
            // Needle char not found — treat as typo (deletion of this needle char).
            // Restore j so that subsequent needle chars can still match
            // remaining haystack characters (needle deletion skips only the
            // needle char, not haystack chars).
            j = saved_j;
            typos_used += 1;
            if typos_used > max_typos {
                return false;
            }
        }
    }

    true
}

// ---------------------------------------------------------------------------
// Typo-tolerant scoring with rolling rows
// ---------------------------------------------------------------------------

/// Thread-local DP buffers to avoid per-call allocation in the typo path.
#[derive(Debug, Default)]
struct TypoDpBuffers {
    /// Per-typo-layer D and M rolling rows: `[t][cur_or_prev][j]`
    /// Flattened as: `[(t_max+1) * 2 * m_cap]` for D and M each.
    d_buf: Vec<i64>,
    m_buf: Vec<i64>,
    /// For the full-matrix path (positions needed), we store full D and M:
    /// `[(t_max+1) * n * m]` each.
    d_full: Vec<i64>,
    m_full: Vec<i64>,
}

/// Score-only typo-tolerant fzy matching using rolling rows.
///
/// For each typo layer `t` we maintain two rows (current and previous needle row)
/// of D and M values. This is `O((t_max+1)` * n * m) time but only
/// `O((t_max+1)` * m) space.
#[allow(clippy::too_many_arguments)]
fn fzy_score_typos_rolling(
    needle: &[char],
    haystack: &[char],
    lower_needle: &[char],
    lower_haystack: &[char],
    match_bonus: &[i64],
    case_sensitive: bool,
    max_typos: usize,
    bufs: &mut TypoDpBuffers,
) -> Option<i64> {
    let n = needle.len();
    let m = haystack.len();
    let t_max = max_typos;

    // We need two rows (prev, cur) per typo layer for both D and M.
    // Layout: [(t_max+1) * 2 * m] — row index is (t * 2 + row_parity) * m + j
    let row_size = m;
    let layer_size = 2 * row_size;
    let total = (t_max + 1) * layer_size;

    bufs.d_buf.clear();
    bufs.d_buf.resize(total, SCORE_MIN);
    bufs.m_buf.clear();
    bufs.m_buf.resize(total, SCORE_MIN);

    let d = &mut bufs.d_buf;
    let m_arr = &mut bufs.m_buf;

    // Index helpers
    let ri = |t: usize, parity: usize, j: usize| -> usize { t * layer_size + parity * row_size + j };

    for i in 0..n {
        let cur = i & 1;
        let prev = 1 - cur;
        let gap_score = if i == n - 1 {
            SCORE_GAP_TRAILING
        } else {
            SCORE_GAP_INNER
        };

        for t in 0..=t_max {
            let mut prev_score = SCORE_MIN;

            for j in 0..m {
                let matched = if case_sensitive {
                    needle[i] == haystack[j]
                } else {
                    lower_needle[i] == lower_haystack[j]
                };

                let mut d_val = SCORE_MIN;

                // --- Exact match ---
                if matched {
                    if i == 0 {
                        d_val = i64::try_from(j).unwrap_or(i64::MAX) * SCORE_GAP_LEADING + match_bonus[j];
                    } else if j > 0 {
                        let pm = m_arr[ri(t, prev, j - 1)];
                        let pd = d[ri(t, prev, j - 1)];
                        if pm != SCORE_MIN {
                            d_val = i64::max(d_val, pm + match_bonus[j]);
                        }
                        if pd != SCORE_MIN {
                            d_val = i64::max(d_val, pd + SCORE_MATCH_CONSECUTIVE);
                        }
                    }
                }

                // --- Substitution typo: consume both needle[i] and haystack[j] ---
                if !matched && t > 0 {
                    if i == 0 {
                        d_val = i64::max(
                            d_val,
                            i64::try_from(j).unwrap_or(i64::MAX) * SCORE_GAP_LEADING + SCORE_TYPO,
                        );
                    } else if j > 0 {
                        let pm = m_arr[ri(t - 1, prev, j - 1)];
                        if pm != SCORE_MIN {
                            d_val = i64::max(d_val, pm + SCORE_TYPO);
                        }
                    }
                }

                d[ri(t, cur, j)] = d_val;

                // --- Compute M from D and gap ---
                if d_val == SCORE_MIN {
                    prev_score += gap_score;
                } else {
                    prev_score = i64::max(d_val, prev_score + gap_score);
                }

                // --- Needle deletion: skip needle[i], use a typo ---
                // M[t][i][j] can come from M[t-1][i-1][j] + SCORE_TYPO
                if t > 0 {
                    let del_from = if i == 0 {
                        // Deleting first needle char
                        i64::try_from(j).unwrap_or(i64::MAX) * SCORE_GAP_LEADING + SCORE_TYPO
                    } else {
                        let pv = m_arr[ri(t - 1, prev, j)];
                        if pv == SCORE_MIN { SCORE_MIN } else { pv + SCORE_TYPO }
                    };
                    if del_from != SCORE_MIN {
                        prev_score = i64::max(prev_score, del_from);
                    }
                }

                m_arr[ri(t, cur, j)] = prev_score;
            }
        }
    }

    // Find best score across all typo layers
    let final_row = (n - 1) & 1;
    let mut best = SCORE_MIN;
    for t in 0..=t_max {
        let s = m_arr[ri(t, final_row, m - 1)];
        if s > best {
            best = s;
        }
    }

    if best == SCORE_MIN { None } else { Some(best) }
}

/// Full-matrix typo-tolerant scoring for position recovery.
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
fn fzy_score_typos_full(
    needle: &[char],
    haystack: &[char],
    lower_needle: &[char],
    lower_haystack: &[char],
    match_bonus: &[i64],
    case_sensitive: bool,
    max_typos: usize,
    positions: &mut Vec<IndexType>,
    bufs: &mut TypoDpBuffers,
) -> Option<i64> {
    let n = needle.len();
    let m = haystack.len();
    let t_max = max_typos;

    let layer_size = n * m;
    let total = (t_max + 1) * layer_size;

    bufs.d_full.clear();
    bufs.d_full.resize(total, SCORE_MIN);
    bufs.m_full.clear();
    bufs.m_full.resize(total, SCORE_MIN);

    let d_flat = &mut bufs.d_full;
    let m_flat = &mut bufs.m_full;

    let idx = |t: usize, i: usize, j: usize| -> usize { t * layer_size + i * m + j };

    // Fill DP
    for t in 0..=t_max {
        for i in 0..n {
            let gap_score = if i == n - 1 {
                SCORE_GAP_TRAILING
            } else {
                SCORE_GAP_INNER
            };
            let mut prev_score = SCORE_MIN;

            for j in 0..m {
                let matched = if case_sensitive {
                    needle[i] == haystack[j]
                } else {
                    lower_needle[i] == lower_haystack[j]
                };

                let mut d_val = SCORE_MIN;

                // Exact match
                if matched {
                    if i == 0 {
                        d_val = i64::try_from(j).unwrap_or(i64::MAX) * SCORE_GAP_LEADING + match_bonus[j];
                    } else if j > 0 {
                        let pm = m_flat[idx(t, i - 1, j - 1)];
                        let pd = d_flat[idx(t, i - 1, j - 1)];
                        if pm != SCORE_MIN {
                            d_val = i64::max(d_val, pm + match_bonus[j]);
                        }
                        if pd != SCORE_MIN {
                            d_val = i64::max(d_val, pd + SCORE_MATCH_CONSECUTIVE);
                        }
                    }
                }

                // Substitution typo
                if !matched && t > 0 {
                    if i == 0 {
                        d_val = i64::max(
                            d_val,
                            i64::try_from(j).unwrap_or(i64::MAX) * SCORE_GAP_LEADING + SCORE_TYPO,
                        );
                    } else if j > 0 {
                        let pm = m_flat[idx(t - 1, i - 1, j - 1)];
                        if pm != SCORE_MIN {
                            d_val = i64::max(d_val, pm + SCORE_TYPO);
                        }
                    }
                }

                d_flat[idx(t, i, j)] = d_val;

                if d_val == SCORE_MIN {
                    prev_score += gap_score;
                } else {
                    prev_score = i64::max(d_val, prev_score + gap_score);
                }

                // Needle deletion
                if t > 0 {
                    let del_from = if i == 0 {
                        i64::try_from(j).unwrap_or(i64::MAX) * SCORE_GAP_LEADING + SCORE_TYPO
                    } else {
                        let pv = m_flat[idx(t - 1, i - 1, j)];
                        if pv == SCORE_MIN { SCORE_MIN } else { pv + SCORE_TYPO }
                    };
                    if del_from != SCORE_MIN {
                        prev_score = i64::max(prev_score, del_from);
                    }
                }

                m_flat[idx(t, i, j)] = prev_score;
            }
        }
    }

    // Find best score and typo layer
    let mut best_score = SCORE_MIN;
    let mut best_t = 0;
    for t in 0..=t_max {
        let s = m_flat[idx(t, n - 1, m - 1)];
        if s > best_score {
            best_score = s;
            best_t = t;
        }
    }

    if best_score == SCORE_MIN {
        return None;
    }

    // Backtrace for positions
    positions.clear();
    let mut cur_t = best_t;
    let mut j = m - 1;
    let mut rev_positions: Vec<Option<usize>> = Vec::with_capacity(n);

    for i in (0..n).rev() {
        loop {
            let cur_m = m_flat[idx(cur_t, i, j)];

            // Check needle deletion (exact integer comparison — no epsilon needed)
            if cur_t > 0 {
                let del_from = if i == 0 {
                    i64::try_from(j).unwrap_or(i64::MAX) * SCORE_GAP_LEADING + SCORE_TYPO
                } else {
                    let pv = m_flat[idx(cur_t - 1, i - 1, j)];
                    if pv == SCORE_MIN { SCORE_MIN } else { pv + SCORE_TYPO }
                };
                if del_from != SCORE_MIN && cur_m == del_from {
                    rev_positions.push(None);
                    cur_t -= 1;
                    break;
                }
            }

            // Check match/substitution at (i, j) (exact integer comparison)
            let d_val = d_flat[idx(cur_t, i, j)];
            if d_val != SCORE_MIN && d_val == cur_m {
                rev_positions.push(Some(j));
                j = j.saturating_sub(1);
                break;
            }

            // Gap — haystack[j] skipped
            if j == 0 {
                rev_positions.push(None);
                break;
            }
            j -= 1;
        }
    }

    rev_positions.reverse();
    for p in rev_positions.iter().flatten() {
        positions.push(*p);
    }

    Some(best_score)
}

// ---------------------------------------------------------------------------
// Score conversion
// ---------------------------------------------------------------------------

/// Convert internal ×200-scaled integer score to skim's `ScoreType` (×1000 convention).
#[inline]
fn internal_to_skim_score(score: i64) -> ScoreType {
    if score == SCORE_MAX {
        ScoreType::MAX / 2
    } else if score == SCORE_MIN {
        ScoreType::MIN / 2
    } else {
        // Saturate rather than panic: the DP sentinel (SCORE_MIN) can end up
        // offset by a few accumulated bonuses/penalties along a path that's
        // still effectively "no match" (see the fuzz-found case-folding bug
        // this fixed), landing close to but not exactly on SCORE_MIN/SCORE_MAX.
        score.saturating_mul(SCORE_TO_SKIM)
    }
}

// ---------------------------------------------------------------------------
// Public matcher struct
// ---------------------------------------------------------------------------

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
enum CaseMatching {
    Respect,
    Ignore,
    Smart,
}

/// Fuzzy matcher using the fzy algorithm.
///
/// This is a clean reimplementation of the scoring algorithm from
/// [fzy](https://github.com/jhawthorn/fzy) by John Hawthorn.
///
/// Supports optional typo tolerance via [`max_typos`](Self::max_typos).
#[derive(Debug)]
pub struct FzyMatcher {
    case: CaseMatching,
    use_cache: bool,
    max_typos: Option<usize>,
    c_cache: ThreadLocal<RefCell<Vec<char>>>,
    p_cache: ThreadLocal<RefCell<Vec<char>>>,
    lc_cache: ThreadLocal<RefCell<Vec<char>>>,
    lp_cache: ThreadLocal<RefCell<Vec<char>>>,
    typo_bufs: ThreadLocal<RefCell<TypoDpBuffers>>,
}

impl Default for FzyMatcher {
    fn default() -> Self {
        Self {
            case: CaseMatching::Ignore,
            use_cache: true,
            max_typos: None,
            c_cache: ThreadLocal::new(),
            p_cache: ThreadLocal::new(),
            lc_cache: ThreadLocal::new(),
            lp_cache: ThreadLocal::new(),
            typo_bufs: ThreadLocal::new(),
        }
    }
}

impl FzyMatcher {
    /// Sets the matcher to ignore case when matching.
    #[must_use]
    pub fn ignore_case(mut self) -> Self {
        self.case = CaseMatching::Ignore;
        self
    }

    /// Sets the matcher to use smart case.
    #[must_use]
    pub fn smart_case(mut self) -> Self {
        self.case = CaseMatching::Smart;
        self
    }

    /// Sets the matcher to respect case exactly.
    #[must_use]
    pub fn respect_case(mut self) -> Self {
        self.case = CaseMatching::Respect;
        self
    }

    /// Enables or disables thread-local caching.
    #[must_use]
    pub fn use_cache(mut self, use_cache: bool) -> Self {
        self.use_cache = use_cache;
        self
    }

    /// Sets the maximum number of typos allowed during matching.
    ///
    /// - `None` (default): strict subsequence matching with no typos.
    /// - `Some(n)`: allows up to `n` typos.
    #[must_use]
    pub fn max_typos(mut self, max_typos: Option<usize>) -> Self {
        self.max_typos = max_typos;
        self
    }

    fn contains_upper(string: &str) -> bool {
        string.chars().any(char::is_uppercase)
    }

    fn is_case_sensitive(&self, pattern: &str) -> bool {
        match self.case {
            CaseMatching::Respect => true,
            CaseMatching::Ignore => false,
            CaseMatching::Smart => Self::contains_upper(pattern),
        }
    }
}

impl FuzzyMatcher for FzyMatcher {
    fn fuzzy_indices(&self, choice: &str, pattern: &str) -> Option<(ScoreType, MatchIndices)> {
        let case_sensitive = self.is_case_sensitive(pattern);

        let mut choice_chars = self.c_cache.get_or(|| RefCell::new(Vec::new())).borrow_mut();
        let mut pattern_chars = self.p_cache.get_or(|| RefCell::new(Vec::new())).borrow_mut();

        choice_chars.clear();
        choice_chars.extend(choice.chars());
        pattern_chars.clear();
        pattern_chars.extend(pattern.chars());

        match self.max_typos {
            None => {
                cheap_matches(&choice_chars, &pattern_chars, case_sensitive)?;
                let mut positions = Vec::with_capacity(pattern_chars.len());
                let s = fzy_score(&pattern_chars, &choice_chars, case_sensitive, Some(&mut positions))?;
                Some((internal_to_skim_score(s), MatchIndices::from(positions)))
            }
            Some(max_t) => {
                // Fast path: try exact subsequence match first
                if cheap_matches(&choice_chars, &pattern_chars, case_sensitive).is_some() {
                    let mut positions = Vec::with_capacity(pattern_chars.len());
                    if let Some(s) = fzy_score(&pattern_chars, &choice_chars, case_sensitive, Some(&mut positions)) {
                        return Some((internal_to_skim_score(s), MatchIndices::from(positions)));
                    }
                }

                if max_t == 0 {
                    return None;
                }

                // Slow path: typo-tolerant matching
                let n = pattern_chars.len();
                let m = choice_chars.len();

                if n == 0 || m > MATCH_MAX_LEN || n > m + max_t {
                    return None;
                }

                // Compute lowercase pattern (small, fixed size) for prefilter
                let mut lower_pattern = self.lp_cache.get_or(|| RefCell::new(Vec::new())).borrow_mut();
                lower_pattern.clear();
                lower_pattern.extend(pattern_chars.iter().map(char::to_ascii_lowercase));

                if !can_match_with_typos(&choice_chars, &pattern_chars, &lower_pattern, case_sensitive, max_t) {
                    return None;
                }

                // Only compute lowercase choice after prefilter passes
                let mut lower_choice = self.lc_cache.get_or(|| RefCell::new(Vec::new())).borrow_mut();
                lower_choice.clear();
                lower_choice.extend(choice_chars.iter().map(char::to_ascii_lowercase));

                let match_bonus = precompute_bonus(&choice_chars);
                let mut bufs = self
                    .typo_bufs
                    .get_or(|| RefCell::new(TypoDpBuffers::default()))
                    .borrow_mut();
                let mut positions = Vec::with_capacity(n);
                let s = fzy_score_typos_full(
                    &pattern_chars,
                    &choice_chars,
                    &lower_pattern,
                    &lower_choice,
                    &match_bonus,
                    case_sensitive,
                    max_t,
                    &mut positions,
                    &mut bufs,
                )?;

                // Release the lowercase-cache borrows before optionally freeing
                // them; `replace` would otherwise re-borrow the same RefCells.
                drop(lower_choice);
                drop(lower_pattern);
                if !self.use_cache {
                    self.lc_cache.get().map(|cell| cell.replace(vec![]));
                    self.lp_cache.get().map(|cell| cell.replace(vec![]));
                }

                Some((internal_to_skim_score(s), MatchIndices::from(positions)))
            }
        }
    }

    fn fuzzy_match(&self, choice: &str, pattern: &str) -> Option<ScoreType> {
        let case_sensitive = self.is_case_sensitive(pattern);

        let mut choice_chars = self.c_cache.get_or(|| RefCell::new(Vec::new())).borrow_mut();
        let mut pattern_chars = self.p_cache.get_or(|| RefCell::new(Vec::new())).borrow_mut();

        choice_chars.clear();
        choice_chars.extend(choice.chars());
        pattern_chars.clear();
        pattern_chars.extend(pattern.chars());

        match self.max_typos {
            None => {
                cheap_matches(&choice_chars, &pattern_chars, case_sensitive)?;
                let s = fzy_score(&pattern_chars, &choice_chars, case_sensitive, None)?;
                Some(internal_to_skim_score(s))
            }
            Some(max_t) => {
                // Fast path: try exact subsequence match first
                if cheap_matches(&choice_chars, &pattern_chars, case_sensitive).is_some()
                    && let Some(s) = fzy_score(&pattern_chars, &choice_chars, case_sensitive, None)
                {
                    return Some(internal_to_skim_score(s));
                }

                if max_t == 0 {
                    return None;
                }

                // Slow path: typo-tolerant matching
                let n = pattern_chars.len();
                let m = choice_chars.len();

                if n == 0 || m > MATCH_MAX_LEN || n > m + max_t {
                    return None;
                }

                // Compute lowercase pattern (small, fixed size) for prefilter
                let mut lower_pattern = self.lp_cache.get_or(|| RefCell::new(Vec::new())).borrow_mut();
                lower_pattern.clear();
                lower_pattern.extend(pattern_chars.iter().map(char::to_ascii_lowercase));

                if !can_match_with_typos(&choice_chars, &pattern_chars, &lower_pattern, case_sensitive, max_t) {
                    return None;
                }

                // Only compute lowercase choice after prefilter passes
                let mut lower_choice = self.lc_cache.get_or(|| RefCell::new(Vec::new())).borrow_mut();
                lower_choice.clear();
                lower_choice.extend(choice_chars.iter().map(char::to_ascii_lowercase));

                let match_bonus = precompute_bonus(&choice_chars);
                let mut bufs = self
                    .typo_bufs
                    .get_or(|| RefCell::new(TypoDpBuffers::default()))
                    .borrow_mut();
                let s = fzy_score_typos_rolling(
                    &pattern_chars,
                    &choice_chars,
                    &lower_pattern,
                    &lower_choice,
                    &match_bonus,
                    case_sensitive,
                    max_t,
                    &mut bufs,
                )?;

                // Release the lowercase-cache borrows before optionally freeing
                // them; `replace` would otherwise re-borrow the same RefCells.
                drop(lower_choice);
                drop(lower_pattern);
                if !self.use_cache {
                    self.lc_cache.get().map(|cell| cell.replace(vec![]));
                    self.lp_cache.get().map(|cell| cell.replace(vec![]));
                }

                Some(internal_to_skim_score(s))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience free functions
// ---------------------------------------------------------------------------

/// Fuzzy match `choice` against `pattern` using the fzy algorithm, returning
/// the score and matched character indices.
#[must_use]
pub fn fuzzy_indices(choice: &str, pattern: &str) -> Option<(ScoreType, MatchIndices)> {
    FzyMatcher::default().ignore_case().fuzzy_indices(choice, pattern)
}

/// Fuzzy match `choice` against `pattern` using the fzy algorithm, returning
/// only the score.
#[must_use]
pub fn fuzzy_match(choice: &str, pattern: &str) -> Option<ScoreType> {
    FzyMatcher::default().ignore_case().fuzzy_match(choice, pattern)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "fzy_tests.rs"]
mod tests;
