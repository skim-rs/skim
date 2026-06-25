use super::*;
use crate::engine::exact::{ExactEngine, ExactMatchingParam};

fn exact(query: &str) -> Box<dyn MatchEngine> {
    Box::new(ExactEngine::builder(query, ExactMatchingParam::default()).build())
}

#[test]
fn or_engine_matches_if_any_subengine_matches() {
    let engine = OrEngine::builder().engines(vec![exact("foo"), exact("zzz")]).build();
    assert!(engine.match_item(&"a foo bar".to_string()).is_some());
}

#[test]
fn or_engine_returns_none_when_no_subengine_matches() {
    let engine = OrEngine::builder().engines(vec![exact("xxx"), exact("zzz")]).build();
    assert!(engine.match_item(&"a foo bar".to_string()).is_none());
}

#[test]
fn or_engine_empty_returns_none() {
    let engine = OrEngine::builder().build();
    assert!(engine.match_item(&"anything".to_string()).is_none());
}

#[test]
fn and_engine_single_engine_fast_path() {
    let engine = AndEngine::builder().engines(vec![exact("foo")]).build();
    assert!(engine.match_item(&"foobar".to_string()).is_some());
    assert!(engine.match_item(&"nope".to_string()).is_none());
}

#[test]
fn and_engine_requires_all_subengines_to_match() {
    let engine = AndEngine::builder().engines(vec![exact("foo"), exact("bar")]).build();
    // Both substrings present -> matched, ranges merged.
    let result = engine.match_item(&"foo and bar".to_string());
    assert!(result.is_some());
    let result = result.unwrap();
    assert!(matches!(result.matched_range, MatchRange::Chars(_)));

    // Missing one substring -> no match.
    assert!(engine.match_item(&"foo only".to_string()).is_none());
}

#[test]
fn and_engine_empty_returns_none() {
    // With no sub-engines the multi-engine path collects nothing and bails.
    let engine = AndEngine::builder().build();
    assert!(engine.match_item(&"anything".to_string()).is_none());
}

#[test]
fn display_formats_combinators() {
    let or = OrEngine::builder().engines(vec![exact("a")]).build();
    assert!(format!("{or}").starts_with("(Or:"));
    let and = AndEngine::builder().engines(vec![exact("a")]).build();
    assert!(format!("{and}").starts_with("(And:"));
}

fn result(range: MatchRange, score: i32, begin: i32, end: i32) -> MatchResult {
    MatchResult {
        rank: crate::Rank {
            score,
            begin,
            end,
            ..Default::default()
        },
        matched_range: range,
    }
}

#[test]
fn merge_handles_char_range_and_chars_variants() {
    // CharRange expands to its index span; Chars copies indices verbatim.
    // Scores are summed and begin/end take the widest span.
    let merged = AndEngine::merge_matched_items(
        vec![
            result(MatchRange::CharRange(0, 2), 5, 0, 2),
            result(MatchRange::Chars(vec![4, 5]), 3, 4, 5),
        ],
        "abcdef",
    );
    assert_eq!(merged.rank.score, 8);
    assert_eq!(merged.rank.begin, 0);
    assert_eq!(merged.rank.end, 5);
    assert_eq!(merged.matched_range, MatchRange::Chars(vec![0, 1, 4, 5]));
}

#[test]
fn merge_dedups_and_sorts_overlapping_ranges() {
    let merged = AndEngine::merge_matched_items(
        vec![
            result(MatchRange::Chars(vec![3, 1]), 1, 1, 3),
            result(MatchRange::CharRange(1, 3), 1, 1, 3),
        ],
        "abcdef",
    );
    // Sorted and de-duplicated union of {3,1} and {1,2}.
    assert_eq!(merged.matched_range, MatchRange::Chars(vec![1, 2, 3]));
}
