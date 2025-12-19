#[allow(dead_code)]
mod common;

use common::Keys::*;
use common::TmuxController;
use std::io::Result;

#[test]
fn issue_359_multi_regex_unicode() -> Result<()> {
    let tmux = TmuxController::new()?;
    tmux.start_sk(Some("echo 'ああa'"), &["--regex", "-q", "'a'"])?;
    tmux.until(|l| l[0] == "> a")?;

    tmux.until(|l| l.len() > 2 && l[2] == "> ああa")?;

    Ok(())
}

#[test]
fn issue_361_literal_space_control() -> Result<()> {
    let tmux = TmuxController::new()?;
    tmux.start_sk(Some("echo -ne 'foo  bar\\nfoo bar'"), &["-q", "'foo\\ bar'"])?;
    tmux.until(|l| l.len() == 4 && l[0].starts_with(">"))?;
    // The foo bar with a single space should have a better score
    tmux.until(|l| l.len() > 2 && l[2] == "> foo bar")?;

    Ok(())
}
#[test]
fn issue_361_literal_space_invert() -> Result<()> {
    let tmux = TmuxController::new()?;
    tmux.send_keys(&[Str("set +o histexpand"), Enter])?;
    tmux.start_sk(Some("echo -ne 'foo bar\\nfoo  bar'"), &["-q", "'!foo\\ bar'"])?;
    tmux.until(|l| l[0].starts_with(">"))?;
    tmux.until(|l| l.len() > 2 && l[2] == "> foo  bar")?;

    Ok(())
}

#[test]
fn issue_547_null_match() -> Result<()> {
    let tmux = TmuxController::new()?;
    tmux.send_keys(&[Str("set +o histexpand"), Enter])?;
    tmux.start_sk(Some("echo -e \"\\0Test Test Test\""), &["-q", "Test"])?;

    tmux.until(|l| l[0].starts_with(">"))?;
    tmux.until(|l| l[2].starts_with("> Test Test Test"))?;

    let out = tmux.capture_colored()?;
    println!("out {out:?}");
    assert!(
        out.len() > 2
            && out[2].starts_with(
                "\u{1b}[1m\u{1b}[38;5;168m\u{1b}[48;5;236m>\u{1b}[0m \u{1b}[38;5;151m\u{1b}[48;5;236mTest"
            )
    );

    Ok(())
}
