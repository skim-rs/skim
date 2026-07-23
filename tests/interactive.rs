#![allow(missing_docs, clippy::pedantic)]
// Gated to Linux: the Zellij harness (tests/common/zellij.rs) is written to be
// cross-platform, but the Zellij session only comes up reliably on the Linux CI
// runner. On the macOS and Windows runners `wait_ready` mostly times out with
// "pane not rendered yet" (the session's pane never renders under their PTY),
// so the e2e suite runs on Linux only for now.
// TODO(macos, windows): make the Zellij e2e harness render reliably in CI.
#![cfg(target_os = "linux")]
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
