#[allow(dead_code)]
#[macro_use]
mod common;

use common::Keys::*;

sk_test!(test_ansi_flag_enabled, @cmd "echo -e 'plain\\n\\x1b[31mred\\x1b[0m\\n\\x1b[32mgreen\\x1b[0m'", &["--ansi"], tmux => {
    tmux.until(|lines| lines.iter().any(|line| line.contains("plain")))?;

    tmux.send_keys(&[Str("d")])?;

    tmux.until(|l| l.len() > 2 && l[2].starts_with("> red"))?;

    let colored_lines = tmux.capture_colored().unwrap();

    let full_output = colored_lines.join(" ");
    println!("{:?}", full_output);
    assert!(full_output.contains("\u{1b}[38;5;1mre"));

    tmux.send_keys(&[Enter])?;
});

sk_test!(test_ansi_flag_disabled, @cmd "echo -e 'plain\\n\\x1b[31mred\\x1b[0m\\n\\x1b[32mgreen\\x1b[0m'", &[], {
    @lines |l| (l.iter().any(|line| line.contains("plain")));

    @keys Str("red");

    @capture[2] eq("> ?[31mred?[0m");

    @keys Enter;
});

sk_test!(test_ansi_matching_on_stripped_text, @cmd "echo -e '\\x1b[32mgreen\\x1b[0m text\\n\\x1b[31mred\\x1b[0m text\\nplain text'", &["--ansi"], tmux => {
    tmux.until(|lines| lines.iter().any(|line| line.contains("plain")))?;

    tmux.send_keys(&[Str("text")])?;

    tmux.until(|l| l.len() > 2 && l.iter().filter(|line| line.contains("text")).count() >= 3)?;

    let lines = tmux.capture().unwrap();
    assert!(lines.iter().any(|line| line.contains("green")));
    assert!(lines.iter().any(|line| line.contains("red")));
    assert!(lines.iter().any(|line| line.contains("plain")));

    tmux.send_keys(&[Ctrl(&Key('u')), Str("green")])?;

    tmux.until(|l| l.len() == 3 && l[2].contains("green"))?;

    let lines = tmux.capture().unwrap();
    let visible_items = lines.iter().filter(|line| line.contains("text")).count();
    assert_eq!(visible_items, 1);

    tmux.send_keys(&[Enter])?;
});
