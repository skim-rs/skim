#[allow(dead_code)]
mod common;

use common::TmuxController;
use std::io::Result;

#[test]
fn preview_preserve_quotes() -> Result<()> {
    let tmux = TmuxController::new()?;
    tmux.start_sk(Some("echo \"'\\\"ABC\\\"'\""), &["--preview", "\"echo X{}X\""])?;

    tmux.until(|l| l.iter().any(|s| s.contains("X'\"ABC\"'")))
}

#[test]
fn preview_nul_char() -> Result<()> {
    let tmux = TmuxController::new()?;
    tmux.start_sk(Some("echo -ne 'a\\0b'"), &["--preview", "'echo -en {} | hexdump -C'"])?;
    tmux.until(|l| l[0].starts_with(">"))?;
    tmux.until(|l| l.iter().any(|s| s.contains("61 00 62")))
}

#[test]
fn preview_offset_fixed() -> Result<()> {
    let tmux = TmuxController::new()?;
    tmux.start_sk(
        Some("echo -ne 'a\\nb'"),
        &["--preview", "'seq 1000'", "--preview-window", "left:+123"],
    )?;
    tmux.until(|l| l[l.len() - 1].starts_with("123"))?;
    tmux.until(|l| l[l.len() - 1].contains("123/1000"))?;

    Ok(())
}
#[test]
fn preview_offset_expr() -> Result<()> {
    let tmux = TmuxController::new()?;
    tmux.start_sk(
        Some("echo -ne '123 321'"),
        &["--preview", "'seq 1000'", "--preview-window", "left:+{2}"],
    )?;
    tmux.until(|l| l[l.len() - 1].starts_with("321"))?;
    tmux.until(|l| l[l.len() - 1].contains("321/1000"))?;

    Ok(())
}
#[test]
fn preview_offset_fiexd_and_expr() -> Result<()> {
    let tmux = TmuxController::new()?;
    tmux.start_sk(
        Some("echo -ne '123 321'"),
        &["--preview", "'seq 1000'", "--preview-window", "left:+{2}-2"],
    )?;
    tmux.until(|l| l[l.len() - 1].starts_with("319"))?;
    tmux.until(|l| l[l.len() - 1].contains("319/1000"))?;

    Ok(())
}
