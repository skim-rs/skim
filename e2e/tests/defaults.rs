use e2e::test_utils::{sk, Keys, TmuxController};
use std::io::Result;

#[test]
fn vanilla() -> Result<()> {
    let tmux = TmuxController::new()?;
    let _ = tmux.start_sk(Some("seq 1 100000"), &[]);
    tmux.until(|l| l[0].starts_with(">") && l[1].starts_with("  100000"))?;
    let lines = tmux.capture()?;
    assert_eq!(lines[3], "  2");
    assert_eq!(lines[2], "> 1");
    assert!(lines[1].starts_with("  100000/100000"));
    assert!(lines[1].ends_with("0/0"));
    assert_eq!(lines[0], ">");

    Ok(())
}

#[test]
fn default_command() -> Result<()> {
    let tmux = TmuxController::new()?;

    let outfile = tmux.tempfile()?;
    let sk_cmd = sk(&outfile, &[]).replace("SKIM_DEFAULT_COMMAND=", "SKIM_DEFAULT_COMMAND='echo hello'");
    tmux.send_keys(&[Keys::Str(&sk_cmd), Keys::Enter])?;
    tmux.until(|l| l[0].starts_with(">"))?;
    tmux.until(|l| l[1].starts_with("  1/1"))?;
    tmux.until(|l| l[2] == "> hello")?;

    tmux.send_keys(&[Keys::Enter])?;
    tmux.until(|l| !l[0].starts_with(">"))?;

    let output = tmux.output(&outfile)?;

    assert_eq!(output[0], "hello");

    Ok(())
}

#[test]
fn version_long() -> Result<()> {
    let tmux = TmuxController::new()?;

    let outfile = tmux.tempfile()?;
    let sk_cmd = sk(&outfile, &["--version"]);
    tmux.send_keys(&[Keys::Str(&sk_cmd), Keys::Enter])?;

    let output = tmux.output(&outfile)?;

    assert!(output[0].starts_with("sk "));

    Ok(())
}

#[test]
fn version_short() -> Result<()> {
    let tmux = TmuxController::new()?;

    let outfile = tmux.tempfile()?;
    let sk_cmd = sk(&outfile, &["-V"]);
    tmux.send_keys(&[Keys::Str(&sk_cmd), Keys::Enter])?;

    let output = tmux.output(&outfile)?;

    assert!(output[0].starts_with("sk "));

    Ok(())
}
