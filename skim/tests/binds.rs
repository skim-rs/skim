#[allow(dead_code)]
mod common;

use common::Keys::*;
use common::TmuxController;
use common::sk;
use std::io::Result;

fn setup(input: &str, opts: &[&str]) -> Result<TmuxController> {
    let tmux = TmuxController::new()?;
    let _ = tmux.start_sk(Some(&format!("echo -n -e '{input}'")), opts)?;
    tmux.until(|l| l[0].starts_with(">"))?;
    Ok(tmux)
}

#[test]
fn bind_execute_0_results() -> Result<()> {
    let tmux = TmuxController::new()?;
    let outfile = tmux.start_sk(Some("echo -n ''"), &["--bind", "'ctrl-f:execute(echo foo{})'"])?;
    tmux.until(|l| l[0] == ">")?;

    tmux.send_keys(&[Ctrl(&Key('f')), Enter])?;
    tmux.until(|l| l[0] != ">")?;

    let output = tmux.output(&outfile)?;
    println!("Output: {output:?}");
    assert_eq!(output[0], "foo");

    Ok(())
}

#[test]
fn bind_execute_0_results_noref() -> Result<()> {
    let tmux = TmuxController::new()?;
    let outfile = tmux.start_sk(Some("echo -n ''"), &["--bind", "'ctrl-f:execute(echo foo)'"])?;
    tmux.until(|l| l[0] == ">")?;

    tmux.send_keys(&[Ctrl(&Key('f')), Enter])?;
    tmux.until(|l| l[0] != ">")?;

    let output = tmux.output(&outfile)?;
    println!("Output: {output:?}");
    assert_eq!(output[0], "foo");

    Ok(())
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
    tmux.until(|l| l.len() > 2 && l[2] == "> a")?;

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

#[test]
fn bind_first_last() -> Result<()> {
    let tmux = setup(
        "1\\n2\\n3\\n4\\n5\\n6\\n7\\n8\\n9\\n10",
        &["--bind", "'ctrl-f:first,ctrl-l:last'"],
    )?;

    // Wait for items to load
    tmux.until(|l| l.len() > 10)?;

    // Jump to first item (1)
    tmux.send_keys(&[Ctrl(&Key('f'))])?;
    tmux.until(|l| l.iter().any(|line| line == "> 1"))?;

    // Jump to last item (10)
    tmux.send_keys(&[Ctrl(&Key('l'))])?;
    tmux.until(|l| l.iter().any(|line| line == "> 10"))?;

    // Jump back to first item (1)
    tmux.send_keys(&[Ctrl(&Key('f'))])?;
    tmux.until(|l| l.iter().any(|line| line == "> 1"))?;

    Ok(())
}

#[test]
fn bind_top_alias() -> Result<()> {
    let tmux = setup(
        "1\\n2\\n3\\n4\\n5\\n6\\n7\\n8\\n9\\n10",
        &["--bind", "'ctrl-t:top,ctrl-l:last'"],
    )?;

    // Wait for items to load
    tmux.until(|l| l.len() > 10)?;

    // Jump to last item (10)
    tmux.send_keys(&[Ctrl(&Key('l'))])?;
    tmux.until(|l| l.iter().any(|line| line == "> 10"))?;

    // Jump to top (first) item (1) using 'top' alias
    tmux.send_keys(&[Ctrl(&Key('t'))])?;
    tmux.until(|l| l.iter().any(|line| line == "> 1"))?;

    Ok(())
}

#[test]
fn bind_change() -> Result<()> {
    let tmux = setup(
        "1\\n12\\n13\\n14\\n15\\n16\\n17\\n18\\n19\\n10",
        &["--bind", "'change:first'"],
    )?;
    // Wait for items to load
    tmux.until(|l| l.len() > 10)?;

    tmux.send_keys(&[Up, Up])?;

    tmux.until(|l| l.iter().any(|x| x.starts_with("> 13")))?;
    tmux.send_keys(&[Key('1')])?;
    tmux.until(|l| l[2].starts_with("> 1"))?;

    Ok(())
}
