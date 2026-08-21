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

/// An item exposing explicit matching ranges, as `--nth` produces.
struct RangedItem {
    text: String,
    ranges: Vec<(usize, usize)>,
}

impl SkimItem for RangedItem {
    fn text(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Borrowed(&self.text)
    }

    fn get_matching_ranges(&self) -> Option<&[(usize, usize)]> {
        Some(&self.ranges)
    }
}

#[test]
fn inverse_match_checks_every_matching_range() {
    // `--nth 1,2` over "foo bar" yields two ranges: "foo" and "bar".  An inverse
    // query `!foo` must reject the item because one of the ranges contains "foo",
    // even though the *first* range scanned may not.
    let e = engine(
        "foo",
        ExactMatchingParam {
            inverse: true,
            case: CaseMatching::Ignore,
            ..Default::default()
        },
    );

    let foo_in_first_range = RangedItem {
        text: "foo bar".to_string(),
        ranges: vec![(0, 3), (4, 7)],
    };
    assert!(
        e.match_item(&foo_in_first_range).is_none(),
        "item whose first field contains the query must not match an inverse query"
    );

    let foo_in_second_range = RangedItem {
        text: "bar foo".to_string(),
        ranges: vec![(0, 3), (4, 7)],
    };
    assert!(
        e.match_item(&foo_in_second_range).is_none(),
        "item whose second field contains the query must not match an inverse query"
    );

    let no_foo = RangedItem {
        text: "bar baz".to_string(),
        ranges: vec![(0, 3), (4, 7)],
    };
    assert!(
        e.match_item(&no_foo).is_some(),
        "item where no field contains the query must match an inverse query"
    );
}

#[test]
fn inverse_match_with_no_matching_range_does_not_match() {
    // Every `--nth` index out of range leaves the item with no range at all;
    // there is nothing to match against, so the item stays unmatched.
    let e = engine(
        "foo",
        ExactMatchingParam {
            inverse: true,
            case: CaseMatching::Ignore,
            ..Default::default()
        },
    );
    let item = RangedItem {
        text: "bar baz".to_string(),
        ranges: vec![],
    };
    assert!(e.match_item(&item).is_none());
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
