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
