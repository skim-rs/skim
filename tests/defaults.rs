#[allow(dead_code)]
#[macro_use]
mod common;

use common::{Keys, TmuxController, sk};
use std::io::Result;

sk_test!(vanilla_basic, "1\n2\n3", &[], {
  @capture[0] eq(">");
  @capture[1] trim().starts_with("3/3");
  @capture[1] ends_with("0/0");
  @capture[2] eq("> 1");
  @capture[3] eq("  2");
});

sk_test!(vanilla, @cmd "seq 1 100000", &[], {
  @capture[0] eq(">");
  @capture[1] starts_with("  100000");
  @capture[1] ends_with("0/0");
  @capture[2] eq("> 1");
  @capture[3] eq("  2");
});

#[test]
fn default_command() -> Result<()> {
    let tmux = TmuxController::new()?;

    let outfile = tmux.tempfile()?;
    let sk_cmd = sk(&outfile, &[]).replace("SKIM_DEFAULT_COMMAND=", "SKIM_DEFAULT_COMMAND='echo hello'");
    tmux.send_keys(&[Keys::Str(&sk_cmd), Keys::Enter])?;
    tmux.until(|l| l[0].starts_with(">"))?;
    tmux.until(|l| l.len() > 1 && l[1].starts_with("  1/1"))?;
    tmux.until(|l| l.len() > 2 && l[2] == "> hello")?;

    tmux.send_keys(&[Keys::Enter])?;
    tmux.until(|l| !l[0].starts_with(">"))?;

    let output = tmux.output_from(&outfile)?;

    assert_eq!(output[0], "hello");

    Ok(())
}

sk_test!(version_long, "", &["--version"], {
  @output[0] starts_with("sk ");
});
sk_test!(version_short, "", &["-V"], {
  @output[0] starts_with("sk ");
});

sk_test!(interactive_mode_command_execution, "", &["-i", "--cmd=\"echo 'foo {q}'\""], {
  @capture[0] starts_with("c>");
  @capture[2] starts_with("> foo");

  @keys Keys::Str("bar");
  @capture[0] starts_with("c> bar");
  @capture[2] starts_with("> foo bar");

  @keys Keys::Str("baz");
  @capture[0] starts_with("c> barbaz");
  @capture[2] starts_with("> foo barbaz");
});

sk_test!(unicode_input, "", &["-q", "󰬈󰬉󰬊"], {
    @capture[0] starts_with("> 󰬈󰬉󰬊");
    @keys Keys::Key('|');
    @capture[0] starts_with("> 󰬈󰬉󰬊|");
    @keys Keys::Left, Keys::Left, Keys::Key('|');
    @capture[0] starts_with("> 󰬈󰬉|󰬊|");
    @keys Keys::Key('󰬈');
    @capture[0] starts_with("> 󰬈󰬉|󰬈󰬊|");
});
