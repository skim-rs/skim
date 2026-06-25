//! Non-interactive CLI integration tests.
//!
//! These spawn the real `sk` binary in modes that exit without a TTY (filter
//! mode, shell-completion, man-page, and the various `--print-*` / output
//! flags). Because the binary is the instrumented `llvm-cov-target` build under
//! coverage, they exercise `bin/main.rs` and `skim.rs`'s non-interactive paths.
//!
//! The binary is spawned directly (no shell), so the tests are cross-platform.
//! `env_clear()` is intentionally NOT used so that `LLVM_PROFILE_FILE` (set by
//! cargo-llvm-cov) is inherited by the child and its coverage is recorded; only
//! the `SKIM_*` vars are removed explicitly.
#![allow(missing_docs, clippy::pedantic)]

#[allow(dead_code)]
mod common;

use common::SK;
use std::io::Write;
use std::process::{Command, Stdio};

/// The bare binary path, derived from the shell-prefixed `SK` constant by
/// dropping the leading `SKIM_*= …` env assignments (which we now apply via
/// `env_remove` instead). On Windows `SK` has no prefix, so this is a no-op.
fn sk_bin() -> &'static str {
    SK.rsplit(' ').next().unwrap_or(SK)
}

/// Spawn the binary with explicit argv and env, feeding `pipe_input` on stdin.
/// Returns `(exit_code, stdout, stderr)`.
fn run_sk_argv(pipe_input: &str, argv: &[&str], envs: &[(&str, &str)]) -> (Option<i32>, String, String) {
    let mut cmd = Command::new(sk_bin());
    cmd.args(argv)
        .env_remove("SKIM_DEFAULT_OPTIONS")
        .env_remove("SKIM_DEFAULT_COMMAND")
        .env_remove("SKIM_OPTIONS_FILE")
        .stdin(if pipe_input.is_empty() {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in envs {
        cmd.env(k, v);
    }

    let mut child = cmd.spawn().expect("failed to spawn sk");
    if !pipe_input.is_empty() {
        // The callers write escapes (`\n`) the way `printf` once interpreted them.
        let input = pipe_input
            .replace("\\n", "\n")
            .replace("\\t", "\t")
            .replace("\\0", "\0");
        child
            .stdin
            .take()
            .expect("stdin piped")
            .write_all(input.as_bytes())
            .expect("write stdin");
    }
    let out = child.wait_with_output().expect("failed to wait on sk");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Convenience wrapper: tokenize a space-separated `args` string (shell-style)
/// and run with no extra env. Use [`run_sk_argv`] directly when an argument may
/// contain spaces (e.g. a temp-file path).
fn run_sk(pipe_input: &str, args: &str) -> (Option<i32>, String, String) {
    let argv = shlex::split(args).expect("args should tokenize");
    let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
    run_sk_argv(pipe_input, &refs, &[])
}

#[test]
fn filter_mode_prints_matches() {
    // `-f` runs filter mode: no TUI, matched lines printed to stdout, exit 0.
    let (code, stdout, _) = run_sk("apple\\nbanana\\ncherry", "-f a");
    assert_eq!(code, Some(0));
    // 'apple' and 'banana' contain 'a'; 'cherry' does not.
    assert!(stdout.contains("apple"));
    assert!(stdout.contains("banana"));
    assert!(!stdout.contains("cherry"));
}

#[test]
fn filter_mode_empty_query_matches_all() {
    let (code, stdout, _) = run_sk("one\\ntwo\\nthree", "-f ''");
    assert_eq!(code, Some(0));
    assert!(stdout.contains("one"));
    assert!(stdout.contains("two"));
    assert!(stdout.contains("three"));
}

#[test]
fn filter_mode_with_print_query() {
    // --print-query prepends the query line to the output.
    let (code, stdout, _) = run_sk("apple\\nbanana", "-f a --print-query");
    assert_eq!(code, Some(0));
    let mut lines = stdout.lines();
    assert_eq!(lines.next(), Some("a"));
}

#[test]
fn filter_mode_with_print0() {
    // --print0 separates output records with NUL instead of newline.
    let (code, stdout, _) = run_sk("apple\\nbanana", "-f a --print0");
    assert_eq!(code, Some(0));
    assert!(stdout.contains('\0'));
}

#[test]
fn select_1_with_output_format() {
    // --output-format renders the selected item through the printf branch.
    let (code, stdout, _) = run_sk("1\\n2\\n3", "--select-1 -q 3 --output-format '{}'");
    assert_eq!(code, Some(0));
    assert!(stdout.contains('3'));
}

#[test]
fn filter_mode_output_format_current_item_is_empty() {
    // `{}` expands to the *current* (highlighted) item, just like in previews.
    // Filter mode has no interactive cursor, so there is no current item and the
    // token expands to nothing — only the trailing record separator is emitted.
    let (code, stdout, _) = run_sk("apple\\nbanana", "-f a --output-format '{}'");
    assert_eq!(code, Some(0));
    assert!(
        stdout.trim().is_empty(),
        "`{{}}` has no current item in filter mode (got {stdout:?})"
    );
}

#[test]
fn filter_mode_output_format_all_items_token() {
    // `{+}` expands to every matched item, so it works in filter mode where
    // there is no single current item.
    let (code, stdout, _) = run_sk("apple\\nbanana\\ncherry", "-f a --output-format '{+}'");
    assert_eq!(code, Some(0));
    assert!(stdout.contains("apple"), "got {stdout:?}");
    assert!(stdout.contains("banana"), "got {stdout:?}");
    assert!(
        !stdout.contains("cherry"),
        "non-matching item must be excluded (got {stdout:?})"
    );
}

#[test]
fn select_1_writes_history_file() {
    use std::io::Read;
    // A history file records the query on exit (covers write_history_to_file in
    // the real binary).
    let hist = std::env::temp_dir().join(format!("sk_hist_{}", std::process::id()));
    let hist_path = hist.to_str().unwrap();
    // Pass argv explicitly: the temp path may contain spaces on some platforms.
    let (code, _stdout, _) = run_sk_argv("1\\n2\\n3", &["--select-1", "-q", "3", "--history", hist_path], &[]);
    assert_eq!(code, Some(0));
    let mut contents = String::new();
    std::fs::File::open(&hist)
        .expect("history file should exist")
        .read_to_string(&mut contents)
        .unwrap();
    assert!(contents.contains('3'));
    let _ = std::fs::remove_file(&hist);
}

#[test]
fn select_1_print_current() {
    // --print-current prints the current item line before the selected items.
    let (code, stdout, _) = run_sk("1\\n2\\n3", "--select-1 -q 3 --print-current");
    assert_eq!(code, Some(0));
    assert!(stdout.contains('3'));
}

#[test]
fn log_file_initializes_logger() {
    // --log-file routes env_logger to a file (covers init_logger's Pipe target
    // and builder). SKIM_LOG=trace makes the run actually emit records.
    let log = std::env::temp_dir().join(format!("sk_log_{}", std::process::id()));
    let log_path = log.to_str().unwrap();
    let (code, _stdout, _) = run_sk_argv(
        "1\\n2\\n3",
        &["--select-1", "-q", "3", "--log-file", log_path],
        &[("SKIM_LOG", "trace")],
    );
    assert_eq!(code, Some(0));
    // The log file was created by the file target.
    assert!(log.exists());
    let _ = std::fs::remove_file(&log);
}

#[test]
fn shell_completion_bash() {
    // --shell bash generates a completion script and exits 0 without reading stdin.
    let (code, stdout, _) = run_sk("", "--shell bash");
    assert_eq!(code, Some(0));
    assert!(!stdout.is_empty());
}

#[test]
fn shell_completion_with_key_bindings() {
    // --shell zsh together with --shell-bindings emits bindings too.
    let (code, stdout, _) = run_sk("", "--shell zsh --shell-bindings");
    assert_eq!(code, Some(0));
    assert!(!stdout.is_empty());
}

#[test]
fn man_page_generation() {
    // --man writes the man page to stdout and exits 0.
    let (code, stdout, _) = run_sk("", "--man");
    assert_eq!(code, Some(0));
    assert!(stdout.contains(".TH") || stdout.to_lowercase().contains("skim"));
}

#[test]
fn select_1_prints_all_metadata_flags() {
    // A single matching item with select-1 exits without the TUI and prints all
    // the requested metadata lines (query, cmd, header, score).
    let (code, stdout, _) = run_sk(
        "1\\n2\\n3",
        "--select-1 -q 3 --print-query --print-cmd --print-header --print-score",
    );
    assert_eq!(code, Some(0));
    assert!(stdout.contains('3'));
}

#[test]
fn version_flag_exits_zero() {
    let (code, stdout, _) = run_sk("", "--version");
    assert_eq!(code, Some(0));
    assert!(stdout.to_lowercase().contains("sk") || !stdout.is_empty());
}

#[test]
fn help_flag_exits_zero() {
    let (code, stdout, _) = run_sk("", "--help");
    assert_eq!(code, Some(0));
    assert!(!stdout.is_empty());
}

#[test]
fn invalid_flag_exits_with_error() {
    // An unknown flag makes clap print usage and exit non-zero (main()'s
    // `from_env` error path).
    let (code, _stdout, stderr) = run_sk("", "--definitely-not-a-real-flag");
    assert_ne!(code, Some(0));
    assert!(!stderr.is_empty());
}
