#![allow(missing_docs, clippy::pedantic)]
#![cfg(unix)]
#[allow(dead_code)]
mod common;

use std::fs::{self, File, Permissions};
use std::io::{Read, Result, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use common::tmux::Keys::*;
use common::tmux::{TmuxController, wait};

/// Read the whole file at `path` into a `String`.
fn read_file(path: &Path) -> Result<String> {
    let mut s = String::new();
    File::open(path)?.read_to_string(&mut s)?;
    Ok(s)
}

/// Drive an interactive child through an `execute` action and assert it keeps
/// receiving keystrokes, then that skim itself is interactive again once the
/// child exits.
///
/// Two independent bugs used to make a program run via `execute(...)` freeze:
///   1. skim's own input reader kept reading the terminal while the child ran,
///      so skim and the child raced for keystrokes and roughly half were lost.
///   2. the child inherited skim's stdin, which is the item *pipe* here
///      (`printf … | sk`), so an interactive child had no keyboard at all.
///
/// The child below reads four raw single keystrokes and records them, in order.
/// It only completes if it received every keystroke, so if either bug regresses
/// the result file is never finished and the wait times out. Typing a query
/// afterwards confirms skim restarted its reader and repainted — the latter
/// exercised for both the fullscreen and inline layouts, since the post-execute
/// repaint path differs from a normal render.
fn run_interactive_execute(name: &str, extra_opts: &[&str]) -> Result<()> {
    let mut tmux = TmuxController::new_named(name)?;

    let dir = tmux.tempdir.path().to_path_buf();
    let script = dir.join("interactive.sh");
    let ready = dir.join("ready");
    let result = dir.join("keys.txt");

    // A tiny interactive "TUI": announce readiness, then read four raw single
    // keystrokes from the terminal and write them, in order, to the result
    // file. `read -rsn1` requires bash, which is present on the Linux and macOS
    // CI runners. If a keystroke is stolen, the loop blocks on `read` and the
    // result file is never written.
    let script_body = r#"#!/usr/bin/env bash
out="$1"
ready="$2"
: > "$ready"
s=""
for _ in 1 2 3 4; do
    IFS= read -rsn1 c || break
    s="$s$c"
done
printf '%s' "$s" > "$out"
"#;
    File::create(&script)?.write_all(script_body.as_bytes())?;
    fs::set_permissions(&script, Permissions::from_mode(0o755))?;

    let bind = format!(
        "--bind='enter:execute(bash {} {} {})'",
        script.display(),
        result.display(),
        ready.display()
    );
    let mut opts: Vec<&str> = extra_opts.to_vec();
    opts.push(bind.as_str());

    // skim reads its items from a *pipe*; Enter runs the interactive child.
    tmux.start_sk(Some("printf 'aaa\\nbbb\\nccc'"), &opts)?;

    // Wait for skim to come up (prompt line present).
    tmux.until(|l| l.iter().any(|s| s.trim_start().starts_with(">")))?;

    // Trigger the execute action and wait until the child has taken over the
    // terminal. Waiting for the readiness marker guarantees skim has already
    // suspended its own reader, so there is no race for the keystrokes below.
    tmux.send_keys(&[Enter])?;
    wait(|| {
        if ready.exists() {
            Ok(())
        } else {
            Err(std::io::Error::other("child not ready yet"))
        }
    })?;

    // Feed the child four distinct keystrokes.
    tmux.send_keys(&[Key('w'), Key('x'), Key('y'), Key('z')])?;

    // The child finishes only if it received every keystroke.
    wait(|| match read_file(&result) {
        Ok(s) if s == "wxyz" => Ok(()),
        _ => Err(std::io::Error::other("keys not complete yet")),
    })
    .map_err(|_| {
        std::io::Error::other(format!(
            "child did not receive all keystrokes; result file = {:?}",
            read_file(&result).ok()
        ))
    })?;

    // The child has exited: skim should have restarted its reader and repainted
    // (skim's stdout is redirected to a file by `start_sk`, which used to stall
    // the post-execute repaint). Typing a query must filter the piped items.
    tmux.send_keys(&[Str("aaa")])?;
    tmux.until(|l| l.iter().any(|s| s.contains("1/3")))?;

    Ok(())
}

#[test]
fn execute_interactive_child_keeps_receiving_keys_fullscreen() -> Result<()> {
    run_interactive_execute("execute_interactive_fs", &[])
}

#[test]
fn execute_interactive_child_keeps_receiving_keys_inline() -> Result<()> {
    run_interactive_execute("execute_interactive_inline", &["--height=40%"])
}
