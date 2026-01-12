#[allow(dead_code)]
#[macro_use]
mod common;

use common::Keys::*;

sk_test!(highlight_match, @cmd "echo -e 'apple\\nbanana\\ngrape'", &["--color=matched:9,current_match:1"], {
    @capture[2] contains("apple");
    @keys Str("pp");

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

sk_test!(highlight_split_match, @cmd "echo -e 'apple\\nbanana\\ngrape'", &["--color=matched:9,current_match:1"], {
    @capture[2] contains("apple");

    @keys Str("aaa");

    // Wait for filtering to complete - should only show banana
    @capture[1] contains("1/3");
    @capture[2] contains("banana");


    @capture_colored[2] contains("b");
    @capture_colored[2] contains("a");
    @capture_colored[2] contains("n");

    // Check that the 'p' characters in "apple" have highlighting color codes
    @capture_colored[2] contains("\x1b[38;5;1m");
    let highlight_pattern = "\x1b[38;5;1m\x1b[48;5;236ma";
    @capture_colored[2] matches(highlight_pattern).count() == 3;

    @keys Enter;

    @output[0] eq("banana");
});
