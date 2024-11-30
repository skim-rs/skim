use e2e::test_utils::sk;
use e2e::test_utils::Keys::*;
use e2e::test_utils::TmuxController;
use std::io::Result;

fn setup(input: &str, opts: &[&str]) -> Result<TmuxController> {
    let tmux = TmuxController::new()?;
    let _ = tmux.start_sk(Some(&format!("echo -n -e '{input}'")), opts)?;
    tmux.until(|l| l[0].starts_with(">"))?;
    Ok(tmux)
}

#[test]
fn bind_execute_0_results() -> Result<()> {
    let tmux = setup("", &["--bind", "'enter:execute(less {})'"])?;

    tmux.send_keys(&[Enter])?;
    tmux.until(|l| l[0] == ">")?;

    tmux.send_keys(&[Key('q')])?;
    tmux.until(|l| l[0] == "> q")
}

#[test]
fn bind_execute_0_results_noref() -> Result<()> {
    let tmux = setup("", &["--bind", "'enter:execute(less)'"])?;

    tmux.send_keys(&[Enter, Key('q'), Key('g')])?;

    tmux.until(|l| l[0] == "> g")
}

#[test]
fn bind_if_non_matched() -> Result<()> {
    let tmux = setup(
        "a\nb",
        &["--bind", "'enter:if-non-matched(backward-delete-char)'", "-q", "ab"],
    )?;

    tmux.until(|l| l[0].starts_with(">"))?;
    tmux.until(|l| l[0].starts_with("> ab"))?;

    tmux.send_keys(&[Enter])?;
    tmux.until(|l| l[0] == "> a")?;

    tmux.send_keys(&[Enter, Key('c')])?;
    tmux.until(|l| l[0].starts_with("> ac"))?;

    Ok(())
}

#[test]
fn bind_append_and_select() -> Result<()> {
    let tmux = setup("a\\n\\nb\\nc", &["-m", "--bind", "'ctrl-f:append-and-select'"])?;

    tmux.send_keys(&[Str("xyz"), Ctrl(&Key('f'))])?;
    tmux.until(|l| l.len() > 2 && l[2] == ">>xyz")?;

    Ok(())
}

#[test]
fn bind_reload_no_arg() -> Result<()> {
    let tmux = TmuxController::new()?;

    let outfile = tmux.tempfile()?;
    let sk_cmd = sk(&outfile, &["--bind", "'ctrl-a:reload'"])
        .replace("SKIM_DEFAULT_COMMAND=", "SKIM_DEFAULT_COMMAND='echo hello'");
    tmux.send_keys(&[Str(&sk_cmd), Enter])?;
    tmux.until(|l| l[0].starts_with(">"))?;

    tmux.send_keys(&[Ctrl(&Key('a'))])?;
    tmux.until(|l| l.len() > 2 && l[2] == "> hello")?;

    Ok(())
}

#[test]
fn bind_reload_cmd() -> Result<()> {
    let tmux = setup("a\\n\\nb\\nc", &["--bind", "'ctrl-a:reload(echo hello)'"])?;

    tmux.until(|l| l.len() > 2 && l[2] == "> a")?;
    tmux.send_keys(&[Ctrl(&Key('a'))])?;
    tmux.until(|l| l.len() > 2 && l[2] == "> hello")?;

    Ok(())
}
