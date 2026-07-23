#![allow(missing_docs, clippy::pedantic)]
// The Zellij harness (tests/common/zellij.rs) is written to be cross-platform,
// and none of the checks below use unix-only APIs, so these tests are not gated
// to unix in principle. In practice they are skipped on Windows for now: under
// the Windows runner's ConPTY the Zellij session never renders (`dump-screen`
// stays empty and `wait_ready` times out with "pane not rendered yet"), and sk's
// escape-code disambiguation on Windows is a further known gap. Re-enable once
// the Zellij session comes up under ConPTY.
// TODO(windows): drive the Zellij e2e harness on Windows.
#![cfg(not(windows))]
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
