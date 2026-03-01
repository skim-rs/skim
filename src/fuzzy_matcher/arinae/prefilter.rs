//! Prefilters running before the algo to optimize performance on unmatchable items

use super::Atom;
use super::constants::MAX_PAT_LEN;

/// Cheap prefilter for typo-tolerant matching.
///
/// Rejects choices that clearly cannot produce a positive score in the DP.
/// The prefilter is intentionally lenient — false positives are fine (the DP
/// will reject them), but false negatives lose valid matches.
///
/// Strategy:
///   1. The first pattern character must appear somewhere in the choice at
///      position `j_first` (anchoring the alignment).
///   2. Of the remaining `n - 1` pattern characters, at least
///      `floor((n - 1) / 2)` must also appear (unordered, as a multiset) in
///      `choice[j_first..]` — the window the DP actually examines.
///
/// Scoping the tail check to `choice[j_first..]` is strictly correct: the
/// typo-mode DP band starts at `j_first` for every row (bandwidth = n + 4
/// always exceeds n - 1, so the left clamp always hits `j_first`). Any tail
/// character that only exists before `j_first` can never contribute a true
/// diagonal match in the DP; counting it would be a false positive.
///
/// We use a multiset frequency check rather than an ordered greedy scan.
/// An ordered scan causes false negatives when a greedily-consumed character
/// advances the cursor past positions where later characters could still match.
///
/// For the ASCII (`u8`) path the tail frequency table is built in a single
/// O(m) sequential pass over the window, then queried in O(n). For the `char`
/// path we fall back to a small O(n) linear-search table seeded from the
/// tail, queried via a scalar scan of the window — still a single O(m) pass.
pub(super) fn cheap_typo_prefilter<C: Atom>(pattern: &[C], choice: &[C], respect_case: bool) -> bool {
    let n = pattern.len();
    let m = choice.len();

    // A pattern much longer than the choice cannot match.
    if n > m + 2 {
        return false;
    }

    // The first pattern character must be present in the choice.
    // Use the SIMD-backed find_first_in (memchr for u8, scalar for char).
    // j_first is 0-indexed; the DP window is choice[j_first..].
    let first = pattern[0];
    let j_first = match first.find_first_in(choice, respect_case) {
        Some(pos) => pos,
        None => return false,
    };

    if n == 1 {
        return true;
    }

    let min_tail = (n - 1) / 2;
    if min_tail == 0 {
        return true;
    }

    // Tail frequency check scoped to choice[j_first..].
    // Build a frequency table over the window in one pass, then consume
    // entries as we walk the tail pattern characters.
    let window = &choice[j_first..];
    tail_freq_check(pattern, window, respect_case, min_tail)
}

/// Multiset frequency check: count how many of `pattern[1..]` can be
/// satisfied (one-for-one) by characters in `window`, and return `true`
/// as soon as `min_tail` matches are reached.
///
/// Builds a frequency table over `window` in a single O(|window|) pass,
/// then walks the tail in O(n). Total cost: O(m + n) with a single
/// sequential read of `window` — optimal cache behaviour.
#[inline]
fn tail_freq_check<C: Atom>(pattern: &[C], window: &[C], respect_case: bool, min_tail: usize) -> bool {
    // We need a per-character frequency table for the window.
    // Use a small stack-allocated array of (char_value, count) pairs keyed on
    // the PATTERN tail characters (at most MAX_PAT_LEN - 1 = 15 entries).
    // We build it in two passes:
    //   Pass 1 (O(n)): collect distinct tail chars into the table with count=0.
    //   Pass 2 (O(m)): scan window and increment matching table entries.
    //   Pass 3 (O(n)): walk the tail, decrement table entries, count matches.

    const MAX_TAIL: usize = MAX_PAT_LEN - 1;
    let tail = &pattern[1..];
    let tail_len = tail.len().min(MAX_TAIL);

    // Table of (pattern_char, available_count).  At most MAX_TAIL distinct chars.
    // Seed every slot with the first tail char and count=0 so the array is fully
    // initialised; only entries 0..table_len are ever consulted.
    let placeholder = tail[0];
    let mut table: [(C, u8); MAX_TAIL] = [(placeholder, 0); MAX_TAIL];
    let mut table_len = 0usize;

    // Pass 1: populate table with distinct tail chars (count = 0).
    for &pi in tail[..tail_len].iter() {
        if !table[..table_len].iter().any(|&(c, _)| pi.eq(c, respect_case)) {
            table[table_len] = (pi, 0);
            table_len += 1;
        }
    }

    // Pass 2: scan window, increment table counts (saturate at 255).
    for &c in window {
        if let Some(entry) = table[..table_len]
            .iter_mut()
            .find(|(tc, _)| Atom::eq(*tc, c, respect_case))
        {
            entry.1 = entry.1.saturating_add(1);
        }
    }

    // Pass 3: walk the tail, consume from table, count matches.
    let mut matched = 0usize;
    for &pi in tail[..tail_len].iter() {
        if let Some(entry) = table[..table_len]
            .iter_mut()
            .find(|(tc, _)| Atom::eq(pi, *tc, respect_case))
            && entry.1 > 0
        {
            entry.1 -= 1;
            matched += 1;
            if matched >= min_tail {
                return true;
            }
        }
    }

    false
}
