//! Arinae fuzzy matching algorithm.
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

mod algo;
mod atom;
mod banding;
mod constants;
mod helpers;
mod matrix;
mod prefilter;
#[cfg(test)]
mod tests;

use std::cell::RefCell;

use thread_local::ThreadLocal;

use self::algo::{full_dp, range_dp};
use self::atom::Atom;
use self::constants::*;
use self::prefilter::cheap_typo_prefilter;

use self::matrix::{CELL_ZERO, Cell, Dir, SWMatrix};
use crate::{
    CaseMatching,
    fuzzy_matcher::{FuzzyMatcher, MatchIndices, ScoreType},
};

type Score = i16;

fn precompute_bonuses<C: Atom>(cho: &[C], buf: &mut Vec<Score>) {
    // Reset length (O(1), no deallocation) then fill with fresh values.
    buf.clear();
    // The first character always gets START_OF_STRING_BONUS.
    // Subsequent characters get a bonus based on the previous character:
    //   - separator_bonus() when the previous char is a separator (the exact
    //     bonus depends on the separator — see SEPARATOR_TABLE in constants.rs),
    //   - CAMEL_CASE_BONUS when transitioning from lowercase to non-lowercase.
    // Using a safe iterator lets the compiler auto-vectorise the loop.
    let bonus_iter = std::iter::once(START_OF_STRING_BONUS).chain(cho.windows(2).map(|w| {
        let prev = w[0];
        let cur = w[1];
        prev.separator_bonus() + CAMEL_CASE_BONUS * ((prev.is_lowercase() && !cur.is_lowercase()) as Score)
    }));
    buf.extend(bonus_iter);
}

/// Arinae fuzzy matcher: Smith-Waterman local alignment with affine gap
/// penalties and context-sensitive bonuses.
#[derive(Debug, Default)]
pub struct ArinaeMatcher {
    pub(crate) case: CaseMatching,
    pub(crate) allow_typos: bool,
    full_buf: ThreadLocal<RefCell<SWMatrix>>,
    indices_buf: ThreadLocal<RefCell<MatchIndices>>,
    #[allow(clippy::type_complexity)]
    char_buf: ThreadLocal<RefCell<(Vec<char>, Vec<char>)>>,
    bonus_buf: ThreadLocal<RefCell<Vec<Score>>>,
}

impl ArinaeMatcher {
    /// Create a new `ArinaeMatcher` with the given settings.
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

    /// Dispatch to `full_dp` with the appropriate const generics.
    /// Assumes prefilters and bonuses have already been computed.
    fn dispatch_dp<C: Atom>(
        &self,
        cho: &[C],
        pat: &[C],
        bonuses: &[Score],
        respect_case: bool,
        compute_indices: bool,
    ) -> Option<(ScoreType, MatchIndices)> {
        let res = if self.allow_typos {
            if compute_indices {
                full_dp::<true, true, _>(cho, pat, bonuses, respect_case, &self.full_buf, &self.indices_buf)
            } else {
                full_dp::<true, false, _>(cho, pat, bonuses, respect_case, &self.full_buf, &self.indices_buf)
            }
        } else if compute_indices {
            full_dp::<false, true, _>(cho, pat, bonuses, respect_case, &self.full_buf, &self.indices_buf)
        } else {
            full_dp::<false, false, _>(cho, pat, bonuses, respect_case, &self.full_buf, &self.indices_buf)
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

        // Prepare bonuses
        let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
        precompute_bonuses(cho, &mut bonus_buf);

        self.dispatch_dp(cho, pat, &bonus_buf, respect_case, compute_indices)
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

        // Prefilter for typo mode only (see match_slices for rationale).
        if self.allow_typos && !cheap_typo_prefilter(pat_buf, cho_buf, respect_case) {
            return None;
        }

        let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
        precompute_bonuses(cho_buf, &mut bonus_buf);

        // Call dispatch_dp directly to avoid double-borrowing bonus_buf.
        self.dispatch_dp(cho_buf, pat_buf, &bonus_buf, respect_case, compute_indices)
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

        let range = if choice.is_ascii() && pattern.is_ascii() {
            let cho = choice.as_bytes();
            let pat = pattern.as_bytes();
            let respect_case = self.respect_case(pat);
            // Exact mode: compute_banding validates the subsequence implicitly.
            if self.allow_typos && !cheap_typo_prefilter(pat, cho, respect_case) {
                return None;
            }
            let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
            precompute_bonuses(cho, &mut bonus_buf);
            if self.allow_typos {
                range_dp::<true, _>(cho, pat, &bonus_buf, respect_case, &self.full_buf)
            } else {
                range_dp::<false, _>(cho, pat, &bonus_buf, respect_case, &self.full_buf)
            }
        } else {
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
            // Exact mode: compute_banding validates the subsequence implicitly.
            if self.allow_typos && !cheap_typo_prefilter(pat_buf, cho_buf, respect_case) {
                return None;
            }
            let mut bonus_buf = self.bonus_buf.get_or(|| RefCell::new(Vec::new())).borrow_mut();
            precompute_bonuses(cho_buf, &mut bonus_buf);
            if self.allow_typos {
                range_dp::<true, _>(cho_buf, pat_buf, &bonus_buf, respect_case, &self.full_buf)
            } else {
                range_dp::<false, _>(cho_buf, pat_buf, &bonus_buf, respect_case, &self.full_buf)
            }
        };
        range.map(|(s, b, e)| (s as ScoreType, b, e))
    }
}

// ---------------------------------------------------------------------------
// FuzzyMatcher trait implementation
// ---------------------------------------------------------------------------

impl FuzzyMatcher for ArinaeMatcher {
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
