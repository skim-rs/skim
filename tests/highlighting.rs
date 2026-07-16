#![allow(missing_docs, clippy::pedantic)]

#[allow(dead_code)]
#[macro_use]
mod common;

// Matched query characters get the `current_match` color (1) on the selected row
// and `matched` (9) elsewhere. `@snap_color` captures the per-cell styling so the
// highlight is actually asserted, cross-platform.
insta_test!(highlight_match, ["apple", "banana", "grape"], &["--color=matched:9,current_match:1"], {
    @type "pp";
    @snap;
    @snap_color;
});

// With `current_bg:236`, the selected row also carries that background while its
// matched characters keep the `current_match` foreground (1).
insta_test!(highlight_split_match, ["apple", "banana", "grape"], &["--color=matched:9,current_match:1,current_bg:236"], {
    @type "aaa";
    @snap;
    @snap_color;
});
