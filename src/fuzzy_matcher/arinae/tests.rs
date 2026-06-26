use super::*;
use crate::fuzzy_matcher::FuzzyMatcher;

fn matcher() -> ArinaeMatcher {
    ArinaeMatcher::default()
}

fn matcher_typos() -> ArinaeMatcher {
    ArinaeMatcher {
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
fn pattern_longer_than_max_pat_len_is_rejected() {
    // Patterns over MAX_PAT_LEN (32) chars exceed the stack-allocated banding
    // arrays, so the matcher rejects them gracefully rather than panicking.
    let pattern = "a".repeat(40);
    let choice = "a".repeat(50);
    assert!(score(&choice, &pattern).is_none());
    assert!(matcher().fuzzy_indices(&choice, &pattern).is_none());
    // Also via the non-ASCII (char-buffer) path.
    let pattern_u = "é".repeat(40);
    let choice_u = "é".repeat(50);
    assert!(score(&choice_u, &pattern_u).is_none());
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
    let m = ArinaeMatcher {
        case: CaseMatching::Smart,
        allow_typos: false,
        ..Default::default()
    };
    assert!(m.fuzzy_match("FooBar", "foobar").is_some());
}

#[test]
fn smart_case_sensitive_uppercase_pattern() {
    let m = ArinaeMatcher {
        case: CaseMatching::Smart,
        allow_typos: false,
        ..Default::default()
    };
    assert!(m.fuzzy_match("foobar", "FooBar").is_none());
    assert!(m.fuzzy_match("FooBar", "FooBar").is_some());
}

#[test]
fn respect_case() {
    let m = ArinaeMatcher {
        case: CaseMatching::Respect,
        allow_typos: false,
        ..Default::default()
    };
    assert!(m.fuzzy_match("abc", "ABC").is_none());
    assert!(m.fuzzy_match("ABC", "ABC").is_some());
}

#[test]
fn ignore_case() {
    let m = ArinaeMatcher {
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
    let m = ArinaeMatcher {
        case: CaseMatching::Ignore,
        allow_typos: false,
        ..Default::default()
    };
    assert_order(&m, "ab", &["ab", "aoo_boo", "acb"]);
}

#[test]
fn ordering_print() {
    use crate::fuzzy_matcher::util::assert_order;
    let m = ArinaeMatcher {
        case: CaseMatching::Ignore,
        allow_typos: false,
        ..Default::default()
    };
    assert_order(&m, "print", &["printf", "sprintf"]);
}

// ----- Score-only vs full DP consistency -----

#[test]
fn score_only_matches_full_dp() {
    let m = ArinaeMatcher {
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
// grep '.*t.*e.*s.*t' should give the same results as arinae with pattern 'test'.
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
            "fuzzy_match should match subsequence 'test' in {choice:?}",
        );
        assert!(
            m.fuzzy_indices(choice, "test").is_some(),
            "fuzzy_indices should match subsequence 'test' in {choice:?}",
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
        );
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

// ----- Prefilter regression tests -----

/// Extending a typo-tolerant match with an additional character must not cause
/// the candidate to be incorrectly rejected.
///
/// "fobara" matches `src/fuzzy_matcher/arinae/algo.rs` via the typo-tolerant
/// DP (score 91). Typing one more character to form "fobaral" should continue
/// to match — the `a`, `r`, `a` subsequence exists in the choice string and
/// satisfies the prefilter threshold (`min_tail` = 3).
///
/// The old prefilter used a greedy ordered scan that consumed `o` at position 28,
/// locking the cursor past all four `a` occurrences (at positions 11, 18, 22, 25),
/// causing a false negative. The correct approach is an unordered frequency check.
#[test]
fn typo_prefilter_no_false_negative_on_extension() {
    let choice = "src/fuzzy_matcher/arinae/algo.rs";
    // Both the shorter and the extended pattern must match.
    assert!(
        score_typos(choice, "fobara").is_some(),
        "\"fobara\" should match \"{choice}\""
    );
    assert!(
        score_typos(choice, "fobaral").is_some(),
        "\"fobaral\" should match \"{choice}\" (regression: greedy prefilter scan false negative)"
    );
}

#[test]
fn use_last_match_prefers_later_occurrence() {
    // "man/man1/sk.1" contains "man" at indices 0..=2 and again at 4..=6.
    // With use_last_match=true, the matcher should highlight the second one.
    let m = ArinaeMatcher {
        use_last_match: true,
        ..Default::default()
    };
    let (_, got) = m.fuzzy_indices("man/man1/sk.1", "man").expect("should match");
    assert_eq!(got, vec![4, 5, 6], "expected second 'man' (indices 4,5,6), got {got:?}");
}

#[test]
fn no_use_last_match_prefers_first_occurrence() {
    // "man/man1/sk.1" contains "man" at indices 0..=2 and again at 4..=6.
    // With use_last_match=true, the matcher should highlight the second one.
    let m = ArinaeMatcher::default();
    let (_, got) = m.fuzzy_indices("man/man1/sk.1", "man").expect("should match");
    assert_eq!(got, vec![0, 1, 2], "expected second 'man' (indices 0,1,2), got {got:?}");
}

#[test]
fn range_with_use_last_match_prefers_later_occurrence() {
    // fuzzy_match_range with use_last_match takes the `>=` tie-break branch in
    // range_dp, choosing the rightmost matching column.
    let m = ArinaeMatcher {
        use_last_match: true,
        ..Default::default()
    };
    let (_score, begin, end) = m.fuzzy_match_range("man/man1/sk.1", "man").expect("should match");
    assert_eq!(begin, 4);
    assert_eq!(end, 6);
}

#[test]
fn range_with_gaps_walks_traceback() {
    // A scattered match forces gap moves during the range traceback.
    let m = ArinaeMatcher::default();
    let (_score, begin, end) = m.fuzzy_match_range("a_b_c_d_e", "abe").expect("should match");
    // First matched char is 'a' at 0, last is 'e' at 8.
    assert_eq!(begin, 0);
    assert_eq!(end, 8);
}

#[test]
fn range_typo_dead_rows_rejects_long_mismatch() {
    // Typo-tolerant range matching over a choice with no viable alignment
    // exercises the dead-row early-out in range_dp.
    let m = ArinaeMatcher::new(crate::CaseMatching::Smart, true, false);
    assert!(m.fuzzy_match_range("xxxxxxxxxxxxxxxx", "qwerty").is_none());
}

#[test]
fn first_match_inside_brackets_is_highlighted() {
    // Regression test for skim-rs/skim#1075. `[paste] some paste` queried with
    // `paste` should highlight the first occurrence (inside the brackets), not
    // the second. Before treating brackets as word separators the second
    // occurrence scored higher because it followed a space.
    let m = ArinaeMatcher::default();
    let (_, got) = m.fuzzy_indices("[paste] some paste", "paste").expect("should match");
    assert_eq!(
        got,
        vec![1, 2, 3, 4, 5],
        "expected first 'paste' inside brackets, got {got:?}"
    );

    // Same expectation for the other bracket and paren variants now treated
    // as separators.
    for choice in ["(paste) some paste", "{paste} some paste"] {
        let (_, got) = m.fuzzy_indices(choice, "paste").expect("should match");
        assert_eq!(
            got,
            vec![1, 2, 3, 4, 5],
            "expected first 'paste' for {choice:?}, got {got:?}"
        );
    }
}

// ----- fuzzy_match_range edge cases -----

/// Empty pattern / choice short-circuit the range DP.
#[test]
fn range_empty_inputs() {
    let m = matcher();
    assert_eq!(m.fuzzy_match_range("hello", ""), Some((0, 0, 0)));
    assert_eq!(m.fuzzy_match_range("", "hello"), None);
}

/// `fuzzy_match_range` over non-ASCII text must agree with `fuzzy_indices`,
/// exercising the `char`-buffer (non-ASCII) branch of `run_range`.
#[test]
fn range_non_ascii_consistent_with_indices() {
    let cases = [
        ("héllo wörld", "hw"),
        ("café taverne", "café"),
        ("naïve élégance", "néé"),
        ("日本語テキスト", "本テ"),
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
                    assert_eq!(rb, fidx.first().copied().unwrap_or_default());
                    assert_eq!(re, fidx.last().copied().unwrap_or_default());
                }
                _ => panic!("range/indices disagreement for ({choice}, {pattern})"),
            }
        }
    }
}

/// Non-ASCII no-match returns None through the `char`-buffer range branch.
#[test]
fn range_non_ascii_no_match() {
    assert_eq!(matcher().fuzzy_match_range("café", "zzz"), None);
    assert_eq!(matcher_typos().fuzzy_match_range("日本語", "xyz"), None);
}

// ----- Prefilter (typo mode) edge cases -----

/// A pattern more than two characters longer than the choice is rejected by
/// the cheap typo prefilter (`n > m + 2`) before any DP runs.
#[test]
fn typo_prefilter_rejects_overlong_pattern() {
    // "abcdef" (6) vs "ab" (2): 6 > 2 + 2, so the prefilter bails immediately.
    assert!(matcher_typos().fuzzy_match("ab", "abcdef").is_none());
    // Same through the non-ASCII (char) path.
    assert!(matcher_typos().fuzzy_match("éé", "ééçñøü").is_none());
}

/// A single-character pattern only needs its one character present; the
/// prefilter short-circuits on `n == 1`.
#[test]
fn typo_prefilter_single_char_pattern() {
    // 'a' is present in "banana" → accepted and matched.
    assert!(matcher_typos().fuzzy_match("banana", "a").unwrap() > 0);
    // 'z' absent → rejected at the first-char anchor.
    assert!(matcher_typos().fuzzy_match("banana", "z").is_none());
}

// ----- ASCII vs non-ASCII dispatch -----

/// An ASCII choice with a non-ASCII pattern must fall through to the
/// char-buffer path in both `fuzzy_match` and `fuzzy_match_range`.
#[test]
fn ascii_choice_non_ascii_pattern_uses_char_path() {
    // No match, but the char path is exercised and returns cleanly.
    assert!(matcher().fuzzy_match("hello world", "élé").is_none());
    assert!(matcher().fuzzy_match_range("hello world", "élé").is_none());
    // A real match where the pattern char only exists in the (ascii) choice as
    // its base form would not match; use a choice that actually contains it.
    assert!(matcher().fuzzy_match("clichéless", " é").is_none());
}

/// Non-ASCII pattern longer than the choice is rejected by the prefilter on
/// the char path of `fuzzy_match`.
#[test]
fn typo_non_ascii_overlong_pattern_rejected() {
    assert!(matcher_typos().fuzzy_match("café", "ééééééééé").is_none());
}

// ----- Typo DP: no positive alignment & substitutions -----

/// A single-substitution match in typo mode: the traceback walks a Diag step
/// whose characters differ, so that position is NOT reported as a match index.
#[test]
fn typo_substitution_excluded_from_indices() {
    let m = matcher_typos();
    let (score, idx) = m.fuzzy_indices("cat", "cot").expect("typo match");
    assert!(score > 0);
    // 'c' (0) and 't' (2) are true matches; the substituted 'o'→'a' at index 1
    // must not appear in the reported indices.
    assert_eq!(idx, vec![0, 2], "substituted position should be excluded");

    // The range variant walks the same substitution during its traceback.
    let (rscore, begin, end) = m.fuzzy_match_range("cat", "cot").expect("typo range match");
    assert!(rscore > 0);
    assert_eq!((begin, end), (0, 2));
}

/// Typo range matching where, after the anchor, two consecutive rows produce
/// no positive cell triggers the dead-row early termination in `range_dp`.
#[test]
fn typo_range_dead_rows_after_anchor() {
    // 'a' anchors at index 0, but "qq" can match nothing in the tail, so rows
    // 2 and 3 are dead and the DP bails.
    assert!(matcher_typos().fuzzy_match_range("axxxxxxxx", "aqq").is_none());
}

/// A clean contiguous match's index traceback consumes the whole pattern,
/// exiting the traceback loop via the `i > 0` guard (not an early `Dir::None`).
#[test]
fn full_match_traceback_consumes_pattern() {
    let (_, idx) = matcher().fuzzy_indices("abcdef", "abc").expect("prefix match");
    assert_eq!(idx, vec![0, 1, 2]);
}

// ---------------------------------------------------------------------------
// Direct unit tests for the DP kernels and banding helpers.
//
// `compute_banding` guarantees a feasible diagonal band before `full_dp` /
// `range_dp` ever run, so the kernels' defensive pruning paths (a row entirely
// outside the band, a fully-dead matrix, an empty last row) cannot be reached
// through the public matcher API. These tests drive the kernels directly with
// hand-built `BandingInfo` to verify those guards behave correctly — i.e. an
// infeasible band or an unmatchable pattern yields no match rather than reading
// stale/garbage cells.
// ---------------------------------------------------------------------------
mod kernel {
    use std::cell::RefCell;
    use thread_local::ThreadLocal;

    use super::super::algo::{full_dp, range_dp};
    use super::super::banding::BandingInfo;
    use super::super::constants::MAX_PAT_LEN;
    use super::super::helpers::{compute_last_match_cols, compute_row_col_bounds};
    use super::ArinaeMatcher;

    /// Build an exact-mode `BandingInfo` with explicit per-row column bounds.
    fn exact_banding(j_first: usize, rows: &[(usize, usize)]) -> BandingInfo {
        let mut lo = [0usize; MAX_PAT_LEN];
        let mut hi = [0usize; MAX_PAT_LEN];
        for (idx, &(l, h)) in rows.iter().enumerate() {
            lo[idx] = l;
            hi[idx] = h;
        }
        BandingInfo {
            row_bounds: Some((lo, hi)),
            j_first,
            bandwidth: 0,
            min_true_matches: 0,
        }
    }

    /// Build a typo-mode `BandingInfo` (no per-row bounds; uses the V-band).
    fn typo_banding(j_first: usize, bandwidth: usize) -> BandingInfo {
        BandingInfo {
            row_bounds: None,
            j_first,
            bandwidth,
            min_true_matches: 0,
        }
    }

    // ----- match_slices empty-input guards -----

    #[test]
    fn match_slices_empty_pattern_is_trivial_match() {
        let m = ArinaeMatcher::default();
        // Empty pattern → score 0 with no indices, regardless of the choice.
        assert_eq!(m.match_slices(b"abc", b"", true), Some((0, vec![])));
    }

    #[test]
    fn match_slices_empty_choice_never_matches() {
        let m = ArinaeMatcher::default();
        assert_eq!(m.match_slices(b"", b"abc", true), None);
    }

    // ----- helpers guards -----

    #[test]
    fn compute_last_match_cols_rejects_overlong_pattern() {
        // A pattern longer than MAX_PAT_LEN overflows the stack banding arrays,
        // so the helper bails with None instead of indexing out of bounds.
        let pat = vec![b'a'; MAX_PAT_LEN + 1];
        let cho = vec![b'a'; MAX_PAT_LEN + 5];
        assert_eq!(compute_last_match_cols(&pat, &cho, false), None);
        // A pattern exactly at the limit is still accepted.
        let pat_ok = vec![b'a'; MAX_PAT_LEN];
        assert!(compute_last_match_cols(&pat_ok, &cho, false).is_some());
    }

    #[test]
    fn compute_row_col_bounds_handles_first_match_at_column_one() {
        // When a later pattern char first matches at column 1, the forward pass
        // must NOT extend the previous row's upper bound (next_lo <= 1).
        let mut first = [0usize; MAX_PAT_LEN];
        let mut last = [0usize; MAX_PAT_LEN];
        // pat[0] matches at col 3, pat[1] matches first at col 1 (next_lo == 1).
        first[0] = 3;
        last[0] = 3;
        first[1] = 1;
        last[1] = 4;
        let (lo, hi) = compute_row_col_bounds(2, 5, &first, &last);
        // Row 0's upper bound stays at its own last-match (3), not widened by a
        // next_lo of 1; all bounds remain clamped within [1, m].
        assert!(lo[0] >= 1 && hi[0] >= lo[0] && hi[0] <= 5);
        assert!(lo[1] >= 1 && hi[1] >= lo[1] && hi[1] <= 5);
        assert_eq!(hi[0], 3, "next_lo == 1 must not widen the previous row");
    }

    // ----- full_dp: unmatchable pattern → no positive cell -----

    #[test]
    fn full_dp_no_match_returns_none() {
        // pat[0] ('q') never appears in the choice, so every cell stays <= 0 and
        // the last-row scan finds best_score == 0.
        let full_buf = ThreadLocal::new();
        let indices_buf = ThreadLocal::new();
        let cho = b"hello";
        let pat = b"qq";
        let bonuses = [0i16; 5];
        let banding = exact_banding(1, &[(1, 5), (2, 5)]);
        let res = full_dp::<false, true, u8>(cho, pat, &bonuses, true, &full_buf, &indices_buf, false, &banding);
        assert_eq!(res, None);
    }

    // ----- full_dp: row entirely outside the band (infeasible diagonal) -----

    #[test]
    fn full_dp_exact_band_skip_is_infeasible() {
        // j_first = 3 with a 4-char pattern over a 4-char choice forces the
        // diagonal off the end: rows 3 and 4 fall entirely outside the band.
        let full_buf = ThreadLocal::new();
        let indices_buf = ThreadLocal::new();
        let cho = b"abcd";
        let pat = b"abcd";
        let bonuses = [0i16; 4];
        // Per-row bounds keep the (unused) last-row scan feasible; the kernel's
        // V-band still prunes rows 3 and 4.
        let banding = exact_banding(3, &[(3, 4), (4, 4), (4, 4), (4, 4)]);
        let res = full_dp::<false, false, u8>(cho, pat, &bonuses, true, &full_buf, &indices_buf, false, &banding);
        // No row can place pattern chars 3 and 4 → no complete alignment.
        assert_eq!(res, None);
    }

    #[test]
    fn full_dp_typo_band_skip_is_infeasible() {
        // Same infeasible diagonal in typo mode (bandwidth 0): exercises the
        // ALLOW_TYPOS branch of the next-row peek inside the skip handler.
        let full_buf = ThreadLocal::new();
        let indices_buf = ThreadLocal::new();
        let cho = b"abcd";
        let pat = b"abcd";
        let bonuses = [0i16; 4];
        let banding = typo_banding(3, 0);
        let res = full_dp::<true, false, u8>(cho, pat, &bonuses, true, &full_buf, &indices_buf, false, &banding);
        assert_eq!(res, None);
    }

    // ----- range_dp: unmatchable pattern → dead rows early-out -----

    #[test]
    fn range_dp_dead_rows_returns_none() {
        // No 'q' in the choice: every row is all-zero, so two consecutive dead
        // rows trip the early-out.
        let full_buf = ThreadLocal::new();
        let cho = b"hello";
        let pat = b"qq";
        let bonuses = [0i16; 5];
        let banding = exact_banding(1, &[(1, 5), (1, 5)]);
        let res = range_dp::<false, u8>(cho, pat, &bonuses, true, &full_buf, false, &banding);
        assert_eq!(res, None);
    }

    // ----- range_dp: middle row outside band, then a feasible row -----

    #[test]
    fn range_dp_band_skip_middle_row() {
        // Row 2's bounds are inverted (lo > hi) → that row is skipped, but rows
        // 1 and 3 are feasible, exercising the single-skip path with a feasible
        // next-row peek.
        let full_buf = ThreadLocal::new();
        let cho = b"abcde";
        let pat = b"abc";
        let bonuses = [0i16; 5];
        let banding = exact_banding(1, &[(1, 5), (5, 2), (1, 5)]);
        let res = range_dp::<false, u8>(cho, pat, &bonuses, true, &full_buf, false, &banding);
        // Pattern char 2's row is skipped, so its cells stay zeroed. Row 3 then
        // matches 'c' (col index 2) afresh, and the traceback terminates cleanly
        // at the zeroed row (Dir::None) — yielding the single-char span (2, 2)
        // with score == MATCH_BONUS rather than reading stale cells.
        assert_eq!(res, Some((18, 2, 2)));
    }

    #[test]
    fn range_dp_band_skip_two_consecutive_rows() {
        // Rows 2 and 3 are both skipped → two consecutive band-skipped rows trip
        // the dead-row early-out inside the skip handler.
        let full_buf = ThreadLocal::new();
        let cho = b"abcde";
        let pat = b"abcd";
        let bonuses = [0i16; 5];
        let banding = exact_banding(1, &[(1, 5), (5, 2), (5, 2), (1, 5)]);
        let res = range_dp::<false, u8>(cho, pat, &bonuses, true, &full_buf, false, &banding);
        assert_eq!(res, None);
    }

    #[test]
    fn range_dp_empty_last_row() {
        // Rows 1 produces real matches, but the last row's bounds are inverted,
        // so the last-row scan is skipped and best_score stays 0.
        let full_buf = ThreadLocal::new();
        let cho = b"ab";
        let pat = b"ab";
        let bonuses = [0i16; 2];
        let banding = exact_banding(1, &[(1, 2), (5, 2)]);
        let res = range_dp::<false, u8>(cho, pat, &bonuses, true, &full_buf, false, &banding);
        assert_eq!(res, None);
    }
}

// ---------------------------------------------------------------------------
// More kernel guards: rows whose band lies entirely *past* the end of the
// choice (lo <= hi but lo > m). These hit the `<= m` second operands of the
// band-skip / last-row short-circuits, which the V-band (where hi is always m)
// can never trigger on its own.
// ---------------------------------------------------------------------------
mod kernel_past_end {
    use thread_local::ThreadLocal;

    use super::super::algo::{full_dp, range_dp};
    use super::super::banding::BandingInfo;
    use super::super::constants::MAX_PAT_LEN;

    fn exact_banding(j_first: usize, rows: &[(usize, usize)]) -> BandingInfo {
        let mut lo = [0usize; MAX_PAT_LEN];
        let mut hi = [0usize; MAX_PAT_LEN];
        for (idx, &(l, h)) in rows.iter().enumerate() {
            lo[idx] = l;
            hi[idx] = h;
        }
        BandingInfo {
            row_bounds: Some((lo, hi)),
            j_first,
            bandwidth: 0,
            min_true_matches: 0,
        }
    }

    fn typo_banding(j_first: usize, bandwidth: usize) -> BandingInfo {
        BandingInfo {
            row_bounds: None,
            j_first,
            bandwidth,
            min_true_matches: 0,
        }
    }

    /// `full_dp`: an intermediate skipped row whose next-row peek bounds start
    /// beyond the choice end exercises the `nj_lo <= m` guard's false arm.
    #[test]
    fn full_dp_peek_row_past_choice_end() {
        let full_buf = ThreadLocal::new();
        let indices_buf = ThreadLocal::new();
        let cho = b"abcd"; // m = 4
        let pat = b"abcde"; // n = 5 — diagonal runs off the end (j_first = 3)
        let bonuses = [0i16; 4];
        // Row 4's peeked bounds (idx 3) start past m; the real last row (idx 4)
        // is in range so the scan stays in bounds.
        let banding = exact_banding(3, &[(3, 4), (4, 4), (4, 4), (7, 8), (4, 4)]);
        let res = full_dp::<false, false, u8>(cho, pat, &bonuses, true, &full_buf, &indices_buf, false, &banding);
        assert_eq!(res, None);
    }

    /// `range_dp`: rows whose band starts beyond the choice end are skipped via
    /// the `j_lo > m` arm; two such rows trip the dead-row early-out, and the
    /// peek for the next out-of-range row hits the `nj_lo <= m` false arm.
    #[test]
    fn range_dp_rows_past_choice_end() {
        let full_buf = ThreadLocal::new();
        let cho = b"abcde"; // m = 5
        let pat = b"abc";
        let bonuses = [0i16; 5];
        let banding = exact_banding(1, &[(1, 5), (7, 8), (7, 8)]);
        let res = range_dp::<false, u8>(cho, pat, &bonuses, true, &full_buf, false, &banding);
        assert_eq!(res, None);
    }

    /// `range_dp`: the last row's band starts past the choice end, so the
    /// last-row scan is skipped (the `last_j_lo <= m` false arm) and no best
    /// score is recorded despite an earlier matching row.
    #[test]
    fn range_dp_last_row_past_choice_end() {
        let full_buf = ThreadLocal::new();
        let cho = b"ab"; // m = 2
        let pat = b"ab";
        let bonuses = [0i16; 2];
        let banding = exact_banding(1, &[(1, 2), (7, 8)]);
        let res = range_dp::<false, u8>(cho, pat, &bonuses, true, &full_buf, false, &banding);
        assert_eq!(res, None);
    }

    /// `range_dp` in typo mode: an off-the-end diagonal skips a row, exercising
    /// the ALLOW_TYPOS arm of the next-row peek in the range kernel.
    #[test]
    fn range_dp_typo_band_skip() {
        let full_buf = ThreadLocal::new();
        let cho = b"xxab"; // 'a' at col 3 matches the fabricated j_first
        let pat = b"abcd";
        let bonuses = [0i16; 4];
        let banding = typo_banding(3, 0);
        let res = range_dp::<true, u8>(cho, pat, &bonuses, true, &full_buf, false, &banding);
        assert_eq!(res, None);
    }
}
