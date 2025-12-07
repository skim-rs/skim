#[allow(dead_code)]
mod common;

use common::Keys::*;
use common::TmuxController;
use std::io::Result;

#[test]
fn test_ansi_flag_enabled() -> Result<()> {
    let tmux = TmuxController::new().unwrap();
    let _outfile = tmux
        .start_sk(
            Some("echo -e 'plain\\n\\x1b[31mred\\x1b[0m\\n\\x1b[32mgreen\\x1b[0m'"),
            &["--ansi"],
        )
        .unwrap();

    tmux.until(|lines| lines.iter().any(|line| line.contains("plain")))?;

    tmux.send_keys(&[Str("d")])?;

    tmux.until(|l| l.len() > 2 && l[2].starts_with("> red"))?;

    let colored_lines = tmux.capture_colored().unwrap();

    // With --ansi flag, ANSI codes should be parsed and not visible as raw text
    // Check that we don't see the raw ANSI escape sequences like \x1b[31m
    let full_output = colored_lines.join(" ");
    println!("{:?}", full_output);
    assert!(full_output.contains("\u{1b}[31mre"));

    tmux.send_keys(&[Enter])
}

#[test]
fn test_ansi_flag_disabled() -> Result<()> {
    let tmux = TmuxController::new().unwrap();
    let _outfile = tmux
        .start_sk(
            Some("echo -e 'plain\\n\\x1b[31mred\\x1b[0m\\n\\x1b[32mgreen\\x1b[0m'"),
            &[],
        )
        .unwrap();

    tmux.until(|lines| lines.iter().any(|line| line.contains("plain")))
        .unwrap();

    tmux.send_keys(&[Str("red")])?;

    tmux.until(|l| l.len() > 2 && l[2] == "> ?[31mred?[0m")?;

    tmux.send_keys(&[Enter])
}
