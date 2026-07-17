#![allow(missing_docs, clippy::pedantic)]

#[allow(dead_code)]
#[macro_use]
mod common;

// With --ansi the colored input is interpreted: the items render with their
// ANSI colors and the matched query characters are highlighted. `@snap_color`
// captures the per-cell styling so this actually verifies color, while
// `--color current_match_bg:1,current_bg:2` exercises the themed selection.
insta_test!(test_ansi_flag_enabled, @bytes b"plain\n\x1b[31mred\x1b[0m\n\x1b[32mgreen\x1b[0m\n", &["--ansi", "--color", "current_match_bg:1,current_bg:2"], {
    @type "d";
    @snap;
    @snap_color;
});

// Without --ansi, the escape sequences are not interpreted: they are matched
// and displayed as literal text, and the items carry no color. `@snap_color`
// asserts the absence of ANSI-derived styling on the item rows.
insta_test!(test_ansi_flag_disabled, @bytes b"plain\n\x1b[31mred\x1b[0m\n\x1b[32mgreen\x1b[0m\n", &[], {
    @type "red";
    @snap;
    @snap_color;
});

// With --ansi, matching happens on the ANSI-stripped text and the tiebreak
// reorders the matches. The color snapshot confirms each item keeps its own
// (red / green) foreground after matching.
insta_test!(test_ansi_matching_on_stripped_text, @bytes b"\x1b[32mgreen\x1b[0m text\n\x1b[31mred\x1b[0m text\nplain text\n", &["--ansi"], {
    @type "text";
    @snap;
    @snap_color;
    @ctrl 'u';
    @type "green";
    @snap;
});

// --no-strip-ansi only affects the accepted output (it keeps the escape
// sequences); on screen it renders identically to --ansi.
insta_test!(test_ansi_flag_no_strip, @bytes b"plain\n\x1b[31mred\x1b[0m\n\x1b[32mgreen\x1b[0m\n", &["--ansi", "--no-strip-ansi", "--color", "current_match_bg:1,current_bg:2"], {
    @type "d";
    @snap;
    @snap_color;
});

insta_test!(test_prompt_ansi, ["a"], &["--prompt", "\x1b[1;34mprompt\x1b[0m nocol"], {
    @snap;
    @snap_color;
});

// --ansi combined with --hide-nth: the hidden (red) middle field is removed from
// the rendered line, while the surviving green/plain fields keep their ANSI colors.
// The color snapshot confirms the green foreground survives and the red one is gone.
insta_test!(
    test_ansi_hide_nth,
    @bytes b"\x1b[32mgreen\x1b[0m \x1b[31mred\x1b[0m plain\n",
    &["--ansi", "--delimiter", " ", "--hide-nth", "2"],
    {
        @snap;
        @snap_color;
    }
);

// The hidden ANSI field stays searchable: matching its text ("red") still selects
// the item even though the field is not shown, and no highlight leaks onto the
// visible text.
insta_test!(
    test_ansi_hide_nth_searchable,
    @bytes b"\x1b[32mgreen\x1b[0m \x1b[31mred\x1b[0m plain\n",
    &["--ansi", "--delimiter", " ", "--hide-nth", "2"],
    {
        @type "red";
        @snap;
        @snap_color;
    }
);
