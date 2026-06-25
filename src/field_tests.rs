use super::FieldRange::*;
#[test]
fn test_parse_range() {
    assert_eq!(FieldRange::from_str("1"), Some(Single(1)));
    assert_eq!(FieldRange::from_str("-1"), Some(Single(-1)));

    assert_eq!(FieldRange::from_str("1.."), Some(RightInf(1)));
    assert_eq!(FieldRange::from_str("-1.."), Some(RightInf(-1)));

    assert_eq!(FieldRange::from_str("..1"), Some(LeftInf(1)));
    assert_eq!(FieldRange::from_str("..-1"), Some(LeftInf(-1)));

    assert_eq!(FieldRange::from_str("1..3"), Some(Both(1, 3)));
    assert_eq!(FieldRange::from_str("-1..-3"), Some(Both(-1, -3)));

    assert_eq!(FieldRange::from_str(".."), Some(RightInf(0)));
    assert_eq!(FieldRange::from_str("a.."), None);
    assert_eq!(FieldRange::from_str("..b"), None);
    assert_eq!(FieldRange::from_str("a..b"), None);
}

use regex::Regex;

#[test]
fn test_parse_field_range() {
    assert_eq!(Single(0).to_index_pair(10), None);
    assert_eq!(Single(1).to_index_pair(10), Some((0, 1)));
    assert_eq!(Single(10).to_index_pair(10), Some((9, 10)));
    assert_eq!(Single(11).to_index_pair(10), None);
    assert_eq!(Single(-1).to_index_pair(10), Some((9, 10)));
    assert_eq!(Single(-10).to_index_pair(10), Some((0, 1)));
    assert_eq!(Single(-11).to_index_pair(10), None);

    assert_eq!(LeftInf(0).to_index_pair(10), None);
    assert_eq!(LeftInf(1).to_index_pair(10), Some((0, 1)));
    assert_eq!(LeftInf(8).to_index_pair(10), Some((0, 8)));
    assert_eq!(LeftInf(10).to_index_pair(10), Some((0, 10)));
    assert_eq!(LeftInf(11).to_index_pair(10), Some((0, 10)));
    assert_eq!(LeftInf(-1).to_index_pair(10), Some((0, 10)));
    assert_eq!(LeftInf(-8).to_index_pair(10), Some((0, 3)));
    assert_eq!(LeftInf(-9).to_index_pair(10), Some((0, 2)));
    assert_eq!(LeftInf(-10).to_index_pair(10), Some((0, 1)));
    assert_eq!(LeftInf(-11).to_index_pair(10), None);

    assert_eq!(RightInf(0).to_index_pair(10), Some((0, 10)));
    assert_eq!(RightInf(1).to_index_pair(10), Some((0, 10)));
    assert_eq!(RightInf(8).to_index_pair(10), Some((7, 10)));
    assert_eq!(RightInf(10).to_index_pair(10), Some((9, 10)));
    assert_eq!(RightInf(11).to_index_pair(10), None);
    assert_eq!(RightInf(-1).to_index_pair(10), Some((9, 10)));
    assert_eq!(RightInf(-8).to_index_pair(10), Some((2, 10)));
    assert_eq!(RightInf(-9).to_index_pair(10), Some((1, 10)));
    assert_eq!(RightInf(-10).to_index_pair(10), Some((0, 10)));
    assert_eq!(RightInf(-11).to_index_pair(10), Some((0, 10)));

    assert_eq!(Both(0, 0).to_index_pair(10), None);
    assert_eq!(Both(0, 1).to_index_pair(10), Some((0, 1)));
    assert_eq!(Both(0, 10).to_index_pair(10), Some((0, 10)));
    assert_eq!(Both(0, 11).to_index_pair(10), Some((0, 10)));
    assert_eq!(Both(1, -1).to_index_pair(10), Some((0, 10)));
    assert_eq!(Both(1, -9).to_index_pair(10), Some((0, 2)));
    assert_eq!(Both(1, -10).to_index_pair(10), Some((0, 1)));
    assert_eq!(Both(1, -11).to_index_pair(10), None);
    assert_eq!(Both(-9, -9).to_index_pair(10), Some((1, 2)));
    assert_eq!(Both(-9, -8).to_index_pair(10), Some((1, 3)));
    assert_eq!(Both(-9, 0).to_index_pair(10), None);
    assert_eq!(Both(-9, 1).to_index_pair(10), None);
    assert_eq!(Both(-9, 2).to_index_pair(10), Some((1, 2)));
    assert_eq!(Both(-1, 0).to_index_pair(10), None);
    assert_eq!(Both(11, 20).to_index_pair(10), None);
    assert_eq!(Both(-11, -11).to_index_pair(10), None);
}

#[test]
fn test_to_index_pair_zero_length() {
    // With no fields at all, every range variant must yield `None` rather than
    // panicking or producing an out-of-bounds pair (the `length == 0` guards).
    assert_eq!(Single(1).to_index_pair(0), None);
    assert_eq!(LeftInf(1).to_index_pair(0), None);
    assert_eq!(RightInf(1).to_index_pair(0), None);
    assert_eq!(Both(1, 2).to_index_pair(0), None);
}

#[test]
fn test_parse_transform_fields() {
    // delimiter is ","
    let re = Regex::new(",").unwrap();

    assert_eq!(
        super::parse_transform_fields(&re, "A,B,C,D,E,F", &[Single(2), Single(4), Single(-1), Single(-7)]),
        "B,D,F"
    );

    assert_eq!(
        super::parse_transform_fields(&re, "A,B,C,D,E,F", &[LeftInf(3), LeftInf(-6), LeftInf(-7)]),
        "A,B,C,A,"
    );

    assert_eq!(
        super::parse_transform_fields(
            &re,
            "A,B,C,D,E,F",
            &[RightInf(5), RightInf(-2), RightInf(-1), RightInf(8)]
        ),
        "E,FE,FF"
    );

    assert_eq!(
        super::parse_transform_fields(
            &re,
            "A,B,C,D,E,F",
            &[Both(3, 3), Both(-9, 2), Both(6, 10), Both(-9, -5)]
        ),
        "C,A,B,FA,B,"
    );
}

#[test]
fn test_parse_matching_fields() {
    // delimiter is ","
    let re = Regex::new(",").unwrap();

    // bytes:3  3  3 3
    //       中,华,人,民,E,F",

    assert_eq!(
        super::parse_matching_fields(&re, "中,华,人,民,E,F", &[Single(2), Single(4), Single(-1), Single(-7)]),
        vec![(4, 8), (12, 16), (18, 19)]
    );

    assert_eq!(
        super::parse_matching_fields(&re, "中,华,人,民,E,F", &[LeftInf(3), LeftInf(-6), LeftInf(-7)]),
        vec![(0, 12), (0, 4)]
    );

    assert_eq!(
        super::parse_matching_fields(
            &re,
            "中,华,人,民,E,F",
            &[RightInf(5), RightInf(-2), RightInf(-1), RightInf(7)]
        ),
        vec![(16, 19), (16, 19), (18, 19)]
    );

    assert_eq!(
        super::parse_matching_fields(
            &re,
            "中,华,人,民,E,F",
            &[Both(3, 3), Both(-8, 2), Both(6, 10), Both(-8, -5)]
        ),
        vec![(8, 12), (0, 8), (18, 19), (0, 8)]
    );
}

use super::*;

#[test]
fn test_null_delimiter() {
    // Test with null byte delimiter
    let re = Regex::new("\x00").unwrap();
    let text = "a\x00b\x00c";

    // Test field extraction
    assert_eq!(get_string_by_field(&re, text, &Single(1)), Some("a"));
    assert_eq!(get_string_by_field(&re, text, &Single(2)), Some("b"));
    assert_eq!(get_string_by_field(&re, text, &Single(3)), Some("c"));

    // Test matching fields - ranges include the delimiter after the field
    // text bytes: a(0), \0(1), b(2), \0(3), c(4)
    // Field 2 is "b" at byte 2, range includes delimiter at byte 3, so (2, 4)
    assert_eq!(parse_matching_fields(&re, text, &[Single(2)]), vec![(2, 4)]);

    // Field 1 is "a" at byte 0, range includes delimiter at byte 1, so (0, 2)
    // Field 3 is "c" at byte 4, no delimiter after it, so (4, 5)
    assert_eq!(
        parse_matching_fields(&re, text, &[Single(1), Single(3)]),
        vec![(0, 2), (4, 5)]
    );
}

#[test]
fn test_get_string_by_field() {
    // delimiter is ","
    let re = Regex::new(",").unwrap();
    let text = "a,b,c,";
    assert_eq!(get_string_by_field(&re, text, &Single(0)), None);
    assert_eq!(get_string_by_field(&re, text, &Single(1)), Some("a"));
    assert_eq!(get_string_by_field(&re, text, &Single(2)), Some("b"));
    assert_eq!(get_string_by_field(&re, text, &Single(3)), Some("c"));
    assert_eq!(get_string_by_field(&re, text, &Single(4)), Some(""));
    assert_eq!(get_string_by_field(&re, text, &Single(5)), None);
    assert_eq!(get_string_by_field(&re, text, &Single(6)), None);
    assert_eq!(get_string_by_field(&re, text, &Single(-1)), Some(""));
    assert_eq!(get_string_by_field(&re, text, &Single(-2)), Some("c"));
    assert_eq!(get_string_by_field(&re, text, &Single(-3)), Some("b"));
    assert_eq!(get_string_by_field(&re, text, &Single(-4)), Some("a"));
    assert_eq!(get_string_by_field(&re, text, &Single(-5)), None);
    assert_eq!(get_string_by_field(&re, text, &Single(-6)), None);

    assert_eq!(get_string_by_field(&re, text, &LeftInf(0)), None);
    assert_eq!(get_string_by_field(&re, text, &LeftInf(1)), Some("a"));
    assert_eq!(get_string_by_field(&re, text, &LeftInf(2)), Some("a,b"));
    assert_eq!(get_string_by_field(&re, text, &LeftInf(3)), Some("a,b,c"));
    assert_eq!(get_string_by_field(&re, text, &LeftInf(4)), Some("a,b,c,"));
    assert_eq!(get_string_by_field(&re, text, &LeftInf(5)), Some("a,b,c,"));
    assert_eq!(get_string_by_field(&re, text, &LeftInf(-5)), None);
    assert_eq!(get_string_by_field(&re, text, &LeftInf(-4)), Some("a"));
    assert_eq!(get_string_by_field(&re, text, &LeftInf(-3)), Some("a,b"));
    assert_eq!(get_string_by_field(&re, text, &LeftInf(-2)), Some("a,b,c"));
    assert_eq!(get_string_by_field(&re, text, &LeftInf(-1)), Some("a,b,c,"));

    assert_eq!(get_string_by_field(&re, text, &RightInf(0)), Some("a,b,c,"));
    assert_eq!(get_string_by_field(&re, text, &RightInf(1)), Some("a,b,c,"));
    assert_eq!(get_string_by_field(&re, text, &RightInf(2)), Some("b,c,"));
    assert_eq!(get_string_by_field(&re, text, &RightInf(3)), Some("c,"));
    assert_eq!(get_string_by_field(&re, text, &RightInf(4)), Some(""));
    assert_eq!(get_string_by_field(&re, text, &RightInf(5)), None);
    assert_eq!(get_string_by_field(&re, text, &RightInf(-5)), Some("a,b,c,"));
    assert_eq!(get_string_by_field(&re, text, &RightInf(-4)), Some("a,b,c,"));
    assert_eq!(get_string_by_field(&re, text, &RightInf(-3)), Some("b,c,"));
    assert_eq!(get_string_by_field(&re, text, &RightInf(-2)), Some("c,"));
    assert_eq!(get_string_by_field(&re, text, &RightInf(-1)), Some(""));

    assert_eq!(get_string_by_field(&re, text, &Both(0, 0)), None);
    assert_eq!(get_string_by_field(&re, text, &Both(0, 1)), Some("a"));
    assert_eq!(get_string_by_field(&re, text, &Both(0, 2)), Some("a,b"));
    assert_eq!(get_string_by_field(&re, text, &Both(0, 3)), Some("a,b,c"));
    assert_eq!(get_string_by_field(&re, text, &Both(0, 4)), Some("a,b,c,"));
    assert_eq!(get_string_by_field(&re, text, &Both(0, 5)), Some("a,b,c,"));
    assert_eq!(get_string_by_field(&re, text, &Both(1, 1)), Some("a"));
    assert_eq!(get_string_by_field(&re, text, &Both(1, 2)), Some("a,b"));
    assert_eq!(get_string_by_field(&re, text, &Both(1, 3)), Some("a,b,c"));
    assert_eq!(get_string_by_field(&re, text, &Both(1, 4)), Some("a,b,c,"));
    assert_eq!(get_string_by_field(&re, text, &Both(1, 5)), Some("a,b,c,"));
    assert_eq!(get_string_by_field(&re, text, &Both(2, 5)), Some("b,c,"));
    assert_eq!(get_string_by_field(&re, text, &Both(3, 5)), Some("c,"));
    assert_eq!(get_string_by_field(&re, text, &Both(4, 5)), Some(""));
    assert_eq!(get_string_by_field(&re, text, &Both(5, 5)), None);
    assert_eq!(get_string_by_field(&re, text, &Both(6, 5)), None);
    assert_eq!(get_string_by_field(&re, text, &Both(2, 3)), Some("b,c"));
    assert_eq!(get_string_by_field(&re, text, &Both(3, 3)), Some("c"));
    assert_eq!(get_string_by_field(&re, text, &Both(4, 3)), None);
}

#[test]
fn test_get_string_by_range() {
    let re = Regex::new(",").unwrap();
    let text = "a,b,c";
    // Parses the range string then extracts the matching field(s).
    assert_eq!(get_string_by_range(&re, text, "1"), Some("a"));
    assert_eq!(get_string_by_range(&re, text, "2.."), Some("b,c"));
    assert_eq!(get_string_by_range(&re, text, "..2"), Some("a,b"));
    // An unparsable range yields None.
    assert_eq!(get_string_by_range(&re, text, "not-a-range"), None);
}
