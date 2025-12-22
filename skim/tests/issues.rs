#[allow(dead_code)]
#[macro_use]
mod common;

use common::Keys::*;
use common::TmuxController;
use std::io::Result;

sk_test!(issue_359_multi_regex_unicode, @cmd "echo 'ああa'", &["--regex", "-q", "'a'"], @dsl {
  @ line 0 == "> a";
  @ line 2 == "> ああa";
});

sk_test!(issue_361_literal_space_control, @cmd "echo -ne 'foo  bar\\nfoo bar'", &["-q", "'foo\\ bar'"], @dsl {
  @ lines |l| (l.len() == 4 && l[0].starts_with(">"));
  @ line 2 == "> foo bar";
});
#[test]
fn issue_361_literal_space_invert() -> Result<()> {
    let mut tmux = TmuxController::new()?;
    tmux.send_keys(&[Str("set +o histexpand"), Enter])?;
    tmux.start_sk(Some("echo -ne 'foo bar\\nfoo  bar'"), &["-q", "'!foo\\ bar'"])?;
    tmux.until(|l| l[0].starts_with(">"))?;
    tmux.until(|l| l.len() > 2 && l[2] == "> foo  bar")?;

    Ok(())
}

#[test]
fn issue_547_null_match() -> Result<()> {
    let mut tmux = TmuxController::new()?;
    tmux.send_keys(&[Str("set +o histexpand"), Enter])?;
    tmux.start_sk(Some("echo -e \"\\0Test Test Test\""), &["-q", "Test"])?;

    tmux.until(|l| l[0].starts_with(">"))?;
    tmux.until(|l| l[2].starts_with("> Test Test Test"))?;

    let out = tmux.capture_colored()?;
    println!("out {out:?}");
    assert!(
        out.len() > 2
            && out[2].starts_with(
                "\u{1b}[1m\u{1b}[38;5;168m\u{1b}[48;5;236m>\u{1b}[0m \u{1b}[38;5;151m\u{1b}[48;5;236mTest"
            )
    );

    Ok(())
}
