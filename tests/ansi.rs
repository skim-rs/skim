#[allow(dead_code)]
#[macro_use]
mod common;

use common::Keys::*;

sk_test!(test_ansi_flag_enabled, @cmd "echo -e 'plain\\n\\x1b[31mred\\x1b[0m\\n\\x1b[32mgreen\\x1b[0m'", &["--ansi"], {
    @capture[0] starts_with(">");
    @lines |l| (l.len() >= 3 && l.iter().any(|line| line.contains("plain")));

    @keys Key('d');
    @capture[2] starts_with("> red");

    @capture_colored[*] contains("\u{1b}[48;5;236mre");
    @keys Enter;
});

sk_test!(test_ansi_flag_disabled, @cmd "echo -e 'plain\\n\\x1b[31mred\\x1b[0m\\n\\x1b[32mgreen\\x1b[0m'", &[], {
    @capture[0] starts_with(">");
    @capture[*] contains("plain");

    @keys Str("red");

    @capture[2] eq("> ?[31mred?[0m");

    @keys Enter;
});

sk_test!(test_ansi_matching_on_stripped_text, @cmd "echo -e '\\x1b[32mgreen\\x1b[0m text\\n\\x1b[31mred\\x1b[0m text\\nplain text'", &["--ansi"], {
    @capture[0] starts_with(">");
    @lines |l| (l.len() >= 3 && l.iter().any(|line| line.contains("plain")));
    @keys Str("text");
    // Tiebreak will reorder items
    @capture[2] contains("red text");
    @capture[3] contains("green text");
    @capture[4] contains("plain text");


    @keys Ctrl(&Key('u')), Str("green");
    @capture[2] contains("green");

    @lines |l| (l.len() == 3);
});
