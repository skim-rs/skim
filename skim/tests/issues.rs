#[allow(dead_code)]
#[macro_use]
mod common;

use crate::common::Keys::*;

sk_test!(issue_359_multi_regex_unicode, @cmd "echo 'ああa'", &["--regex", "-q", "'a'"], {
  @capture[0] eq("> a");
  @capture[2] eq("> ああa");
});

sk_test!(issue_361_literal_space_control, "foo  bar\\nfoo bar", &["-q", "'foo\\ bar'"], {
  @lines |l| (l.len() == 4);
  @capture[0] starts_with(">");
  @capture[2] eq("> foo bar");
});

sk_test!(issue_361_literal_space_invert, "foo  bar\\nfoo bar", &["-q", "'!foo\\ bar'"], {
  @capture[0] starts_with(">");
  @capture[2] eq("> foo  bar");
});

sk_test!(issue_547_null_match, "\\0Test Test Test", &[], {
  @keys Str("Test");
  @capture[0] starts_with(">");
  @capture[2] starts_with("> Test Test Test");

  @capture_colored[2] starts_with("\u{1b}[1m\u{1b}[38;5;168m\u{1b}[48;5;236m>\u{1b}[0m \u{1b}[38;5;151m\u{1b}[48;5;236mTest");
});

sk_test!(issue_xxx_null_delimiter_with_nth, "a\\0b\\0c", &["--delimiter", "'\\x00'", "--with-nth", "2"], {
  @capture[0] starts_with(">");
  @capture[2] starts_with("> b");
});
sk_test!(issue_xxx_null_delimiter_nth, "a\\0b\\0c", &["--delimiter", "'\\x00'", "--nth", "2"], {
  @keys Key('c');
  @capture[0] starts_with("> c");
  @capture[1] contains("0/1");
  @keys BSpace, Key('b');
  @capture[0] starts_with("> b");
  @capture[2] starts_with("> abc");
});
