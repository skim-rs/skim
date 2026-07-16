#![allow(missing_docs, clippy::pedantic)]

use crate::common::SK;
use std::process::Command;

#[allow(dead_code)]
#[macro_use]
mod common;

insta_test!(opt_with_nth_preview, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth", "2..", "--preview", "echo X{1}Y"], {
    @snap;
});

// Use info=hidden to hide the spinner
insta_test!(opt_min_query_length, ["line1", "line2", "line3"], &["--min-query-length", "3", "--info", "hidden"], {
    @snap;
    @type "li";
    @snap;
    @char 'n';
    @snap;
});

// Use info=hidden to hide the spinner
#[cfg(unix)]
insta_test!(opt_min_query_length_interactive, @interactive, &["-i", "--min-query-length", "3", "--cmd", "printf 'line1\\nline2\\nline3'", "--info", "hidden"], {
    @snap;
    @type "li";
    @snap;
    @char 'n';
    @snap;
});

#[cfg(windows)]
insta_test!(opt_min_query_length_interactive, @interactive, &["-i", "--min-query-length", "3", "--cmd", "echo line1 & echo line2 & echo line3", "--info", "hidden"], {
    @snap;
    @type "li";
    @snap;
    @char 'n';
    @snap;
});

insta_test!(opt_with_nth_1, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth", "1"], {
    @snap;
});

insta_test!(opt_with_nth_2, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth", "2"], {
    @snap;
});

insta_test!(opt_with_nth_4, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth", "4"], {
    @snap;
});

insta_test!(opt_with_nth_oob, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth", "5"], {
    @snap;
});

insta_test!(opt_with_nth_neg_1, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth=-1"], {
    @snap;
});

insta_test!(opt_with_nth_neg_2, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth=-2"], {
    @snap;
});

insta_test!(opt_with_nth_neg_4, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth=-4"], {
    @snap;
});

insta_test!(opt_with_nth_oob_4, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth=-5"], {
    @snap;
});

insta_test!(opt_with_nth_range_to_end, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth", "2.."], {
    @snap;
});

insta_test!(opt_with_nth_range_from_start, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth", "..3"], {
    @snap;
});

insta_test!(opt_with_nth_range_closed, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth", "2..3"], {
    @snap;
});

insta_test!(opt_with_nth_range_desc, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth", "3..2"], {
    @snap;
});

insta_test!(opt_nth_1, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth", "1"], {
    @snap;
    @char '1';
    @snap;
    @ctrl 'w';
    @snap;
    @char '2';
    @snap;
});

insta_test!(opt_nth_2, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth", "2"], {
    @snap;
    @char '2';
    @snap;
    @ctrl 'w';
    @snap;
    @char '1';
    @snap;
});

insta_test!(opt_nth_4, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth", "4"], {
    @snap;
    @char '4';
    @snap;
    @ctrl 'w';
    @snap;
    @char '1';
    @snap;
});

insta_test!(opt_nth_oob, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth", "5"], {
    @snap;
    @char '1';
    @snap;
});

insta_test!(opt_nth_neg_1, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth=-1"], {
    @snap;
    @char '4';
    @snap;
    @ctrl 'w';
    @snap;
    @char '1';
    @snap;
});

insta_test!(opt_nth_neg_2, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth=-2"], {
    @snap;
    @char '3';
    @snap;
    @ctrl 'w';
    @snap;
    @char '1';
    @snap;
});

insta_test!(opt_nth_neg_4, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth=-4"], {
    @snap;
    @char '1';
    @snap;
    @ctrl 'w';
    @snap;
    @char '2';
    @snap;
});

insta_test!(opt_nth_neg_oob, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth=-5"], {
    @snap;
    @char '1';
    @snap;
});

insta_test!(opt_nth_range_to_end, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth", "2.."], {
    @snap;
    @char '3';
    @snap;
    @ctrl 'w';
    @snap;
    @char '1';
    @snap;
});

insta_test!(opt_nth_range_from_start, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth", "..3"], {
    @snap;
    @char '1';
    @snap;
    @ctrl 'w';
    @snap;
    @char '4';
    @snap;
});

insta_test!(opt_nth_range_closed, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth", "2..3"], {
    @snap;
    @char '2';
    @snap;
    @ctrl 'w';
    @snap;
    @char '3';
    @snap;
    @ctrl 'w';
    @snap;
    @char '1';
    @snap;
    @ctrl 'w';
    @snap;
    @char '4';
    @snap;
});

insta_test!(opt_nth_range_dec, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth", "3..2"], {
    @snap;
    @char '1';
    @snap;
});

insta_test!(opt_hscroll_begin, [&format!("b{}", ["a"; 1000].join(""))], &["-q", "b"], {
    @snap;
});

insta_test!(opt_hscroll_middle, [&format!("{}b{}", ["a"; 1000].join(""), ["a"; 1000].join(""))], &["-q", "b"], {
    @snap;
});

insta_test!(opt_hscroll_end, [&format!("{}b", ["a"; 1000].join(""))], &["-q", "b"], {
    @snap;
});

insta_test!(opt_no_hscroll, [&format!("{}b", ["a"; 1000].join(""))], &["-q", "b", "--no-hscroll"], {
    @snap;
});

insta_test!(opt_tabstop_default, ["a\tb"], &[], {
    @snap;
});

insta_test!(opt_tabstop_1, ["a\tb"], &["--tabstop", "1"], {
    @snap;
});

insta_test!(opt_tabstop_3, ["aa\tb"], &["--tabstop", "3"], {
    @snap;
});

insta_test!(opt_info_control, ["a", "b", "c"], &[], {
    @snap;
    @char 'a';
    @snap;
});

insta_test!(opt_info_default, ["a", "b", "c"], &["--info", "default"], {
    @snap;
    @char 'a';
    @snap;
});

insta_test!(opt_no_info, ["a", "b", "c"], &["--no-info"], {
    @snap;
});

insta_test!(opt_info_hidden, ["a", "b", "c"], &["--info", "hidden"], {
    @snap;
});

insta_test!(opt_info_inline, ["a", "b", "c"], &["--info", "inline"], {
    @snap;
    @char 'a';
    @snap;
});

insta_test!(opt_info_inline_right, ["a", "b", "c"], &["--info", "inline-right"], {
    @snap;
    @char 'a';
    @snap;
});

insta_test!(opt_info_inline_custom, ["a", "b", "c"], &["--info", "inline:SEP"], {
    @snap;
    @char 'a';
    @snap;
});

insta_test!(opt_info_inline_right_custom, ["a", "b", "c"], &["--info", "inline-right:SEP"], {
    @snap;
    @char 'a';
    @snap;
});

insta_test!(opt_inline_info, ["a", "b", "c"], &["--inline-info"], {
    @snap;
    @char 'a';
    @snap;
});

insta_test!(opt_header_only, ["a", "b", "c"], &["--header", "test_header"], {
    @snap;
});

insta_test!(opt_header_multiline, ["a", "b", "c"], &["--header", "header 1\nheader 2"], {
    @snap;
});

insta_test!(opt_header_inline_info, ["a", "b", "c"], &["--header", "test_header", "--inline-info"], {
    @snap;
});

insta_test!(opt_header_reverse, ["a", "b", "c"], &["--header", "test_header", "--reverse"], {
    @snap;
});

insta_test!(opt_header_reverse_inline_info, ["a", "b", "c"], &["--header", "test_header", "--reverse", "--inline-info"], {
    @snap;
});

insta_test!(opt_header_lines_1, ["a", "b", "c"], &["--header-lines", "1"], {
    @snap;
});

insta_test!(opt_header_lines_all, ["a", "b", "c"], &["--header-lines", "4"], {
    @snap;
});

insta_test!(opt_header_lines_inline_info, ["a", "b", "c"], &["--header-lines", "1", "--inline-info"], {
    @snap;
});

insta_test!(opt_header_lines_reverse, ["a", "b", "c"], &["--header-lines", "1", "--reverse"], {
    @snap;
});

insta_test!(opt_header_lines_reverse_inline_info, ["a", "b", "c"], &["--header-lines", "1", "--reverse", "--inline-info"], {
    @snap;
});

insta_test!(opt_skip_to_pattern, ["a/b/c"], &["--skip-to-pattern", "[^/]*$", "--bind", "ctrl-a:scroll-left", "--bind", "ctrl-x:scroll-right"], {
    @snap;
    @ctrl 'a';
    @snap;
    @ctrl 'x';
    @snap;
});

insta_test!(opt_multi, ["a", "b", "c"], &["--multi"], {
    @snap;
    @shift Tab;
    @snap;
    @shift Tab;
    @snap;
});

insta_test!(opt_pre_select_n, ["a", "b", "c"], &["-m", "--pre-select-n", "2"], {
    @snap;
});

insta_test!(opt_pre_select_items, ["a", "b", "c"], &["-m", "--pre-select-items", "$'b\\nc'"], {
    @snap;
});

insta_test!(opt_pre_select_pat, ["a", "b", "c"], &["-m", "--pre-select-pat", "[b|c]"], {
    @snap;
});

#[cfg(unix)]
insta_test!(opt_no_clear_if_empty, @interactive, &["-i", "--no-clear-if-empty", "-c", "printf {q}", "--cmd-query", "xxxx"], {
    @snap;
    @ctrl 'w';
    @snap;
});

#[cfg(windows)]
insta_test!(opt_no_clear_if_empty, @interactive, &["-i", "--no-clear-if-empty", "-c", "if not [{q}]==[] echo.{q}", "--cmd-query", "xxxx"], {
    @snap;
    @ctrl 'w';
    @snap;
});

insta_test!(opt_tac, ["a", "b"], &["--tac"], {
    @snap;
});

insta_test!(opt_tac_with_header_lines, ["a", "b", "c", "d", "e"], &["--tac", "--header-lines", "2"], {
    @snap;
});

insta_test!(opt_replstr, ["a", "b", "c"], &["-I", "..", "--preview", "echo foo {} .."], {
    @snap;
    @char 'a';
    @snap;
});

insta_test!(opt_selector, ["a", "b", "c"], &["--selector", "$"], {
    @snap;
});

insta_test!(opt_no_sort, ["ac", "bc", "cc"], &["--no-sort"], {
    @snap;
    @char 'c';
    @snap;
});

insta_test!(opt_multi_selector, ["a", "b", "c"], &["--multi-selector", "$", "-m"], {
    @snap;
    @shift Tab;
    @snap;
});

insta_test!(opt_cycle, ["a", "b", "c"], &["--cycle"], {
    @snap;
    @key Down;
    @snap;
    @key Up;
    @snap;
});

insta_test!(opt_cycle_header_lines, ["a", "b", "c", "d"], &["--cycle", "--header-lines", "1"], {
    @snap;
    @key Down;
    @snap;
    @key Up;
    @snap;
});

insta_test!(opt_disabled, ["a", "b", "c", "d"], &["--disabled"], {
    @snap;
    @char 'b';
    @snap;
});

const LONG_INPUT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
insta_test!(opt_wrap, [LONG_INPUT], &["--wrap"], {
    @snap;
});

insta_test!(opt_multiple_flags_prompt, [""], &["--prompt", "a", "--prompt", "b", "-p", "c"], {
    @snap;
});

#[cfg(unix)]
insta_test!(opt_multiple_flags_cmd_prompt, @interactive, &["-i", "--cmd-prompt", "a", "--cmd-prompt", "c", "--cmd", "echo"], {
    @snap;
});

#[cfg(windows)]
insta_test!(opt_multiple_flags_cmd_prompt, @interactive, &["-i", "--cmd-prompt", "a", "--cmd-prompt", "c", "--cmd", "echo."], {
    @snap;
});

#[cfg(unix)]
insta_test!(opt_multiple_flags_cmd_query, @interactive, &["-i", "--cmd-query", "a", "--cmd-query", "b", "--cmd", "echo"], {
    @snap;
});

#[cfg(windows)]
insta_test!(opt_multiple_flags_cmd_query, @interactive, &["-i", "--cmd-query", "a", "--cmd-query", "b", "--cmd", "echo."], {
    @snap;
});

#[cfg(unix)]
insta_test!(opt_multiple_flags_interactive, @interactive, &["-i", "--interactive", "--interactive", "--cmd", "echo"], {
    @snap;
});

#[cfg(windows)]
insta_test!(opt_multiple_flags_interactive, @interactive, &["-i", "--interactive", "--interactive", "--cmd", "echo."], {
    @snap;
});

insta_test!(opt_multiple_flags_reverse, [""], &["--reverse", "--reverse"], {
    @snap;
});

insta_test!(opt_multiple_flags_combined_nth, ["a b c", "d e f"], &["--nth", "1,2"], {
    @snap;
    @char 'c';
    @snap;
});

insta_test!(opt_multiple_flags_combined_with_nth, ["a b c", "d e f"], &["--with-nth", "1,2"], {
    @snap;
});

insta_test!(opt_multiple_flags_reverse_and_layout, ["a b c", "d e f"], &["--reverse", "--layout", "default"], {
    @snap;
});

insta_test!(opt_multiple_flags_layout_and_reverse, ["a b c", "d e f"], &["--layout", "default", "--reverse"], {
    @snap;
});

insta_test!(opt_border_default, ["a", "b", "c", "ac"], &["-q", "a", "--border"], {
    @snap;
});

insta_test!(opt_border_none, ["a", "b", "c", "ac"], &["-q", "a", "--border", "none"], {
    @snap;
});

insta_test!(opt_border_plain, ["a", "b", "c", "ac"], &["-q", "a", "--border", "plain"], {
    @snap;
});

insta_test!(opt_border_rounded, ["a", "b", "c", "ac"], &["-q", "a", "--border", "rounded"], {
    @snap;
});

insta_test!(opt_border_double, ["a", "b", "c", "ac"], &["-q", "a", "--border", "double"], {
    @snap;
});

insta_test!(opt_border_thick, ["a", "b", "c", "ac"], &["-q", "a", "--border", "thick"], {
    @snap;
});

insta_test!(opt_border_light_double_dashed, ["a", "b", "c", "ac"], &["-q", "a", "--border", "light-double-dashed"], {
    @snap;
});

insta_test!(opt_border_heavy_double_dashed, ["a", "b", "c", "ac"], &["-q", "a", "--border", "heavy-double-dashed"], {
    @snap;
});

insta_test!(opt_border_light_triple_dashed, ["a", "b", "c", "ac"], &["-q", "a", "--border", "light-triple-dashed"], {
    @snap;
});

insta_test!(opt_border_heavy_triple_dashed, ["a", "b", "c", "ac"], &["-q", "a", "--border", "heavy-triple-dashed"], {
    @snap;
});

insta_test!(opt_border_light_quadruple_dashed, ["a", "b", "c", "ac"], &["-q", "a", "--border", "light-quadruple-dashed"], {
    @snap;
});

insta_test!(opt_border_heavy_quadruple_dashed, ["a", "b", "c", "ac"], &["-q", "a", "--border", "heavy-quadruple-dashed"], {
    @snap;
});

insta_test!(opt_border_quadrant_inside, ["a", "b", "c", "ac"], &["-q", "a", "--border", "quadrant-inside"], {
    @snap;
});

insta_test!(opt_border_quadrant_outside, ["a", "b", "c", "ac"], &["-q", "a", "--border", "quadrant-outside"], {
    @snap;
});

#[cfg(unix)]
#[test]
fn opt_select_1() -> std::io::Result<()> {
    let res = Command::new("/bin/sh")
        .arg("-c")
        .env_clear()
        .arg(format!("printf '1\n2\n3' | {SK} --select-1 -q 3"))
        .stdin(std::process::Stdio::null())
        .output()?;
    assert_eq!(res.status.code(), Some(0));
    assert_eq!(res.stdout, b"3\n");
    Ok(())
}

#[cfg(windows)]
#[test]
fn opt_select_1_windows() -> std::io::Result<()> {
    let res = Command::new("cmd")
        .arg("/C")
        .arg(format!(r"(echo 1 & echo 2 & echo 3) | {SK} --select-1 -q 3"))
        .env("SKIM_DEFAULT_OPTIONS", "")
        .env("SKIM_DEFAULT_COMMAND", "")
        .env("SKIM_OPTIONS_FILE", "")
        .stdin(std::process::Stdio::null())
        .output()?;
    assert_eq!(res.status.code(), Some(0));
    assert!(res.stdout.starts_with(b"3"));
    Ok(())
}

#[cfg(unix)]
#[test]
fn opt_exit_0() -> std::io::Result<()> {
    let res = Command::new("/bin/sh")
        .arg("-c")
        .env_clear()
        .arg(format!("printf '1\n2\n3' | {SK} --exit-0 -q 4"))
        .stdin(std::process::Stdio::null())
        .output()?;
    assert_eq!(res.status.code(), Some(1));
    assert_eq!(res.stdout, &[]);
    Ok(())
}

#[cfg(windows)]
#[test]
fn opt_exit_0_windows() -> std::io::Result<()> {
    let res = Command::new("cmd")
        .arg("/C")
        .arg(format!(r"(echo 1 & echo 2 & echo 3) | {SK} --exit-0 -q 4"))
        .env("SKIM_DEFAULT_OPTIONS", "")
        .env("SKIM_DEFAULT_COMMAND", "")
        .env("SKIM_OPTIONS_FILE", "")
        .stdin(std::process::Stdio::null())
        .output()?;
    assert_eq!(res.status.code(), Some(1));
    assert_eq!(res.stdout, &[]);
    Ok(())
}

insta_test!(opt_select_1_enter, ["1", "2", "3", "11"], &["-q", "1", "--select-1"], {
    @snap;
});
insta_test!(opt_exit_0_enter, ["1", "2", "3"], &["-q", "1", "--exit-0"], {
    @snap;
});

insta_test!(opt_ellipsis, ["aabbccddeeffggghiijjkkllmmnnooppqqrrssttuuvvwwxxyyzz"], &["--preview", "echo a", "--preview-window", "right:80%", "-q", "ij", "--ellipsis", "%%%"], {
    @snap;
});

insta_test!(opt_multiline, ["a", "b1\\nb2", "c"], &["--multiline"], {
    @snap;
    @key Up;
    @snap;
    @key Up;
    @snap;
});

insta_test!(opt_multiline_custom_sep, ["x", "p|q|r", "y"], &["--multiline", "|"], {
    @snap;
});

// Scrolling with multiline items: the screen is 24 rows (22 usable).  Each
// multiline item occupies 2 rows, so 11 such items already fill the screen.
// We feed 15 items and navigate to the top to force the offset to advance.
insta_test!(opt_multiline_scroll, [
    "a1\\na2", "b1\\nb2", "c1\\nc2", "d1\\nd2", "e1\\ne2",
    "f1\\nf2", "g1\\ng2", "h1\\nh2", "i1\\ni2", "j1\\nj2",
    "k1\\nk2", "l1\\nl2", "m1\\nm2", "n1\\nn2", "o1\\no2"
], &["--multiline"], {
    @snap;
    @action Last;
    @snap;
});

insta_test!(opt_multiline_scroll_incr, [
    "a1\\na2", "b1\\nb2", "c1\\nc2", "d1\\nd2", "e1\\ne2",
    "f1\\nf2", "g1\\ng2", "h1\\nh2", "i1\\ni2", "j1\\nj2",
    "k1\\nk2", "l1\\nl2", "m1\\nm2", "n1\\nn2", "o1\\no2"
], &["--multiline"], {
    @snap;
    @key Up;
    @key Up;
    @key Up;
    @key Up;
    @key Up;
    @key Up;
    @key Up;
    @key Up;
    @key Up;
    @key Up;
    @key Up;
    @key Up;
    @snap;
    @key Up;
    @key Up;
    @key Up;
    @key Up;
    @key Up;
    @key Up;
    @key Up;
    @snap;
});

// 30 items — overflows the default 22-row list area, so the scrollbar thumb is partial.
// Use --info=hidden to avoid a non-deterministic spinner character in the snapshot.
const SCROLLBAR_ITEMS: [&str; 30] = [
    "item_01", "item_02", "item_03", "item_04", "item_05", "item_06", "item_07", "item_08", "item_09", "item_10",
    "item_11", "item_12", "item_13", "item_14", "item_15", "item_16", "item_17", "item_18", "item_19", "item_20",
    "item_21", "item_22", "item_23", "item_24", "item_25", "item_26", "item_27", "item_28", "item_29", "item_30",
];

// Default scrollbar: double-vertical symbols, thumb at bottom (beginning of list visible).
insta_test!(opt_scrollbar_default, SCROLLBAR_ITEMS, &["--info=hidden"], {
    @snap;
});

// Scrolled half-way: thumb should have moved toward the top.
insta_test!(opt_scrollbar_scrolled, SCROLLBAR_ITEMS, &["--info=hidden"], {
    @snap;
    @action Up(10);
    @snap;
    @action Up(20);
    @snap;
});

// --no-scrollbar: no scrollbar column, full 80-column list.
insta_test!(opt_no_scrollbar, SCROLLBAR_ITEMS, &["--info=hidden", "--no-scrollbar"], {
    @snap;
});

// --scrollbar="" is equivalent to --no-scrollbar.
insta_test!(opt_scrollbar_empty_string, SCROLLBAR_ITEMS, &["--info=hidden", "--scrollbar="], {
    @snap;
});

// --scrollbar="|": only the thumb character is shown; track/begin/end are hidden.
insta_test!(opt_scrollbar_custom_thumb, SCROLLBAR_ITEMS, &["--info=hidden", "--scrollbar=|"], {
    @snap;
});

// --scrollbar with reverse layout: verify scrollbar works in TopToBottom direction too.
insta_test!(opt_scrollbar_reverse, SCROLLBAR_ITEMS, &["--info=hidden", "--layout=reverse"], {
    @snap;
});

// The thumb is styled with the themed `scrollbar` color (defaulting to the border
// color) instead of inheriting the fg/bg of the row it is drawn over. After Up(10)
// the thumb overlaps the current line (`> item_11`), which --highlight-line fills
// with the `current` style; the thumb cell in the rightmost column keeps
// fg=Indexed(59) (dark256's scrollbar default), not the current-line highlight.
insta_test!(opt_scrollbar_thumb_style, SCROLLBAR_ITEMS, &["--info=hidden", "--highlight-line"], {
    @action Up(10);
    @snap;
    @snap_color;
});

// The `scrollbar` color key themes the thumb independently of the border. With
// --color=scrollbar:208 the thumb cell renders fg=Indexed(208) on every row,
// including the --highlight-line current line (row 12), where it keeps fg=208
// over the current-line background rather than inheriting it.
insta_test!(opt_scrollbar_thumb_color, SCROLLBAR_ITEMS, &["--info=hidden", "--highlight-line", "--color=scrollbar:208"], {
    @action Up(10);
    @snap_color;
});

// Basic rendering: prompt, counters, and the item list.
insta_test!(vanilla_basic, ["1", "2", "3"], &[], {
    @snap;
});

// --read0 splits the NUL-separated stream into three items.
insta_test!(opt_read0, @bytes b"a\0b\0c", &["--read0"], {
    @snap;
});

// A NUL delimiter with --with-nth shows only the second field of the single item.
insta_test!(opt_null_delimiter_with_nth, @bytes b"a\0b\0c", &[r"--delimiter", r"\x00", "--with-nth", "2"], {
    @snap;
});

// A NUL delimiter with --nth restricts matching to the second field ("b"):
// typing 'c' matches nothing, while 'b' matches the whole item.
insta_test!(opt_null_delimiter_nth, @bytes b"a\0b\0c", &[r"--delimiter", r"\x00", "--nth", "2"], {
    @snap;
    @char 'c';
    @snap;
    @key Backspace;
    @char 'b';
    @snap;
});

// --pre-select-file marks items "b" and "c" as selected at startup.
#[test]
fn opt_pre_select_file() -> color_eyre::Result<()> {
    use std::io::Write;

    let mut pre_select = tempfile::NamedTempFile::new()?;
    pre_select.write_all(b"b\nc")?;
    let path = pre_select.path().to_str().expect("utf-8 temp path");

    let options = common::insta::parse_options(&["-m", "--pre-select-file", path]);
    let mut h = common::insta::enter_items(["a", "b", "c"], options)?;
    snap!(
        h,
        "input: items [\"a\", \"b\", \"c\"]\noptions: -m --pre-select-file <file with b,c>"
    );
    Ok(())
}

// Reserved fzf-compatibility options must be accepted by the parser.
#[test]
fn opt_reserved_options_parse() {
    let reserved = [
        "--extended",
        "--literal",
        "--no-mouse",
        "--hscroll-off=10",
        "--filepath-word",
        "--jump-labels=CHARS",
        "--no-bold",
        "--history-size=10",
    ];
    for option in reserved {
        // parse_options panics on a parse/build failure, which is the assertion.
        let _ = common::insta::parse_options(&[option]);
    }
}

// A flag accepted more than once (long/short, with or without `=`) must parse.
#[test]
fn opt_multiple_flags_parse() {
    let combos = [
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
        "--inline-info --inline-info",
        "--no-bold --no-bold",
        "--print-query --print-query",
        "--print-cmd --print-cmd",
        "--print0 --print0",
        "--sync --sync",
        "--extended --extended",
        "--no-sort --no-sort",
        "--exit-0 --exit-0",
    ];
    for combo in combos {
        let args = shlex::split(combo).expect("valid shell tokens");
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let _ = common::insta::parse_options(&refs);
    }
}
