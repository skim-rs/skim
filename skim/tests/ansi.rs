#[allow(dead_code)]
mod common;

use common::Keys::*;
use common::TmuxController;
use std::io::Result;

#[test]
fn test_ansi_flag_enabled() -> Result<()> {
    let tmux = TmuxController::new().unwrap();
    let _outfile = tmux
        .start_sk(
            Some("echo -e 'plain\\n\\x1b[31mred\\x1b[0m\\n\\x1b[32mgreen\\x1b[0m'"),
            &["--ansi"],
        )
        .unwrap();

    tmux.until(|lines| lines.iter().any(|line| line.contains("plain")))?;

    tmux.send_keys(&[Str("d")])?;

    // This will make sure that no ansi codes are actually outputted
    tmux.until(|l| l.len() > 2 && l[2].starts_with("> red"))?;

    let colored_lines = tmux.capture_colored().unwrap();

    let full_output = colored_lines.join(" ");
    println!("{:?}", full_output);
    // The color code is rerwritten by ansi-to-tui to a 256color one
    assert!(full_output.contains("\u{1b}[38;5;1mre"));

    tmux.send_keys(&[Enter])
}

#[test]
fn test_ansi_flag_disabled() -> Result<()> {
    let tmux = TmuxController::new()?;
    let _outfile = tmux.start_sk(
        Some("echo -e 'plain\\n\\x1b[31mred\\x1b[0m\\n\\x1b[32mgreen\\x1b[0m'"),
        &[],
    )?;

    tmux.until(|lines| lines.iter().any(|line| line.contains("plain")))?;

    tmux.send_keys(&[Str("red")])?;

    tmux.until(|l| l.len() > 2 && l[2] == "> ?[31mred?[0m")?;

    tmux.send_keys(&[Enter])
}

#[test]
fn test_ansi_matching_on_stripped_text() -> Result<()> {
    let tmux = TmuxController::new().unwrap();
    let _outfile = tmux
        .start_sk(
            Some("echo -e '\\x1b[32mgreen\\x1b[0m text\\n\\x1b[31mred\\x1b[0m text\\nplain text'"),
            &["--ansi"],
        )
        .unwrap();

    tmux.until(|lines| lines.iter().any(|line| line.contains("plain")))?;

    // Search for "text" - should match all three lines because matching is on stripped text
    tmux.send_keys(&[Str("text")])?;

    tmux.until(|l| l.len() > 2 && l.iter().filter(|line| line.contains("text")).count() >= 3)?;

    // All three items should be visible
    let lines = tmux.capture().unwrap();
    assert!(lines.iter().any(|line| line.contains("green")));
    assert!(lines.iter().any(|line| line.contains("red")));
    assert!(lines.iter().any(|line| line.contains("plain")));

    // Search for "green" - should only match the first line
    tmux.send_keys(&[Ctrl(&Key('u')), Str("green")])?;

    tmux.until(|l| l.len() == 3 && l[2].contains("green"))?;

    let lines = tmux.capture().unwrap();
    // Should only see one match now
    let visible_items = lines.iter().filter(|line| line.contains("text")).count();
    assert_eq!(visible_items, 1);

    tmux.send_keys(&[Enter])
}
