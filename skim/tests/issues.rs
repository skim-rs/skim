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
