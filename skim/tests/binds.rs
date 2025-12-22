#[allow(dead_code)]
#[macro_use]
mod common;

use common::Keys::*;
use common::TmuxController;
use common::sk;
use std::io::Result;

sk_test!(bind_execute_0_results, "", &["--bind", "'ctrl-f:execute(echo foo{})'"], @dsl {
  @ line 0 == ">";
  @ keys Ctrl(&Key('f')), Enter;
  @ line 0 != ">";

  @ out 0 == "foo";
});

sk_test!(bind_execute_0_results_noref, "", &["--bind", "'ctrl-f:execute(echo foo)'"], @dsl {
  @ line 0 == ">";
  @ keys Ctrl(&Key('f')), Enter;
  @ line 0 != ">";

  @ out 0 == "foo";
});

sk_test!(bind_if_non_matched, "a\\nb", &["--bind", "'enter:if-non-matched(backward-delete-char)'", "-q", "ab"], @dsl {
  @ line 0 starts_with(">");
  @ line 0 starts_with("> ab");

  @ keys Enter;
  @ line 0 == "> a";
  @ line 2 == "> a";

  @ keys Enter, Key('c');
  @ line 0 starts_with("> ac");
});

sk_test!(bind_append_and_select, "a\\n\\nb\\nc", &["-m", "--bind", "'ctrl-f:append-and-select'"], @dsl {
  @ keys Str("xyz"), Ctrl(&Key('f'));
  @ line 2 == ">>xyz";
});

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

sk_test!(bind_reload_cmd, "a\\n\\nb\\nc", &["--bind", "'ctrl-a:reload(echo hello)'"], @dsl {
  @ line 2 == "> a";
  @ keys Ctrl(&Key('a'));
  @ line 2 == "> hello";
});

sk_test!(bind_first_last, @cmd "seq 1 10", &["--bind", "'ctrl-f:first,ctrl-l:last'"], @dsl {
  @ lines |l| (l.len() > 10);

  @ keys Ctrl(&Key('f'));
  @ lines |l| (l.iter().any(|line| line == "> 1"));

  @ keys Ctrl(&Key('l'));
  @ lines |l| (l.iter().any(|line| line == "> 10"));

  @ keys Ctrl(&Key('f'));
  @ lines |l| (l.iter().any(|line| line == "> 1"));
});

sk_test!(bind_top_alias, @cmd "seq 1 10", &["--bind", "'ctrl-t:top,ctrl-l:last'"], @dsl {
  @ lines |l| (l.len() > 10);

  @ keys Ctrl(&Key('l'));
  @ lines |l| (l.iter().any(|line| line == "> 10"));

  @ keys Ctrl(&Key('t'));
  @ lines |l| (l.iter().any(|line| line == "> 1"));
});

sk_test!(bind_change, @cmd "printf '1\\n12\\n13\\n14\\n15\\n16\\n17\\n18\\n19\\n10'", &["--bind", "'change:first'"], @dsl {
  @ lines |l| (l.len() > 10);

  @ keys Up, Up;
  @ lines |l| (l.iter().any(|x| x.starts_with("> 13")));

  @ keys Key('1');
  @ lines |l| (l.iter().any(|x| x.starts_with("> 1")));
});
