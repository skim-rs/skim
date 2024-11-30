use e2e::Keys::*;
use e2e::TmuxController;
use std::fs::File;
use std::fs::Permissions;
use std::io::Read;
use std::io::Result;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn setup_tmux_mock(tmux: &TmuxController) -> Result<String> {
    let dir = &tmux.tempdir;
    let path = dir.path().join("tmux");
    let mock_bin = Path::new(&path);
    let mut writer = File::create_new(mock_bin)?;
    let outfile = dir.path().join("tmux-mock-cmd");
    writer.write_fmt(format_args!(
        "#!/bin/sh

echo $@ > {}
",
        outfile.to_str().unwrap()
    ))?;
    std::fs::set_permissions(mock_bin, Permissions::from_mode(0o777))?;
    tmux.send_keys(&[
        Str(&format!("export PATH={}:$PATH", dir.path().to_str().unwrap())),
        Enter,
    ])?;

    tmux.until(|_| Path::new(&tmux.tempdir.path().join("tmux")).exists())?;

    Ok(outfile.to_str().unwrap().to_string())
}

fn get_tmux_cmd(outfile: &str) -> Result<String> {
    let mut cmd = String::new();
    File::open(outfile)?.read_to_string(&mut cmd)?;
    Ok(cmd)
}

#[test]
fn tmux_vanilla() -> Result<()> {
    let tmux = TmuxController::new()?;
    let outfile = setup_tmux_mock(&tmux)?;
    tmux.start_sk(None, &["--tmux"])?;
    tmux.until(|_| Path::new(&outfile).exists())?;
    let cmd = get_tmux_cmd(&outfile)?;
    assert!(cmd.starts_with("display-popup"));
    assert!(cmd.contains("-E"));
    assert!(!cmd.contains("<"));

    Ok(())
}
#[test]
fn tmux_stdin() -> Result<()> {
    let tmux = TmuxController::new()?;
    let outfile = setup_tmux_mock(&tmux)?;
    tmux.start_sk(Some("ls"), &["--tmux"])?;
    tmux.until(|_| Path::new(&outfile).exists())?;
    let cmd = get_tmux_cmd(&outfile)?;
    println!("{}", cmd);
    assert!(cmd.contains("<"));

    Ok(())
}
