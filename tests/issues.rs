#[allow(dead_code)]
#[macro_use]
mod common;

insta_test!(issue_359_multi_regex_unicode, ["ああa"], &["--regex", "-q", "a"], {
  @snap;
});

insta_test!(issue_361_literal_space_control, ["foo  bar", "foo bar"], &["-q", "foo\\ bar"], {
    @snap;
});

insta_test!(issue_361_literal_space_invert, ["foo  bar", "foo bar"], &["-q", "!foo\\ bar"], {
    @snap;
});

insta_test!(issue_547_null_match, ["\0Test Test Test"], &[], {
    @snap;
    @type "Test";
    @snap;
});

insta_test!(issue_929_double_width_chars, [""], &["-q", "中文测试"], {
    @snap;
    @char '|';
    @snap;
});
