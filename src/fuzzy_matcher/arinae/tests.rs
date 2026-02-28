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
            "fuzzy_match should match subsequence 'test' in {:?}",
            choice
        );
        assert!(
            m.fuzzy_indices(choice, "test").is_some(),
            "fuzzy_indices should match subsequence 'test' in {:?}",
            choice
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
        )
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
/// "fobara" matches "src/fuzzy_matcher/arinae/algo.rs" via the typo-tolerant
/// DP (score 91). Typing one more character to form "fobaral" should continue
/// to match — the `a`, `r`, `a` subsequence exists in the choice string and
/// satisfies the prefilter threshold (min_tail = 3).
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
