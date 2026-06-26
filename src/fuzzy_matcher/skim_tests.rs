use crate::fuzzy_matcher::util::{assert_order, wrap_matches};

use super::*;

fn wrap_fuzzy_match(matcher: &dyn FuzzyMatcher, line: &str, pattern: &str) -> Option<String> {
    let (score, indices) = matcher.fuzzy_indices(line, pattern)?;
    println!("score: {score:?}, indices: {indices:?}");
    Some(wrap_matches(line, &indices))
}

#[test]
fn test_match_or_not() {
    let matcher = SkimMatcherV2::default();
    assert_eq!(Some(0), matcher.fuzzy_match("", ""));
    assert_eq!(Some(0), matcher.fuzzy_match("abcdefaghi", ""));
    assert_eq!(None, matcher.fuzzy_match("", "a"));
    assert_eq!(None, matcher.fuzzy_match("abcdefaghi", "中"));
    assert_eq!(None, matcher.fuzzy_match("abc", "abx"));
    assert!(matcher.fuzzy_match("axbycz", "abc").is_some());
    assert!(matcher.fuzzy_match("axbycz", "xyz").is_some());

    assert_eq!("[a]x[b]y[c]z", &wrap_fuzzy_match(&matcher, "axbycz", "abc").unwrap());
    assert_eq!("a[x]b[y]c[z]", &wrap_fuzzy_match(&matcher, "axbycz", "xyz").unwrap());
    assert_eq!(
        "[H]ello, [世]界",
        &wrap_fuzzy_match(&matcher, "Hello, 世界", "H世").unwrap()
    );
}

#[test]
fn test_match_quality() {
    let matcher = SkimMatcherV2::default().ignore_case();

    // initials
    assert_order(&matcher, "ab", &["ab", "aoo_boo", "acb"]);
    assert_order(&matcher, "CC", &["CamelCase", "camelCase", "camelcase"]);
    assert_order(&matcher, "cC", &["camelCase", "CamelCase", "camelcase"]);
    assert_order(
        &matcher,
        "cc",
        &["camel case", "camelCase", "CamelCase", "camelcase", "camel ace"],
    );
    assert_order(
        &matcher,
        "Da.Te",
        &["Data.Text", "Data.Text.Lazy", "Data.Aeson.Encoding.text"],
    );
    // prefix
    assert_order(&matcher, "is", &["isIEEE", "inSuf"]);
    // shorter
    assert_order(&matcher, "ma", &["map", "many", "maximum"]);
    assert_order(&matcher, "print", &["printf", "sprintf"]);
    // score(PRINT) = kMinScore
    assert_order(&matcher, "ast", &["ast", "AST", "INT_FAST16_MAX"]);
    // score(PRINT) > kMinScore
    assert_order(&matcher, "Int", &["int", "INT", "PRINT"]);
}

fn simple_match(
    matcher: &SkimMatcherV2,
    choice: &str,
    pattern: &str,
    case_sensitive: bool,
    with_pos: bool,
) -> Option<(ScoreType, Vec<IndexType>)> {
    let choice: Vec<char> = choice.chars().collect();
    let pattern: Vec<char> = pattern.chars().collect();
    let first_match_indices = cheap_matches(&choice, &pattern, case_sensitive)?;
    matcher.simple_match(&choice, &pattern, &first_match_indices, case_sensitive, with_pos)
}

#[test]
fn test_match_or_not_simple() {
    let matcher = SkimMatcherV2::default();
    assert_eq!(
        simple_match(&matcher, "axbycz", "xyz", false, true).unwrap().1,
        vec![1, 3, 5]
    );

    assert_eq!(simple_match(&matcher, "", "", false, false), Some((0, vec![])));
    assert_eq!(
        simple_match(&matcher, "abcdefaghi", "", false, false),
        Some((0, vec![]))
    );
    assert_eq!(simple_match(&matcher, "", "a", false, false), None);
    assert_eq!(simple_match(&matcher, "abcdefaghi", "中", false, false), None);
    assert_eq!(simple_match(&matcher, "abc", "abx", false, false), None);
    assert_eq!(
        simple_match(&matcher, "axbycz", "abc", false, true).unwrap().1,
        vec![0, 2, 4]
    );
    assert_eq!(
        simple_match(&matcher, "axbycz", "xyz", false, true).unwrap().1,
        vec![1, 3, 5]
    );
    assert_eq!(
        simple_match(&matcher, "Hello, 世界", "H世", false, true).unwrap().1,
        vec![0, 7]
    );
}

#[test]
fn test_match_or_not_v2() {
    let matcher = SkimMatcherV2::default().debug(true);

    assert_eq!(matcher.fuzzy_match("", ""), Some(0));
    assert_eq!(matcher.fuzzy_match("abcdefaghi", ""), Some(0));
    assert_eq!(matcher.fuzzy_match("", "a"), None);
    assert_eq!(matcher.fuzzy_match("abcdefaghi", "中"), None);
    assert_eq!(matcher.fuzzy_match("abc", "abx"), None);
    assert!(matcher.fuzzy_match("axbycz", "abc").is_some());
    assert!(matcher.fuzzy_match("axbycz", "xyz").is_some());

    assert_eq!(&wrap_fuzzy_match(&matcher, "axbycz", "abc").unwrap(), "[a]x[b]y[c]z");
    assert_eq!(&wrap_fuzzy_match(&matcher, "axbycz", "xyz").unwrap(), "a[x]b[y]c[z]");
    assert_eq!(
        &wrap_fuzzy_match(&matcher, "Hello, 世界", "H世").unwrap(),
        "[H]ello, [世]界"
    );
}

#[test]
fn test_case_option_v2() {
    let matcher = SkimMatcherV2::default().ignore_case();
    assert!(matcher.fuzzy_match("aBc", "abc").is_some());
    assert!(matcher.fuzzy_match("aBc", "aBc").is_some());
    assert!(matcher.fuzzy_match("aBc", "aBC").is_some());

    let matcher = SkimMatcherV2::default().respect_case();
    assert!(matcher.fuzzy_match("aBc", "abc").is_none());
    assert!(matcher.fuzzy_match("aBc", "aBc").is_some());
    assert!(matcher.fuzzy_match("aBc", "aBC").is_none());

    let matcher = SkimMatcherV2::default().smart_case();
    assert!(matcher.fuzzy_match("aBc", "abc").is_some());
    assert!(matcher.fuzzy_match("aBc", "aBc").is_some());
    assert!(matcher.fuzzy_match("aBc", "aBC").is_none());
}

#[test]
fn test_matcher_quality_v2() {
    let matcher = SkimMatcherV2::default();
    assert_order(&matcher, "ab", &["ab", "aoo_boo", "acb"]);
    assert_order(
        &matcher,
        "cc",
        &["camel case", "camelCase", "CamelCase", "camelcase", "camel ace"],
    );
    assert_order(
        &matcher,
        "Da.Te",
        &["Data.Text", "Data.Text.Lazy", "Data.Aeson.Encoding.Text"],
    );
    assert_order(&matcher, "is", &["isIEEE", "inSuf"]);
    assert_order(&matcher, "ma", &["map", "many", "maximum"]);
    assert_order(&matcher, "print", &["printf", "sprintf"]);
    assert_order(&matcher, "ast", &["ast", "AST", "INT_FAST16_MAX"]);
    assert_order(&matcher, "int", &["int", "INT", "PRINT"]);
}

#[test]
fn test_reuse_should_not_affect_indices() {
    let matcher = SkimMatcherV2::default();
    let pattern = "139";
    for num in 0..10000 {
        let choice = num.to_string();
        if let Some((_score, indices)) = matcher.fuzzy_indices(&choice, pattern) {
            assert_eq!(indices.len(), 3);
        }
    }
}

#[test]
fn builder_setters_are_chainable() {
    // score_config and use_cache builder setters return a working matcher.
    let matcher = SkimMatcherV2::default()
        .score_config(SkimScoreConfig::default())
        .use_cache(true)
        .debug(false);
    assert!(matcher.fuzzy_match("foobar", "fb").is_some());
}

#[test]
fn element_limit_falls_back_to_simple_match() {
    // A tiny element limit forces the simple_match path instead of the full DP.
    let matcher = SkimMatcherV2::default().ignore_case().element_limit(1);

    // Single-character pattern hits the dedicated one-char branch.
    let (_score, indices) = matcher.fuzzy_indices("hello", "l").unwrap();
    assert_eq!(indices.len(), 1);

    // Multi-character pattern walks the reverse fill loop.
    let (_score, indices) = matcher.fuzzy_indices("axbycz", "abc").unwrap();
    assert_eq!(indices.len(), 3);

    // simple_match still rejects non-subsequences.
    assert!(matcher.fuzzy_match("abc", "xyz").is_none());
}

#[test]
fn simple_match_empty_pattern_scores_zero() {
    let matcher = SkimMatcherV2::default().element_limit(1);
    assert_eq!(matcher.fuzzy_match("hello", ""), Some(0));
}

#[test]
fn use_cache_disabled_drops_buffers_after_match() {
    // With caching disabled the matcher frees its scratch buffers after each
    // call (the `!use_cache` arms in fuzzy/`fuzzy_with` and the simple path),
    // but still returns correct results across repeated calls.
    let matcher = SkimMatcherV2::default().ignore_case().use_cache(false);
    // Full-DP path (default element_limit) for both indices and score-only.
    let (_score, indices) = matcher.fuzzy_indices("foobar", "fb").unwrap();
    assert_eq!(indices, vec![0, 3]);
    assert!(matcher.fuzzy_match("foobar", "fb").is_some());
    // A second call must still work after the buffers were dropped.
    assert!(matcher.fuzzy_indices("foobar", "bar").is_some());
}

#[test]
fn simple_match_single_char_at_start_has_no_prev_char() {
    // A one-character pattern matching the very first choice character takes the
    // `match_idx == 0` arm (no preceding character → treated as NonWord).
    let matcher = SkimMatcherV2::default();
    let (score, indices) = simple_match(&matcher, "hello", "h", false, true).unwrap();
    assert_eq!(indices, vec![0]);
    // The same char in the middle of a word scores differently (has a prev char).
    let (mid_score, _) = simple_match(&matcher, "ahello", "h", false, true).unwrap();
    assert_ne!(score, mid_score, "start-of-string bonus should differ from mid-word");
}

#[test]
fn simple_match_score_only_skips_position_tracking() {
    // A multi-character match with with_pos = false reaches
    // calculate_score_with_pos via the score-only path (no positions recorded).
    let matcher = SkimMatcherV2::default();
    let (score, positions) = simple_match(&matcher, "axbycz", "abc", false, false).unwrap();
    assert!(score > 0);
    assert!(positions.is_empty(), "score-only path must not collect positions");

    // The position-tracking variant returns the same score with indices filled.
    let (pos_score, positions) = simple_match(&matcher, "axbycz", "abc", false, true).unwrap();
    assert_eq!(score, pos_score);
    assert_eq!(positions, vec![0, 2, 4]);
}

#[test]
fn gappy_match_traceback_skips_to_first_column() {
    // Matches with long gaps between pattern characters force the position
    // traceback to take Skip moves that walk the column index down to the first
    // matched column while pattern rows remain (the `j > first_col` guard).
    let matcher = SkimMatcherV2::default();
    for (choice, pattern, expected) in [
        ("a___________b", "ab", vec![0usize, 12]),
        ("x_a_____b__c", "abc", vec![2, 8, 11]),
        ("a_b_______c", "abc", vec![0, 2, 10]),
    ] {
        let (_score, indices) = matcher.fuzzy_indices(choice, pattern).expect("should match");
        let got: Vec<usize> = indices.iter().map(|&i| i as usize).collect();
        assert_eq!(got, expected, "choice={choice:?} pattern={pattern:?}");
    }
}
