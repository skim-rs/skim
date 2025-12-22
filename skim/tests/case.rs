#[allow(dead_code)]
#[macro_use]
mod common;

use common::Keys::*;
use common::TmuxController;
use std::io::Result;

sk_test!(case_smart_lower, "aBcDeF", &["--case", "smart"], @dsl {
  @ keys Str("abc");
  @ line 1 contains("1/1");
});

sk_test!(case_smart_exact, "aBcDeF", &["--case", "smart"], @dsl {
  @ keys Str("aBc");
  @ line 1 contains("1/1");
});

sk_test!(case_smart_no_match, "aBcDeF", &["--case", "smart"], @dsl {
  @ keys Str("Abc");
  @ line 1 contains("0/1");
});

sk_test!(case_ignore_lower, "aBcDeF", &["--case", "ignore"], @dsl {
  @ keys Str("abc");
  @ line 1 contains("1/1");
});

sk_test!(case_ignore_exact, "aBcDeF", &["--case", "ignore"], @dsl {
  @ keys Str("aBc");
  @ line 1 contains("1/1");
});

sk_test!(case_ignore_different, "aBcDeF", &["--case", "ignore"], @dsl {
  @ keys Str("Abc");
  @ line 1 contains("1/1");
});

sk_test!(case_ignore_no_match, "aBcDeF", &["--case", "ignore"], @dsl {
  @ keys Str("z");
  @ line 1 contains("0/1");
});

sk_test!(case_respect_lower, "aBcDeF", &["--case", "respect"], @dsl {
  @ keys Str("abc");
  @ line 1 contains("0/1");
});

sk_test!(case_respect_exact, "aBcDeF", &["--case", "respect"], @dsl {
  @ keys Str("aBc");
  @ line 1 contains("1/1");
});

sk_test!(case_respect_no_match, "aBcDeF", &["--case", "respect"], @dsl {
  @ keys Str("Abc");
  @ line 1 contains("0/1");
});
