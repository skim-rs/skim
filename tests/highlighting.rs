#[allow(dead_code)]
#[macro_use]
mod common;

use common::tmux::Keys::*;

sk_test!(highlight_match, @cmd "echo -e 'apple\\nbanana\\ngrape'", &["--color=matched:9,current_match:1"], {
    @capture[2] contains("apple");
    @keys Key('p'), Key('p');

    // Wait for filtering to complete - should only show apple
    @capture[1] contains("1/3");
    @capture[2] contains("apple");

    @capture_colored[2] contains("a");
    @capture_colored[2] contains("pp");
    @capture_colored[2] contains("le");

    // Check that the 'p' characters in "apple" have highlighting color codes
    @capture_colored[2] contains("\x1b[38;5;1m");
    @capture_colored[2] contains("pp\x1b[");

    @keys Enter;

    @output[0] eq("apple");
});

sk_test!(highlight_split_match, @cmd "echo -e 'apple\\nbanana\\ngrape'", &["--color=matched:9,current_match:1,current_bg:236"], {
    @capture[2] contains("apple");

    @keys Key('a'), Key('a'), Key('a');

    // Wait for filtering to complete - should only show banana
    @capture[1] contains("1/3");
    @capture[2] contains("banana");


    @capture_colored[2] contains("b");
    @capture_colored[2] contains("a");
    @capture_colored[2] contains("n");

    // Check that matched characters have the current_match foreground color (color 1)
    @capture_colored[2] contains("\x1b[38;5;1m");
    // Check that the current line has the current background color (color 236)
    @capture_colored[2] contains("\x1b[48;5;236m");
    // Check that there are 3 matched 'a' characters with foreground color 1
    let match_fg_pattern = "\x1b[38;5;1ma";
    @capture_colored[2] matches(match_fg_pattern).count() == 3;

    @keys Enter;

    @output[0] eq("banana");
});
