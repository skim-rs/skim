#![cfg(feature = "cli")]

use std::process::{Command, Stdio};

fn sk_bin() -> &'static str {
    env!("CARGO_BIN_EXE_sk")
}

fn emit_lines_cmd() -> &'static str {
    if cfg!(windows) {
        "echo a&echo b&echo c"
    } else {
        "printf 'a\\nb\\nc\\n'"
    }
}

#[test]
fn filter_mode_executes_cmd_and_prints_reversed_matches() {
    let cmd = emit_lines_cmd();
    let output = Command::new(sk_bin())
        .args([
            "--filter",
            ".",
            "--regex",
            "--tac",
            "--interactive",
            "--print-query",
            "--print-cmd",
            "--cmd",
            cmd,
        ])
        .env("SKIM_DEFAULT_OPTIONS", "")
        .env("SKIM_DEFAULT_COMMAND", "")
        .stdin(Stdio::null())
        .output()
        .expect("run sk");

    assert!(output.status.success(), "sk failed: status={:?}", output.status.code());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, format!(".\n{cmd}\nc\nb\na\n"));
}

#[test]
fn filter_mode_exit_code_is_one_on_no_match() {
    let cmd = emit_lines_cmd();
    let output = Command::new(sk_bin())
        .args(["--filter", "^z$", "--regex", "--interactive", "--cmd", cmd])
        .env("SKIM_DEFAULT_OPTIONS", "")
        .env("SKIM_DEFAULT_COMMAND", "")
        .stdin(Stdio::null())
        .output()
        .expect("run sk");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
}
