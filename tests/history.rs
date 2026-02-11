#[allow(dead_code)]
mod common;

use common::tmux::Keys::*;
use common::tmux::TmuxController;
use std::fs::File;
use std::io::Read;
use std::io::Result;
use std::io::Write;
use std::path::Path;

#[test]
fn query_history() -> Result<()> {
    let mut tmux = TmuxController::new()?;
    let histfile = tmux.tempfile()?;

    File::create(&histfile)?.write_all(b"a\nb\nc")?;

    tmux.start_sk(Some("echo -e -n 'a\\nb\\nc'"), &["--history", &histfile])?;
    tmux.until(|l| l[0].starts_with(">"))?;

    tmux.send_keys(&[Ctrl(&Key('p'))])?;
    tmux.until(|l| l[0].trim() == "> c")?;

    tmux.send_keys(&[Ctrl(&Key('p'))])?;
    tmux.until(|l| l[0].trim() == "> b")?;

    tmux.send_keys(&[Ctrl(&Key('p'))])?;
    tmux.until(|l| l[0].trim() == "> a")?;

    tmux.send_keys(&[Ctrl(&Key('n'))])?;
    tmux.until(|l| l[0].trim() == "> b")?;

    tmux.send_keys(&[Key('n')])?;
    tmux.until(|l| l[0].trim() == "> bn")?;

    tmux.send_keys(&[Enter])?;

    tmux.until(|_| {
        let mut buf = String::new();
        File::open(Path::new(&histfile))
            .unwrap()
            .read_to_string(&mut buf)
            .unwrap();

        println!("{}", buf);
        buf == "a\nb\nc\nbn"
    })?;

    Ok(())
}

#[test]
fn cmd_history() -> Result<()> {
    let mut tmux = TmuxController::new()?;
    let histfile = tmux.tempfile()?;

    File::create(&histfile)?.write_all(b"a\nb\nc")?;

    tmux.start_sk(
        Some("echo -e -n 'a\\nb\\nc'"),
        &["-i", "-c", "'echo {}'", "--cmd-history", &histfile],
    )?;
    tmux.until(|l| l[0].starts_with("c>"))?;

    tmux.send_keys(&[Ctrl(&Key('p'))])?;
    tmux.until(|l| l[0].trim() == "c> c")?;

    tmux.send_keys(&[Ctrl(&Key('p'))])?;
    tmux.until(|l| l[0].trim() == "c> b")?;

    tmux.send_keys(&[Ctrl(&Key('p'))])?;
    tmux.until(|l| l[0].trim() == "c> a")?;

    tmux.send_keys(&[Ctrl(&Key('n'))])?;
    tmux.until(|l| l[0].trim() == "c> b")?;

    tmux.send_keys(&[Key('n')])?;
    tmux.until(|l| l[0].trim() == "c> bn")?;

    tmux.send_keys(&[Enter])?;

    tmux.until(|_| {
        let mut buf = String::new();
        File::open(Path::new(&histfile))
            .unwrap()
            .read_to_string(&mut buf)
            .unwrap();

        println!("{}", buf);
        buf == "a\nb\nc\nbn"
    })?;

    Ok(())
}
