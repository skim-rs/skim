//! Prefilters running before the algo to optimize performance on unmatchable items

use super::Atom;

/// Cheap prefilter for typo-tolerant matching.
///
/// Rejects choices that clearly cannot produce a positive score in the DP.
/// The prefilter is intentionally lenient — false positives are fine (the DP
/// will reject them), but false negatives lose valid matches.
///
/// Strategy: the first pattern character must appear somewhere in the choice
/// (anchoring the alignment). Of the remaining `n - 1` pattern characters,
/// at least `floor((n - 1) / 2)` must also appear (unordered) in the choice.
pub(super) fn cheap_typo_prefilter<C: Atom>(pattern: &[C], choice: &[C], respect_case: bool) -> bool {
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
