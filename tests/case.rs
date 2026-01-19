#[allow(dead_code)]
#[macro_use]
mod common;

use common::Keys::*;

sk_test!(case_smart_lower, "aBcDeF", &["--case", "smart"], {
  @capture[0] starts_with(">");
  @keys Str("abc");
  @capture[1] contains("1/1");
});

sk_test!(case_smart_exact, "aBcDeF", &["--case", "smart"], {
  @capture[0] starts_with(">");
  @keys Str("aBc");
  @capture[1] contains("1/1");
});

sk_test!(case_smart_no_match, "aBcDeF", &["--case", "smart"], {
  @capture[0] starts_with(">");
  @keys Str("Abc");
  @capture[1] contains("0/1");
});

sk_test!(case_ignore_lower, "aBcDeF", &["--case", "ignore"], {
  @capture[0] starts_with(">");
  @keys Str("abc");
  @capture[1] contains("1/1");
});

sk_test!(case_ignore_exact, "aBcDeF", &["--case", "ignore"], {
  @capture[0] starts_with(">");
  @keys Str("aBc");
  @capture[1] contains("1/1");
});

sk_test!(case_ignore_different, "aBcDeF", &["--case", "ignore"], {
  @capture[0] starts_with(">");
  @keys Str("Abc");
  @capture[1] contains("1/1");
});

sk_test!(case_ignore_no_match, "aBcDeF", &["--case", "ignore"], {
  @capture[0] starts_with(">");
  @keys Str("z");
  @capture[1] contains("0/1");
});

sk_test!(case_respect_lower, "aBcDeF", &["--case", "respect"], {
  @capture[0] starts_with(">");
  @keys Str("abc");
  @capture[1] contains("0/1");
});

sk_test!(case_respect_exact, "aBcDeF", &["--case", "respect"], {
  @capture[0] starts_with(">");
  @keys Str("aBc");
  @capture[1] contains("1/1");
});

sk_test!(case_respect_no_match, "aBcDeF", &["--case", "respect"], {
  @capture[0] starts_with(">");
  @keys Str("Abc");
  @capture[1] contains("0/1");
});
