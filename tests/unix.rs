#![cfg(unix)]
#[allow(dead_code)]
#[macro_use]
mod common;
use common::tmux::{Keys, TmuxController, sk};
use std::io::Result;

sk_test!(vanilla_basic, "1\n2\n3", &[], {
  @capture[0] eq(">");
  @capture[1] trim().starts_with("3/3");
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

sk_test!(tmux_version_long, "", &["--version"], {
  @output[0] starts_with("sk ");
});
sk_test!(tmux_version_short, "", &["-V"], {
  @output[0] starts_with("sk ");
});

sk_test!(opt_read0, "a\\0b\\0c", &["--read0"], {
  @capture[1] starts_with("  3/3");
  @capture[2] starts_with("> a");
  @capture[3] ends_with("b");
  @capture[4] ends_with("c");
});

sk_test!(opt_print0, "a\\nb\\nc", &["-m", "--print0"], {
  @lines |l| (l.len() > 4);
  @keys BTab, BTab, Enter;
  @lines |l| (l.len() > 0 && !l[0].starts_with(">"));
  @output[0] trim().eq("a\0b\0");
});

sk_test!(opt_print_query, "10\\n20\\n30", &["-q", "2", "--print-query"], {
  @capture[2] trim().eq("> 20");
  @keys Enter;
  @capture[0] ne("> 2");

  @dbg;
  @output[0] trim().eq("2");
  @output[1] trim().eq("20");
});

sk_test!(opt_print_cmd, "1\\n2\\n3", &["--cmd-query", "cmd", "--print-cmd"], {
  @lines |l| (l.len() > 4);
  @capture[0] starts_with(">");
  @capture[2] trim().eq("> 1");
  @keys Enter;
  @output[0] trim().eq("cmd");
  @output[1] trim().eq("1");
});

sk_test!(opt_print_cmd_and_query, "10\\n20\\n30", &["--cmd-query", "cmd", "--print-cmd", "-q", "2", "--print-query"], {
  @capture[0] starts_with("> 2");
  @capture[2] trim().eq("> 20");
  @keys Enter;
  @output[0] trim().eq("2");
  @output[1] trim().eq("cmd");
  @output[2] trim().eq("20");
});

sk_test!(opt_print_header, "x", &["--header", "foo", "--print-header"], {
    @capture[0] starts_with(">");
    @keys Enter;
    @output[0] trim().eq("foo");
    @output[1] trim().eq("x");
});

sk_test!(opt_print_score, "x\\nyx\\nyz", &["--print-score"], {
    @capture[0] starts_with(">");
    @keys Key('x');
    @capture[0] starts_with("> x");
    @capture[1] trim().starts_with("2/3");
    @keys Enter;
    @output[0] trim().eq("x");
    @output[1] trim().eq("-31");
});

sk_test!(opt_print_score_multi, "x\\nyx\\nyz", &["--print-score", "-m"], {
    @capture[0] starts_with(">");
    @keys Key('x');
    @capture[0] starts_with("> x");
    @capture[1] trim().starts_with("2/3");
    @keys BTab;
    @capture[2] trim().eq(">x");
    @capture[3] trim().eq("> yx");
    @keys BTab;
    @capture[3] trim().eq(">>yx");
    @keys Enter;
    @output[0] trim().eq("x");
    @output[1] trim().eq("-31");
    @output[2] trim().eq("yx");
    @output[3] trim().eq("-15");
});

sk_test!(opt_ansi_null, "a\\0b", &["--ansi"], {
  @capture[1] trim().starts_with("1/1");
  @keys Enter;
  @output[0] contains("\0");
});

use common::tmux::Keys::*;

sk_test!(opt_reserved_options, "a\\nb", &[], tmux => {
  let reserved_options = [
      "--extended",
      "--literal",
      "--no-mouse",
      "--cycle",
      "--hscroll-off=10",
      "--filepath-word",
      "--jump-labels=CHARS",
      "--border",
      "--inline-info",
      "--header=STR",
      "--header-lines=1",
      "--no-bold",
      "--history-size=10",
      "--sync",
      "--no-sort",
      "--select-1",
      "-1",
      "--exit-0",
      "-0",
  ];

  for option in reserved_options {
      println!("Starting sk with opt {}", option);
      let mut tmux = TmuxController::new()?;
      tmux.start_sk(Some(&format!("echo -n -e 'a\\nb'")), &[option])?;
      tmux.until(|l| l.len() > 0 && l[0].starts_with(">"))?;
  }
});

sk_test!(opt_multiple_flags_basic, "a\\nb", &[], tmux => {
  let basic_flags = [
      "--bind=ctrl-a:cancel --bind ctrl-b:cancel",
      "--tiebreak=begin --tiebreak=score",
      "--cmd asdf --cmd find",
      "--query asdf -q xyz",
      "--delimiter , --delimiter . -d ,",
      "--nth 1,2 --nth=1,3 -n 1,3",
      "--with-nth 1,2 --with-nth=1,3",
      "-I {} -I XX",
      "--color base --color light",
      "--margin 30% --margin 0",
      "--min-height 30% --min-height 10",
      "--preview 'ls {}' --preview 'cat {}'",
      "--preview-window up --preview-window down",
      "--multi -m",
      "--no-multi --no-multi",
      "--tac --tac",
      "--ansi --ansi",
      "--exact -e",
      "--regex --regex",
      "--literal --literal",
      "--no-mouse --no-mouse",
      "--cycle --cycle",
      "--no-hscroll --no-hscroll",
      "--filepath-word --filepath-word",
      "--border --border",
      "--inline-info --inline-info",
      "--no-bold --no-bold",
      "--print-query --print-query",
      "--print-cmd --print-cmd",
      "--print0 --print0",
      "--sync --sync",
      "--extended --extended",
      "--no-sort --no-sort",
      "--select-1 --select-1",
      "--exit-0 --exit-0",
  ];

  for cmd_flags in basic_flags {
      let mut tmux = TmuxController::new()?;
      tmux.start_sk(Some(&format!("echo -n -e 'a\\nb'")), &[cmd_flags])?;
      tmux.until(|l| l.len() > 0 && l[0].starts_with(">"))?;
  }
});

use std::io::Write;
use tempfile::NamedTempFile;

sk_test!(opt_pre_select_file, "a\\nb\\nc", &[], tmux => {
  let mut pre_select_file = NamedTempFile::new()?;
  pre_select_file.write(b"b\nc")?;
  let mut tmux = TmuxController::new()?;
  tmux.start_sk(
      Some(&format!("echo -n -e 'a\\nb\\nc'")),
      &["-m", "--pre-select-file", pre_select_file.path().to_str().unwrap()],
  )?;
  tmux.until(|l| l.len() > 4 && l[2] == "> a" && l[3].trim() == ">b" && l[4].trim() == ">c")?;
});

sk_test!(opt_accept_arg, "a\\nb", &["--bind", "ctrl-a:accept:hello"], {
  @capture[1] trim().starts_with("2/2");
  @keys Ctrl(&Key('a'));
  @output[0] trim().eq("hello");
  @output[1] trim().eq("a");
});

// Bind tests that require output capture

sk_test!(bind_execute_0_results, "", &["--bind", "'ctrl-f:execute(echo foo{})'"], {
  @capture[0] eq(">");
  @keys Ctrl(&Key('f')), Enter;
  @capture[0] ne(">");

  @output[0] eq("foo");
});

sk_test!(bind_execute_0_results_noref, "", &["--bind", "'ctrl-f:execute(echo foo)'"], {
  @capture[0] eq(">");
  @keys Ctrl(&Key('f')), Enter;
  @capture[0] ne(">");

  @output[0] eq("foo");
});

#[test]
fn bind_reload_no_arg() -> Result<()> {
    let tmux = TmuxController::new()?;

    let outfile = tmux.tempfile()?;
    let sk_cmd = sk(&outfile, &["--bind", "'ctrl-a:reload'"])
        .replace("SKIM_DEFAULT_COMMAND=", "SKIM_DEFAULT_COMMAND='echo hello'");
    tmux.send_keys(&[Keys::Str(&sk_cmd), Keys::Enter])?;
    tmux.until(|l| l[0].starts_with(">"))?;

    tmux.send_keys(&[Keys::Ctrl(&Keys::Key('a'))])?;
    tmux.until(|l| l.len() > 2 && l[2] == "> hello")?;

    Ok(())
}

sk_test!(bind_reload_cmd, "a\\n\\nb\\nc", &["--bind", "'ctrl-a:reload(echo hello)'"], {
  @capture[2] eq("> a");
  @keys Ctrl(&Key('a'));
  @capture[2] eq("> hello");
});
