#![cfg(unix)]

#[allow(dead_code)]
#[macro_use]
mod common;

use common::tmux::Keys::*;

sk_test!(test_ansi_flag_enabled, @cmd "echo -e 'plain\\n\\x1b[31mred\\x1b[0m\\n\\x1b[32mgreen\\x1b[0m'", &["--ansi", "--color", "current_match_bg:1,current_bg:2"], {
    @capture[0] starts_with(">");
    @lines |l| (l.len() >= 3 && l.iter().any(|line| line.contains("plain")));

    @keys Key('d');
    @capture[2] starts_with("> red");

    @capture_colored[*] contains("mre\u{1b}");
    @keys Enter;
    @output[*] trim().eq("red");

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

sk_test!(test_ansi_flag_no_strip, @cmd "echo -e 'plain\\n\\x1b[31mred\\x1b[0m\\n\\x1b[32mgreen\\x1b[0m'", &["--ansi", "--no-strip-ansi", "--color", "current_match_bg:1,current_bg:2"], {
    @capture[0] starts_with(">");
    @lines |l| (l.len() >= 3 && l.iter().any(|line| line.contains("plain")));

    @keys Key('d');
    @capture[2] starts_with("> red");

    @capture_colored[*] contains("mre\u{1b}");
    @keys Enter;
    @output[*] contains("mred\u{1b}");
});

insta_test!(test_prompt_ansi, ["a"], &["--prompt", "\x1b[1;34mprompt\x1b[0m nocol"], {
    @snap;
});
