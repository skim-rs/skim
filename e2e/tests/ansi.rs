use e2e::SK;
use e2e::TmuxController;
use std::io::Result;

#[test]
pub fn reset_after_exit_height() -> Result<()> {
    let tmux = TmuxController::new()?;
    let _ = tmux.send_keys(&[
        e2e::Keys::Str(&format!("echo -e '1\n2' | {SK} --height 10")),
        e2e::Keys::Enter,
    ])?;
    tmux.until(|l| l.len() > 0 && l[0] == ">")?;
    tmux.send_keys(&[e2e::Keys::Up])?;
    tmux.until(|l| l.len() > 3 && l[3] == "> 2")?;
    tmux.send_keys(&[e2e::Keys::Enter])?;
    tmux.until(|l| l[0].contains("$"))?;
    let lines = tmux.capture_ansi()?;
    println!("Lines: {:?}", lines);

    // Ensure we properly reset the color after exiting, at least the foreground color
    assert!(lines[1].contains("\u{1b}[39m2") || lines[1].contains("\u{1b}[0m2"));

    Ok(())
}
