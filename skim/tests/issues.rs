#[allow(dead_code)]
#[macro_use]
mod common;

sk_test!(issue_359_multi_regex_unicode, @cmd "echo 'ああa'", &["--regex", "-q", "'a'"], {
  @capture[0] eq("> a");
  @capture[2] eq("> ああa");
});

sk_test!(issue_361_literal_space_control, "foo  bar\\nfoo bar", &["-q", "'foo\\ bar'"], {
  @lines |l| (l.len() == 4);
  @capture[0] starts_with(">");
  @capture[2] eq("> foo bar");
});

sk_test!(issue_361_literal_space_invert, "foo  bar\\nfoo bar", &["-q", "'!foo\\ bar'"], {
  @capture[0] starts_with(">");
  @capture[2] eq("> foo  bar");
});

sk_test!(issue_547_null_match, "\\0Test Test Test", &[], tmux => {
  tmux.send_keys(&[crate::common::Keys::Str("Test")])?;
  tmux.until(|l| l.len() > 0 && l[0].starts_with(">"))?;
  tmux.until(|l| l.len() > 2 && l[2].starts_with("> Test Test Test"))?;

  let out = tmux.capture_colored()?;
  println!("out {out:?}");
  assert!(
      out.len() > 2
          && out[2].starts_with(
              "\u{1b}[1m\u{1b}[38;5;168m\u{1b}[48;5;236m>\u{1b}[0m \u{1b}[38;5;151m\u{1b}[48;5;236mTest"
          )
  );
});
