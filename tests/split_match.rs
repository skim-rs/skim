#[allow(dead_code)]
#[macro_use]
mod common;

// Test 1: Basic split match - query without delimiter matches before delimiter in item
insta_test!(insta_split_match_query_before_delimiter, ["foo:bar", "baz:qux", "foo:qux", "baz:foo", "fbaz:boo"], &["--split-match", ":"], {
    @snap;
    @type "foo";
    @snap;
});

// Test 2: Query with delimiter matches both parts
insta_test!(insta_split_match_both_parts, ["foo:bar", "baz:qux", "foo:qux"], &["--split-match", ":"], {
    @snap;
    @type "foo:bar";
    @snap;
});

// Test 3: Query with delimiter - empty before, match after
insta_test!(insta_split_match_empty_before, ["foo:bar", "baz:bar"], &["--split-match", ":"], {
    @snap;
    @type ":bar";
    @snap;
});

// Test 4: Query with delimiter - match before, empty after
insta_test!(insta_split_match_empty_after, ["foo:bar", "foo:qux"], &["--split-match", ":"], {
    @snap;
    @type "foo:";
    @snap;
});

// Test 5: Item without delimiter - query without delimiter matches whole item
insta_test!(insta_split_match_no_delimiter_in_item, ["foobar", "bazqux"], &["--split-match", ":"], {
    @snap;
    @type "foo";
    @snap;
});

// Test 6: Item without delimiter - query with delimiter doesn't match
insta_test!(insta_split_match_delimiter_in_query_not_item, ["foobar", "bazqux"], &["--split-match", ":"], {
    @snap;
    @type "foo:bar";
    @snap;
});

// Test 7: Multiple delimiters in item - only first one is used for splitting
insta_test!(insta_split_match_multiple_delimiters_in_item, ["a:b:c", "x:y:z", "a:bc:cd"], &["--split-match", ":"], {
    @snap;
    @type "a:b:c";
    @snap;
});

// Test 8: Custom delimiter (/)
insta_test!(insta_split_match_custom_delimiter, ["foo/bar", "baz/qux"], &["--split-match", "/"], {
    @snap;
    @type "foo/bar";
    @snap;
});

insta_test!(insta_split_match_or, ["a:bc", "x:yz", "z:ab"], &["--split-match", ":"], {
    @snap;
    @type "a:b | x:y";
    @snap;
});
