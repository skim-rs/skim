#![allow(missing_docs, clippy::pedantic)]
// Pure Zellij-harness e2e tests: they drive `sk` entirely through the terminal
// and depend on nothing OS-specific beyond the harness itself, which is
// cross-platform (see tests/common/zellij.rs). So these run on Linux, macOS and
// Windows.
#[allow(dead_code)]
#[macro_use]
mod common;
use common::zellij::Keys::*;

sk_test!(sk_version_long, "", &["--version"], {
  @output[0] starts_with("sk ");
});
sk_test!(sk_version_short, "", &["-V"], {
  @output[0] starts_with("sk ");
});

sk_test!(inline_clear_on_exit, @cmd "seq 1 10", &["--height=50%"], {
    @capture[0] starts_with(">");
    @keys Escape;
    @lines |l| (!l.iter().any(|line| line.starts_with(">")));
});

sk_test!(inline_clear_on_exit_reverse, @cmd "seq 1 10", &["--height=50%", "--layout=reverse"], {
    @capture[*] starts_with(">");
    @keys Escape;
    @lines |l| (!l.iter().any(|line| line.starts_with(">")));
});

sk_test!(inline_clear_on_exit_reverse_list, @cmd "seq 1 10", &["--height=50%", "--layout=reverse-list"], {
    @capture[*] starts_with(">");
    @keys Escape;
    @lines |l| (!l.iter().any(|line| line.starts_with(">")));
});

sk_test!(issue_1120_height_mode_clears_on_exit, @cmd "seq 1 10", &["--height=50%"], {
    @capture[0] starts_with(">");
    @keys Key('\x1b');
    @lines |l| (!l.iter().any(|line| line.starts_with(">")));
});
