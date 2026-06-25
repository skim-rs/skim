use super::*;

fn engine(query: &str, param: ExactMatchingParam) -> ExactEngine {
    ExactEngine::builder(query, param).build()
}

#[test]
fn case_respect_is_sensitive() {
    let e = engine(
        "Foo",
        ExactMatchingParam {
            case: CaseMatching::Respect,
            ..Default::default()
        },
    );
    assert!(e.match_item(&"a Foo b".to_string()).is_some());
    assert!(e.match_item(&"a foo b".to_string()).is_none());
}

#[test]
fn case_ignore_is_insensitive() {
    let e = engine(
        "Foo",
        ExactMatchingParam {
            case: CaseMatching::Ignore,
            ..Default::default()
        },
    );
    assert!(e.match_item(&"a foo b".to_string()).is_some());
    assert!(e.match_item(&"a FOO b".to_string()).is_some());
}

#[test]
fn case_smart_uppercase_query_is_sensitive() {
    let e = engine(
        "Foo",
        ExactMatchingParam {
            case: CaseMatching::Smart,
            ..Default::default()
        },
    );
    assert!(e.match_item(&"Foo".to_string()).is_some());
    assert!(e.match_item(&"foo".to_string()).is_none());
}

#[test]
fn prefix_and_postfix_anchors() {
    let prefix = engine(
        "foo",
        ExactMatchingParam {
            prefix: true,
            case: CaseMatching::Ignore,
            ..Default::default()
        },
    );
    assert!(prefix.match_item(&"foobar".to_string()).is_some());
    assert!(prefix.match_item(&"barfoo".to_string()).is_none());

    let postfix = engine(
        "foo",
        ExactMatchingParam {
            postfix: true,
            case: CaseMatching::Ignore,
            ..Default::default()
        },
    );
    assert!(postfix.match_item(&"barfoo".to_string()).is_some());
    assert!(postfix.match_item(&"foobar".to_string()).is_none());
}

#[test]
fn inverse_match_excludes_query() {
    let e = engine(
        "foo",
        ExactMatchingParam {
            inverse: true,
            case: CaseMatching::Ignore,
            ..Default::default()
        },
    );
    // Inverse: items WITHOUT the query match.
    assert!(e.match_item(&"bar".to_string()).is_some());
    assert!(e.match_item(&"foo".to_string()).is_none());
}

#[test]
fn empty_query_matches_everything() {
    let e = engine("", ExactMatchingParam::default());
    let result = e.match_item(&"anything".to_string()).unwrap();
    assert_eq!(result.matched_range, MatchRange::ByteRange(0, 0));
}

#[test]
fn display_shows_query_and_inverse_marker() {
    let plain = engine(
        "foo",
        ExactMatchingParam {
            case: CaseMatching::Respect,
            ..Default::default()
        },
    );
    assert_eq!(format!("{plain}"), "(Exact|foo)");

    let inverse = engine(
        "foo",
        ExactMatchingParam {
            inverse: true,
            case: CaseMatching::Respect,
            ..Default::default()
        },
    );
    assert!(format!("{inverse}").starts_with("(Exact|!"));
}
