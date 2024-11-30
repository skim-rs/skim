use e2e::test_utils::Keys::*;
use e2e::test_utils::TmuxController;
use std::io::Result;

fn setup(case: &str) -> Result<TmuxController> {
    let tmux = TmuxController::new()?;
    let _ = tmux.start_sk(Some(&format!("echo -n -e 'aBcDeF'")), &["--case", case])?;
    tmux.until(|l| l[0].starts_with(">"))?;
    Ok(tmux)
}

#[test]
fn case_smart_lower() -> Result<()> {
    let tmux = setup("smart")?;

    tmux.send_keys(&[Str("abc")])?;
    tmux.until(|l| l[1].trim().starts_with("1/1"))
}
#[test]
fn case_smart_exact() -> Result<()> {
    let tmux = setup("smart")?;

    tmux.send_keys(&[Str("aBc")])?;
    tmux.until(|l| l[1].trim().starts_with("1/1"))
}
#[test]
fn case_smart_no_match() -> Result<()> {
    let tmux = setup("smart")?;

    tmux.send_keys(&[Str("Abc")])?;
    tmux.until(|l| l[1].trim().starts_with("0/1"))
}

#[test]
fn case_ignore_lower() -> Result<()> {
    let tmux = setup("ignore")?;

    tmux.send_keys(&[Str("abc")])?;
    tmux.until(|l| l[1].trim().starts_with("1/1"))
}
#[test]
fn case_ignore_exact() -> Result<()> {
    let tmux = setup("ignore")?;

    tmux.send_keys(&[Str("aBc")])?;
    tmux.until(|l| l[1].trim().starts_with("1/1"))
}
#[test]
fn case_ignore_different() -> Result<()> {
    let tmux = setup("ignore")?;

    tmux.send_keys(&[Str("Abc")])?;
    tmux.until(|l| l[1].trim().starts_with("1/1"))
}
#[test]
fn case_ignore_no_match() -> Result<()> {
    let tmux = setup("ignore")?;

    tmux.send_keys(&[Str("z")])?;
    tmux.until(|l| l[1].trim().starts_with("0/1"))
}

#[test]
fn case_respect_lower() -> Result<()> {
    let tmux = setup("respect")?;

    tmux.send_keys(&[Str("abc")])?;
    tmux.until(|l| l[1].trim().starts_with("0/1"))
}
#[test]
fn case_respect_exact() -> Result<()> {
    let tmux = setup("respect")?;

    tmux.send_keys(&[Str("aBc")])?;
    tmux.until(|l| l[1].trim().starts_with("1/1"))
}
#[test]
fn case_respect_no_match() -> Result<()> {
    let tmux = setup("respect")?;

    tmux.send_keys(&[Str("Abc")])?;
    tmux.until(|l| l[1].trim().starts_with("0/1"))
}
