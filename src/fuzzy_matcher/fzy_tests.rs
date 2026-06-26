//! Unit tests for the fzy fuzzy matcher (see [`super`]).

use super::*;
use crate::fuzzy_matcher::util::{assert_order, wrap_matches};

fn wrap_fuzzy_match(choice: &str, pattern: &str) -> Option<String> {
    let (_score, indices) = fuzzy_indices(choice, pattern)?;
    Some(wrap_matches(choice, &indices))
}

#[test]
fn test_no_match() {
    assert_eq!(None, fuzzy_match("abc", "abx"));
    assert_eq!(None, fuzzy_match("abc", "d"));
    assert_eq!(None, fuzzy_match("", "a"));
}

#[test]
fn test_has_match() {
    assert!(fuzzy_match("axbycz", "abc").is_some());
    assert!(fuzzy_match("axbycz", "xyz").is_some());
    assert!(fuzzy_match("abc", "abc").is_some());
}

#[test]
fn test_exact_match_is_max() {
    let matcher = FzyMatcher::default().ignore_case();
    let score = matcher.fuzzy_match("abc", "abc").unwrap();
    assert!(score > 1_000_000);
}

#[test]
fn test_match_indices() {
    assert_eq!("[a]x[b]y[c]z", &wrap_fuzzy_match("axbycz", "abc").unwrap());
    assert_eq!("a[x]b[y]c[z]", &wrap_fuzzy_match("axbycz", "xyz").unwrap());
}

#[test]
fn test_consecutive_bonus() {
    let matcher = FzyMatcher::default().ignore_case();
    let consecutive = matcher.fuzzy_match("foobar", "foo").unwrap();
    let scattered = matcher.fuzzy_match("fxoxo", "foo").unwrap();
    assert!(
        consecutive > scattered,
        "consecutive={consecutive} > scattered={scattered}"
    );
}

#[test]
fn test_word_boundary_bonus() {
    let matcher = FzyMatcher::default().ignore_case();
    let boundary = matcher.fuzzy_match("foo_bar_baz", "fbb").unwrap();
    let inner = matcher.fuzzy_match("fooobarbaz", "fbb").unwrap();
    assert!(boundary > inner, "boundary={boundary} > inner={inner}");
}

#[test]
fn test_path_separator_bonus() {
    let matcher = FzyMatcher::default().ignore_case();
    let path = matcher.fuzzy_match("src/lib/foo.rs", "foo").unwrap();
    let no_path = matcher.fuzzy_match("srcxlibxfoo.rs", "foo").unwrap();
    assert!(path > no_path, "path={path} > no_path={no_path}");
}

#[test]
fn test_camel_case_bonus() {
    let matcher = FzyMatcher::default().ignore_case();
    let camel = matcher.fuzzy_match("FooBarBaz", "fbb").unwrap();
    let no_camel = matcher.fuzzy_match("foobarbaz", "fbb").unwrap();
    assert!(camel > no_camel, "camel={camel} > no_camel={no_camel}");
}

#[test]
fn test_shorter_match_preferred() {
    let matcher = FzyMatcher::default().ignore_case();
    let short = matcher.fuzzy_match("ab", "ab").unwrap();
    let long = matcher.fuzzy_match("axxxxxxb", "ab").unwrap();
    assert!(short > long, "short={short} > long={long}");
}

#[test]
fn test_match_quality_ordering() {
    let matcher = FzyMatcher::default();
    assert_order(&matcher, "monad", &["monad", "Monad", "mONAD"]);
    assert_order(&matcher, "ab", &["ab", "aoo_boo", "acb"]);
    assert_order(&matcher, "ma", &["map", "many", "maximum"]);
}

#[test]
fn test_unicode_match() {
    let matcher = FzyMatcher::default().ignore_case();
    let result = matcher.fuzzy_indices("Hello, 世界", "H世");
    assert!(result.is_some());
    let (_, indices) = result.unwrap();
    assert_eq!(indices.as_slice(), &[0, 7]);
}

#[test]
fn test_smart_case() {
    let matcher = FzyMatcher::default().smart_case();
    assert!(matcher.fuzzy_match("FooBar", "foobar").is_some());
    assert!(matcher.fuzzy_match("foobar", "FooBar").is_none());
    assert!(matcher.fuzzy_match("FooBar", "FooBar").is_some());
}

#[test]
fn test_respect_case() {
    let matcher = FzyMatcher::default().respect_case();
    assert!(matcher.fuzzy_match("abc", "ABC").is_none());
    assert!(matcher.fuzzy_match("ABC", "ABC").is_some());
}

#[test]
fn test_long_haystack() {
    let matcher = FzyMatcher::default().ignore_case();
    let long = "a".repeat(MATCH_MAX_LEN + 1);
    assert_eq!(None, matcher.fuzzy_match(&long, "a"));
}

// -----------------------------------------------------------------------
// Typo-tolerant matching tests
// -----------------------------------------------------------------------

#[test]
fn test_typo_no_typos_behaves_like_default() {
    let strict = FzyMatcher::default().ignore_case();
    let typo0 = FzyMatcher::default().ignore_case().max_typos(Some(0));

    assert!(strict.fuzzy_match("axbycz", "abc").is_some());
    assert!(typo0.fuzzy_match("axbycz", "abc").is_some());

    assert!(strict.fuzzy_match("abc", "abx").is_none());
    assert!(typo0.fuzzy_match("abc", "abx").is_none());
}

#[test]
fn test_typo_substitution_single() {
    let matcher = FzyMatcher::default().ignore_case().max_typos(Some(1));
    assert!(matcher.fuzzy_match("abc", "abx").is_some(), "substitution: 'x' for 'c'");
}

#[test]
fn test_typo_substitution_returns_none_when_too_many_typos() {
    let matcher = FzyMatcher::default().ignore_case().max_typos(Some(1));
    assert!(
        matcher.fuzzy_match("abc", "ayx").is_none(),
        "2 typos needed but only 1 allowed"
    );

    let matcher2 = FzyMatcher::default().ignore_case().max_typos(Some(2));
    assert!(matcher2.fuzzy_match("abc", "ayx").is_some(), "2 typos allowed");
}

#[test]
fn test_typo_needle_deletion() {
    let matcher = FzyMatcher::default().ignore_case().max_typos(Some(1));
    assert!(matcher.fuzzy_match("abd", "abcd").is_some(), "needle deletion of 'c'");

    let strict = FzyMatcher::default().ignore_case();
    assert!(strict.fuzzy_match("abd", "abcd").is_none());
}

#[test]
fn test_typo_exact_match_scores_higher_than_typo_match() {
    let matcher = FzyMatcher::default().ignore_case().max_typos(Some(1));
    let exact = matcher.fuzzy_match("abc", "abc").unwrap();
    let typo = matcher.fuzzy_match("axc", "abc").unwrap();
    assert!(exact > typo, "exact ({exact}) > typo ({typo})");
}

#[test]
fn test_typo_subsequence_beats_typo() {
    let matcher = FzyMatcher::default().ignore_case().max_typos(Some(1));
    let subseq = matcher.fuzzy_match("axbycz", "abc").unwrap();
    let typo = matcher.fuzzy_match("abx", "abc").unwrap();
    assert!(subseq > typo, "subsequence ({subseq}) > typo ({typo})");
}

#[test]
fn test_typo_indices_substitution() {
    let matcher = FzyMatcher::default().ignore_case().max_typos(Some(1));
    let result = matcher.fuzzy_indices("abx", "abc");
    assert!(result.is_some());
    let (_, indices) = result.unwrap();
    assert_eq!(indices.as_slice(), &[0, 1, 2]);
}

#[test]
fn test_typo_indices_needle_deletion() {
    let matcher = FzyMatcher::default().ignore_case().max_typos(Some(1));
    let result = matcher.fuzzy_indices("abd", "abcd");
    assert!(result.is_some());
    let (_, indices) = result.unwrap();
    // 'a'→0, 'b'→1, 'c' deleted (no index), 'd'→2
    assert_eq!(indices.as_slice(), &[0, 1, 2]);
}

#[test]
fn test_typo_max_typos_none_is_zero_overhead() {
    let default = FzyMatcher::default().ignore_case();
    let explicit_none = FzyMatcher::default().ignore_case().max_typos(None);

    let choices = ["foobar", "axbycz", "src/lib/foo.rs", "FooBarBaz"];
    let pattern = "foo";

    for choice in &choices {
        assert_eq!(
            default.fuzzy_match(choice, pattern),
            explicit_none.fuzzy_match(choice, pattern),
            "max_typos(None) should match default for '{choice}'"
        );
    }
}

#[test]
fn test_typo_realistic_filename() {
    let matcher = FzyMatcher::default().ignore_case().max_typos(Some(1));
    let result = matcher.fuzzy_match("controller", "controllr");
    assert!(
        result.is_some(),
        "should match 'controller' with needle 'controllr' (1 typo)"
    );
}

#[test]
fn test_typo_two_typos() {
    let matcher = FzyMatcher::default().ignore_case().max_typos(Some(2));
    assert!(matcher.fuzzy_match("abc", "xyz").is_none());
    assert!(matcher.fuzzy_match("abc", "axz").is_some());
}

#[test]
fn test_typo_empty_pattern() {
    let matcher = FzyMatcher::default().ignore_case().max_typos(Some(1));
    assert_eq!(None, matcher.fuzzy_match("abc", ""));
}

#[test]
fn test_typo_pattern_longer_than_haystack() {
    let matcher = FzyMatcher::default().ignore_case().max_typos(Some(1));
    assert!(matcher.fuzzy_match("ab", "abc").is_some(), "delete 'c' from needle");
    assert!(matcher.fuzzy_match("a", "abc").is_none());

    let matcher2 = FzyMatcher::default().ignore_case().max_typos(Some(2));
    assert!(matcher2.fuzzy_match("a", "abc").is_some());
}

#[test]
fn test_uppercase_after_separator_bonuses() {
    // An uppercase pattern char following a separator earns that separator's
    // bonus (slash / dash / dot), exercising the char_class==2 bonus arms.
    let m = FzyMatcher::default().respect_case();
    assert!(m.fuzzy_match("foo/Bar", "B").is_some());
    assert!(m.fuzzy_match("foo-Bar", "B").is_some());
    assert!(m.fuzzy_match("foo.Bar", "B").is_some());
    // camelCase: uppercase preceded by a lowercase letter.
    assert!(m.fuzzy_match("fooBar", "B").is_some());
    // Uppercase preceded by another uppercase earns no bonus (the `_ => 0` arm).
    assert!(m.fuzzy_match("ABc", "B").is_some());
}

#[test]
fn test_use_cache_setter_is_chainable() {
    // The use_cache builder setter returns a working matcher (cache enabled).
    let m = FzyMatcher::default().ignore_case().use_cache(true);
    assert!(m.fuzzy_match("foobar", "fb").is_some());
}

#[test]
fn test_exact_length_match_returns_all_indices() {
    // When pattern and choice are the same length and fully match, fzy_score
    // short-circuits to SCORE_MAX and returns every index.
    let m = FzyMatcher::default().ignore_case();
    let (_score, indices) = m.fuzzy_indices("abc", "abc").unwrap();
    assert_eq!(indices, vec![0, 1, 2]);
}

#[test]
fn test_case_sensitive_subsequence_scoring() {
    // respect_case with n < m forces fzy_score's `is_match` to use the
    // case-sensitive comparison branch (needle[i] == haystack[j]) on a real,
    // non-degenerate match (cheap_matches passes, then the DP runs).
    let m = FzyMatcher::default().respect_case();
    // "abc" is a strict subsequence of "aXbYc" with matching case.
    let (score, indices) = m.fuzzy_indices("aXbYc", "abc").unwrap();
    assert_eq!(indices, vec![0, 2, 4]);
    assert!(score > 0);
    // A case mismatch in the middle must fail under respect_case.
    assert!(m.fuzzy_match("aXBYc", "abc").is_none());
    // Score-only path (fuzzy_match) over the same case-sensitive subsequence.
    assert!(m.fuzzy_match("aXbYc", "abc").is_some());
}

#[test]
fn test_dp_cell_match_at_first_haystack_char_for_later_needle() {
    // Exercises the `i > 0 && j == 0` matched-cell edge in fzy_score: the
    // second needle char ('b') equals the first haystack char ('b'), which the
    // DP evaluates even though it cannot be part of an in-order match.
    let m = FzyMatcher::default().ignore_case();
    // "ab" is a subsequence of "bab" (a@1, b@2); the DP still visits (i=1,j=0).
    let (_score, indices) = m.fuzzy_indices("bab", "ab").unwrap();
    assert_eq!(indices, vec![1, 2]);
}

#[test]
fn test_case_sensitive_typo_substitution() {
    // respect_case + typos: the substitution path must use the case-sensitive
    // comparison branch in both the rolling (fuzzy_match) and full
    // (fuzzy_indices) typo DP routines.
    let m = FzyMatcher::default().respect_case().max_typos(Some(1));
    // 'X' substitutes for 'c' (one typo), all other chars match case exactly.
    assert!(m.fuzzy_match("abXd", "abcd").is_some());
    let (_score, indices) = m.fuzzy_indices("abXd", "abcd").unwrap();
    assert_eq!(indices.len(), 4);
    // A case-only difference still costs a typo under respect_case.
    let strict = FzyMatcher::default().respect_case();
    assert!(strict.fuzzy_match("abCd", "abcd").is_none());
    assert!(m.fuzzy_match("abCd", "abcd").is_some());
}

#[test]
fn test_typo_indices_zero_allowed_falls_back_to_none() {
    // fuzzy_indices with max_typos(Some(0)): when the cheap subsequence check
    // fails, the `max_t == 0` guard returns None without entering the DP.
    let m = FzyMatcher::default().ignore_case().max_typos(Some(0));
    assert!(m.fuzzy_indices("abc", "abx").is_none());
    // And a clean subsequence still matches through the fast path.
    assert!(m.fuzzy_indices("axbxc", "abc").is_some());
}

#[test]
fn test_typo_indices_pattern_too_long_for_haystack() {
    // fuzzy_indices typo slow-path length guard: n > m + max_t returns None.
    let m = FzyMatcher::default().ignore_case().max_typos(Some(1));
    // pattern len 4, haystack len 2, 1 typo allowed -> 4 > 2 + 1.
    assert!(m.fuzzy_indices("ab", "abcd").is_none());
    // One needle deletion is enough when the gap is exactly max_t.
    assert!(m.fuzzy_indices("abc", "abcd").is_some());
}

#[test]
fn test_typo_use_cache_disabled_in_slow_path() {
    // use_cache(false) on the typo slow path frees the lowercase caches after
    // the DP; the match (which needs a real typo, so it skips the fast path)
    // must still succeed, and a repeated call must work after the drop.
    let m = FzyMatcher::default().ignore_case().max_typos(Some(1)).use_cache(false);
    assert!(m.fuzzy_match("abx", "abc").is_some()); // rolling DP (substitution)
    assert!(m.fuzzy_indices("abx", "abc").is_some()); // full DP
    assert!(m.fuzzy_match("abx", "abc").is_some()); // works again after drop
}

#[test]
fn test_typo_match_at_first_haystack_char_for_later_needle() {
    // Non-subsequence (forces the typo DP) where a later needle char equals the
    // first haystack char: needle "ab" vs haystack "bc" — 'a' has no match, but
    // 'b' equals haystack[0]. Exercises the matched-cell `i > 0 && j == 0` edge
    // in both typo DPs.
    let m = FzyMatcher::default().ignore_case().max_typos(Some(1));
    // The DP visits the (i=1, j=0) matched cell, but no in-order alignment can
    // use it (a needle char before 'b' would need a column < 0), so the result
    // is None even though the branch is exercised.
    assert!(m.fuzzy_match("bc", "ab").is_none());
    assert!(m.fuzzy_indices("bc", "ab").is_none());
}

#[test]
fn test_typo_substitution_at_start_and_end() {
    let m = FzyMatcher::default().ignore_case().max_typos(Some(1));
    // Substitution at the first needle position. (A leading substitution scores
    // identically to a leading needle-deletion, so 'a' is not assigned an index;
    // 'b' and 'c' are.)
    assert!(m.fuzzy_match("Xbc", "abc").is_some());
    let (_s, idx) = m.fuzzy_indices("Xbc", "abc").unwrap();
    assert_eq!(idx, vec![1, 2]);
    // Substitution at the last needle position keeps all three indices.
    assert!(m.fuzzy_match("abX", "abc").is_some());
    let (_s, idx) = m.fuzzy_indices("abX", "abc").unwrap();
    assert_eq!(idx, vec![0, 1, 2]);
}

#[test]
fn test_typo_sparse_alignment_with_min_predecessors() {
    // A non-subsequence with scattered matches makes early DP columns stay at
    // SCORE_MIN, so later matched cells read SCORE_MIN predecessors (the
    // `pm == SCORE_MIN` / `pv == SCORE_MIN` arms) without those cells winning.
    let m = FzyMatcher::default().ignore_case().max_typos(Some(2));
    // needle "abcd": 'a' and 'd' anchor at the ends, 'b','c' need substitutions.
    assert!(m.fuzzy_match("aXYd", "abcd").is_some());
    let (_s, idx) = m.fuzzy_indices("aXYd", "abcd").unwrap();
    assert_eq!(idx.len(), 4);
    // 'a' and 'd' match at the ends; 'b' and 'c' are substituted in the middle.
    assert_eq!(idx, vec![0, 1, 2, 3]);
}

#[test]
fn test_typo_no_alignment_returns_none_from_dp() {
    // Passes the cheap typo prefilter (length ok, enough chars present) but the
    // DP finds no positive alignment within the typo budget → None via the
    // `best_score == SCORE_MIN` guard.
    let m = FzyMatcher::default().ignore_case().max_typos(Some(1));
    // 3 substitutions needed but only 1 allowed.
    assert!(m.fuzzy_match("abc", "xyz").is_none());
    assert!(m.fuzzy_indices("abc", "xyz").is_none());
}

#[test]
fn test_typo_needle_deletion_indices_and_gaps() {
    // Needle deletions plus haystack gaps drive the backtrace's deletion and
    // gap (j == 0) arms in the full typo DP.
    let m = FzyMatcher::default().ignore_case().max_typos(Some(2));
    // Delete 'c' and 'e' from the needle; 'a','b','d' align.
    let (_s, idx) = m.fuzzy_indices("abd", "abcde").unwrap();
    // Only the matched needle chars contribute indices.
    assert!(idx.len() <= 3 && !idx.is_empty());
    assert!(idx.windows(2).all(|w| w[0] < w[1]), "indices strictly increasing");
}

#[test]
fn test_typo_pattern_too_long_even_with_typos() {
    // n > m + max_typos short-circuits to None in the typo slow path (both
    // fuzzy_match and fuzzy_indices).
    let m = FzyMatcher::default().ignore_case().max_typos(Some(1));
    assert!(m.fuzzy_match("ab", "abcde").is_none());
    assert!(m.fuzzy_indices("ab", "abcde").is_none());
}

// ----- Non-typo fzy_score edge cases -----

#[test]
fn test_score_only_match_at_first_char_for_later_needle() {
    // Score-only path: needle "aa" over haystack "aXa" visits (i=1, j=0) where
    // needle[1] matches haystack[0] but j == 0 makes it a SCORE_MIN dead cell.
    let m = FzyMatcher::default().ignore_case();
    assert!(m.fuzzy_match("aXa", "aa").is_some());
}

#[test]
fn test_single_char_pattern_indices() {
    // n == 1 with the full (position) matrix uses the trailing-gap row-0 path.
    let m = FzyMatcher::default().ignore_case();
    let (_s, idx) = m.fuzzy_indices("help", "l").unwrap();
    assert_eq!(idx, vec![2]); // the unique 'l'
}

#[test]
fn test_fzy_score_pattern_longer_than_choice_returns_none() {
    // Direct guard test: the public API rejects pattern-longer-than-choice in
    // cheap_matches first, so exercise fzy_score's own `n > m` guard directly.
    assert_eq!(fzy_score(&['a', 'b', 'c'], &['a'], false, None), None);
    assert_eq!(fzy_score(&[], &['a'], false, None), None); // n == 0 guard
}

#[test]
fn test_can_match_with_typos_length_guard() {
    // Direct guard test: a needle longer than haystack + max_typos cannot match.
    let pat = ['a', 'b', 'c', 'd'];
    assert!(!can_match_with_typos(&['a'], &pat, &pat, false, 1));
    // Within budget (one deletion) it can.
    assert!(can_match_with_typos(&['a', 'b', 'c'], &pat, &pat, false, 1));
}

#[test]
fn test_typo_choice_exceeds_match_max_len() {
    // A choice longer than MATCH_MAX_LEN that is not a clean subsequence reaches
    // the typo slow path and is rejected by the `m > MATCH_MAX_LEN` guard.
    let m = FzyMatcher::default().ignore_case().max_typos(Some(1));
    let huge = "a".repeat(2000); // > MATCH_MAX_LEN (1024)
    // 'z' never appears, so the cheap subsequence fails and we hit the guard.
    assert!(m.fuzzy_match(&huge, "az").is_none());
    assert!(m.fuzzy_indices(&huge, "az").is_none());
}

#[test]
fn test_typo_indices_internal_gap_backtrace() {
    // A typo match (non-subsequence) with an internal haystack gap drives the
    // full-DP backtrace across a skipped column (its D cell is SCORE_MIN).
    let m = FzyMatcher::default().ignore_case().max_typos(Some(1));
    // "abd" vs "aXbc": a@0, gap at X, b@2, 'd' substituted for 'c'@3.
    let (_score, idx) = m.fuzzy_indices("aXbc", "abd").expect("typo match with gap");
    assert_eq!(idx, vec![0, 2, 3]);
}

#[test]
fn test_indices_scattered_match_backtrace_scans_gaps() {
    // A scattered (gappy) exact subsequence makes the position backtrace scan
    // across non-matching columns where d != m before locating each match.
    let m = FzyMatcher::default().ignore_case();
    let (_score, idx) = m.fuzzy_indices("a_b_c_d", "abcd").expect("scattered match");
    assert_eq!(idx, vec![0, 2, 4, 6]);
    // A different gap pattern for additional backtrace shapes.
    let (_score, idx) = m.fuzzy_indices("xaybzc", "abc").expect("scattered match");
    assert_eq!(idx, vec![1, 3, 5]);
}

#[test]
fn test_typo_fast_path_subsequence_too_long_falls_through() {
    // A choice longer than MATCH_MAX_LEN where the pattern IS a clean
    // subsequence: cheap_matches succeeds, but fzy_score returns None (m too
    // long), so the fast path falls through to the (also-rejecting) slow path.
    let m = FzyMatcher::default().ignore_case().max_typos(Some(1));
    let huge = "a".repeat(2000);
    assert!(m.fuzzy_indices(&huge, "aa").is_none());
    assert!(m.fuzzy_match(&huge, "aa").is_none());
}

#[test]
fn test_indices_repeated_char_backtrace_disambiguates() {
    // Repeated pattern characters give the backtrace multiple candidate columns
    // per row, so it must scan past non-optimal match cells (d != m) to find the
    // chosen alignment.
    let m = FzyMatcher::default().ignore_case();
    let (_s, idx) = m.fuzzy_indices("aabaa", "aba").expect("should match");
    assert_eq!(idx.len(), 3);
    assert!(idx.windows(2).all(|w| w[0] < w[1]));
    let (_s, idx) = m.fuzzy_indices("banana", "aaa").expect("should match");
    assert_eq!(idx, vec![1, 3, 5]);
}

#[test]
fn test_indices_nonoptimal_match_cells_in_backtrace() {
    // Inputs where a needle char can match several columns, so the backtrace
    // encounters match cells (d != SCORE_MIN) that are not on the optimal path
    // (d != m), and others where the scan walks toward column 0.
    let m = FzyMatcher::default().ignore_case();
    for (choice, pattern) in [
        ("aXaXa", "aaa"),
        ("aabaab", "aba"),
        ("a.a.a", "aa"),
        ("baba", "ba"),
        ("xaxaxa", "aaa"),
    ] {
        let r = m.fuzzy_indices(choice, pattern);
        assert!(r.is_some(), "{choice:?}/{pattern:?} should match");
        let (_s, idx) = r.unwrap();
        assert_eq!(idx.len(), pattern.chars().count());
        assert!(idx.windows(2).all(|w| w[0] < w[1]), "indices increasing");
    }
}

#[test]
fn test_typo_indices_multiple_gaps_and_deletions() {
    // Typo matches with multiple internal gaps and needle deletions to drive the
    // full-DP backtrace through gap columns (d == SCORE_MIN) and deletion steps.
    let m = FzyMatcher::default().ignore_case().max_typos(Some(2));
    for (choice, pattern) in [("aXbYcd", "abce"), ("a_b_c", "axc"), ("foobar", "fxxr")] {
        let _ = m.fuzzy_indices(choice, pattern);
    }
    // A concrete checked case: gap before a substituted tail char.
    let (_s, idx) = m.fuzzy_indices("aXbYZ", "abc").expect("typo match");
    assert!(!idx.is_empty() && idx.windows(2).all(|w| w[0] < w[1]));
}
