#![allow(missing_docs, clippy::pedantic)]
#![cfg(unix)]
#[allow(dead_code)]
#[macro_use]
mod common;
use common::tmux::Keys::*;

// NOTE: many former tmux tests have been migrated off this file:
//   - display / NUL / pre-select / flag-parsing -> tests/options.rs (in-process)
//   - SKIM_DEFAULT_COMMAND / SKIM_DEFAULT_OPTIONS / SKIM_OPTIONS_FILE ->
//     `SkimOptions::merge_args_and_parse` unit tests in src/options_tests.rs
//   - output serialization (--print-*) -> `SkimOutput::write_output` unit tests
// Only tests that need a real process/terminal (shell execute/reload, height
// clear-on-exit, the binary's own stdout) remain here.

sk_test!(tmux_version_long, "", &["--version"], {
  @output[0] starts_with("sk ");
});
sk_test!(tmux_version_short, "", &["-V"], {
  @output[0] starts_with("sk ");
});

// NOTE: the --print-query / --print-cmd / --print-header / --print-score /
// --print0 tests moved to unit/integration coverage that does not need tmux:
//   - serialization: `SkimOutput::write_output` unit tests (src/output.rs)
//   - the real binary's --print-* output: tests/cli.rs (filter / select-1 mode)
//   - interactive multi-select -> output: the `multi_selection_flows_into_output`
//     unit test in src/skim_tests.rs
//   - NUL passthrough (--ansi "a\0b"): write_output `strip_ansi_keeps_nul_*` unit
//   - accept-with-arg (ctrl-a:accept:hello): binds_tests parse +
//     write_output `accept_key_is_written_before_items` unit

sk_test!(opt_disable_pattern_control, "foo\\nbar", &["--disable-pattern", "foo", "--query bar"], {
    @capture[0] starts_with("> bar");
    @capture[2] starts_with("> bar");
    @keys Enter;
    @output[0] trim().eq("bar");
});
sk_test!(opt_disable_pattern, "foo\\nbar", &["--disable-pattern", "foo", "--query foo"], {
    @capture[0] starts_with("> foo");
    @capture[2] starts_with("> foo");
    @keys Enter;
    @output[0] trim().eq("");
});
sk_test!(opt_disable_pattern_multi, "foo a\\nbar\\nfoo b", &["--disable-pattern", "foo", "--multi", "-q", "a"], {
    @capture[0] starts_with("> a");
    @lines |l| (l.len() == 4);
    @capture[2] starts_with("> foo a");
    @capture[3] starts_with("bar");
    @keys BTab, BTab;
    @capture[2] trim().eq("foo a");
    @capture[3] trim().eq(">>bar");
    @keys Enter;
    @output[0] trim().eq("bar");
});

// NOTE: bind execute/reload (ctrl-f:execute(...), ctrl-a:reload[(...)]) moved to
// cross-platform coverage: bind parsing in src/binds_tests.rs, action dispatch in
// src/tui/app_tests.rs (execute_action_runs_command, execute_silent_spawns_*,
// reload_actions_emit_reload, refresh_cmd_reloads_in_interactive_mode).

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
