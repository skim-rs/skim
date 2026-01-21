#[allow(dead_code)]
#[macro_use]
mod common;

insta_test!(insta_opt_with_nth_preview, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth", "2..", "--preview", "echo X{1}Y"], {
    @snap;
});

// Use info=hidden to hide the spinner
insta_test!(insta_opt_min_query_length, ["line1", "line2", "line3"], &["--min-query-length", "3", "--info", "hidden"], {
    @snap;
    @type "li";
    @snap;
    @char 'n';
    @snap;
});

// Use info=hidden to hide the spinner
insta_test!(insta_opt_min_query_length_interactive, @interactive, &["-i", "--min-query-length", "3", "--cmd", "printf 'line1\\nline2\\nline3'", "--info", "hidden"], {
    @snap;
    @type "li";
    @snap;
    @char 'n';
    @snap;
});

insta_test!(insta_opt_with_nth_1, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth", "1"], {
    @snap;
});

insta_test!(insta_opt_with_nth_2, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth", "2"], {
    @snap;
});

insta_test!(insta_opt_with_nth_4, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth", "4"], {
    @snap;
});

insta_test!(insta_opt_with_nth_oob, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth", "5"], {
    @snap;
});

insta_test!(insta_opt_with_nth_neg_1, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth=-1"], {
    @snap;
});

insta_test!(insta_opt_with_nth_neg_2, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth=-2"], {
    @snap;
});

insta_test!(insta_opt_with_nth_neg_4, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth=-4"], {
    @snap;
});

insta_test!(insta_opt_with_nth_oob_4, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth=-5"], {
    @snap;
});

insta_test!(insta_opt_with_nth_range_to_end, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth", "2.."], {
    @snap;
});

insta_test!(insta_opt_with_nth_range_from_start, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth", "..3"], {
    @snap;
});

insta_test!(insta_opt_with_nth_range_closed, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth", "2..3"], {
    @snap;
});

insta_test!(insta_opt_with_nth_range_desc, ["f1,f2,f3,f4"], &["--delimiter", ",", "--with-nth", "3..2"], {
    @snap;
});

insta_test!(insta_opt_nth_1, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth", "1"], {
    @snap;
    @char '1';
    @snap;
    @ctrl 'w';
    @snap;
    @char '2';
    @snap;
});

insta_test!(insta_opt_nth_2, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth", "2"], {
    @snap;
    @char '2';
    @snap;
    @ctrl 'w';
    @snap;
    @char '1';
    @snap;
});

insta_test!(insta_opt_nth_4, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth", "4"], {
    @snap;
    @char '4';
    @snap;
    @ctrl 'w';
    @snap;
    @char '1';
    @snap;
});

insta_test!(insta_opt_nth_oob, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth", "5"], {
    @snap;
    @char '1';
    @snap;
});

insta_test!(insta_opt_nth_neg_1, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth=-1"], {
    @snap;
    @char '4';
    @snap;
    @ctrl 'w';
    @snap;
    @char '1';
    @snap;
});

insta_test!(insta_opt_nth_neg_2, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth=-2"], {
    @snap;
    @char '3';
    @snap;
    @ctrl 'w';
    @snap;
    @char '1';
    @snap;
});

insta_test!(insta_opt_nth_neg_4, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth=-4"], {
    @snap;
    @char '1';
    @snap;
    @ctrl 'w';
    @snap;
    @char '2';
    @snap;
});

insta_test!(insta_opt_nth_neg_oob, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth=-5"], {
    @snap;
    @char '1';
    @snap;
});

insta_test!(insta_opt_nth_range_to_end, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth", "2.."], {
    @snap;
    @char '3';
    @snap;
    @ctrl 'w';
    @snap;
    @char '1';
    @snap;
});

insta_test!(insta_opt_nth_range_from_start, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth", "..3"], {
    @snap;
    @char '1';
    @snap;
    @ctrl 'w';
    @snap;
    @char '4';
    @snap;
});

insta_test!(insta_opt_nth_range_closed, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth", "2..3"], {
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

insta_test!(insta_opt_nth_range_dec, ["f1,f2,f3,f4"], &["--delimiter", ",", "--nth", "3..2"], {
    @snap;
    @char '1';
    @snap;
});

insta_test!(insta_opt_hscroll_begin, [&format!("b{}", &["a"; 1000].join(""))], &["-q", "b"], {
    @snap;
});

insta_test!(insta_opt_hscroll_middle, [&format!("{}b{}", &["a"; 1000].join(""), &["a"; 1000].join(""))], &["-q", "b"], {
    @snap;
});

insta_test!(insta_opt_hscroll_end, [&format!("{}b", &["a"; 1000].join(""))], &["-q", "b"], {
    @snap;
});

insta_test!(insta_opt_no_hscroll, [&format!("{}b", &["a"; 1000].join(""))], &["-q", "b", "--no-hscroll"], {
    @snap;
});

insta_test!(insta_opt_tabstop_default, ["a\tb"], &[], {
    @snap;
});

insta_test!(insta_opt_tabstop_1, ["a\tb"], &["--tabstop", "1"], {
    @snap;
});

insta_test!(insta_opt_tabstop_3, ["aa\tb"], &["--tabstop", "3"], {
    @snap;
});

insta_test!(insta_opt_info_control, ["a", "b", "c"], &[], {
    @snap;
    @char 'a';
    @snap;
});

insta_test!(insta_opt_info_default, ["a", "b", "c"], &["--info", "default"], {
    @snap;
    @char 'a';
    @snap;
});

insta_test!(insta_opt_no_info, ["a", "b", "c"], &["--no-info"], {
    @snap;
});

insta_test!(insta_opt_info_hidden, ["a", "b", "c"], &["--info", "hidden"], {
    @snap;
});

insta_test!(insta_opt_info_inline, ["a", "b", "c"], &["--info", "inline"], {
    @snap;
    @char 'a';
    @snap;
});

insta_test!(insta_opt_inline_info, ["a", "b", "c"], &["--inline-info"], {
    @snap;
    @char 'a';
    @snap;
});

insta_test!(insta_opt_header_only, ["a", "b", "c"], &["--header", "test_header"], {
    @snap;
});

insta_test!(insta_opt_header_inline_info, ["a", "b", "c"], &["--header", "test_header", "--inline-info"], {
    @snap;
});

insta_test!(insta_opt_header_reverse, ["a", "b", "c"], &["--header", "test_header", "--reverse"], {
    @snap;
});

insta_test!(insta_opt_header_reverse_inline_info, ["a", "b", "c"], &["--header", "test_header", "--reverse", "--inline-info"], {
    @snap;
});

insta_test!(insta_opt_header_lines_1, ["a", "b", "c"], &["--header-lines", "1"], {
    @snap;
});

insta_test!(insta_opt_header_lines_all, ["a", "b", "c"], &["--header-lines", "4"], {
    @snap;
});

insta_test!(insta_opt_header_lines_inline_info, ["a", "b", "c"], &["--header-lines", "1", "--inline-info"], {
    @snap;
});

insta_test!(insta_opt_header_lines_reverse, ["a", "b", "c"], &["--header-lines", "1", "--reverse"], {
    @snap;
});

insta_test!(insta_opt_header_lines_reverse_inline_info, ["a", "b", "c"], &["--header-lines", "1", "--reverse", "--inline-info"], {
    @snap;
});

insta_test!(insta_opt_skip_to_pattern, ["a/b/c"], &["--skip-to-pattern", "[^/]*$", "--bind", "ctrl-a:scroll-left", "--bind", "ctrl-x:scroll-right"], {
    @snap;
    @ctrl 'a';
    @snap;
    @ctrl 'x';
    @snap;
});

insta_test!(insta_opt_multi, ["a", "b", "c"], &["--multi"], {
    @snap;
    @shift Tab;
    @snap;
    @shift Tab;
    @snap;
});

insta_test!(insta_opt_pre_select_n, ["a", "b", "c"], &["-m", "--pre-select-n", "2"], {
    @snap;
});

insta_test!(insta_opt_pre_select_items, ["a", "b", "c"], &["-m", "--pre-select-items", "$'b\\nc'"], {
    @snap;
});

insta_test!(insta_opt_pre_select_pat, ["a", "b", "c"], &["-m", "--pre-select-pat", "[b|c]"], {
    @snap;
});

insta_test!(insta_opt_no_clear_if_empty, @interactive, &["-i", "--no-clear-if-empty", "-c", "printf {}", "--cmd-query", "xxxx"], {
    @snap;
    @ctrl 'w';
    @snap;
});

insta_test!(insta_opt_tac, ["a", "b"], &["--tac"], {
    @snap;
});

insta_test!(insta_opt_tac_with_header_lines, ["a", "b", "c", "d", "e"], &["--tac", "--header-lines", "2"], {
    @snap;
});

insta_test!(insta_opt_replstr, @interactive, &["-I", "..", "-i", "-c", "echo foo {} .."], {
    @snap;
    @char 'a';
    @snap;
});

insta_test!(insta_opt_selector, ["a", "b", "c"], &["--selector", "$"], {
    @snap;
});

insta_test!(insta_opt_multi_selector, ["a", "b", "c"], &["--multi-selector", "$", "-m"], {
    @snap;
    @shift Tab;
    @snap;
});

insta_test!(insta_opt_cycle, ["a", "b", "c"], &["--cycle"], {
    @snap;
    @key Down;
    @snap;
    @key Up;
    @snap;
});

insta_test!(insta_opt_cycle_header_lines, ["a", "b", "c", "d"], &["--cycle", "--header-lines", "1"], {
    @snap;
    @key Down;
    @snap;
    @key Up;
    @snap;
});

insta_test!(insta_opt_disabled, ["a", "b", "c", "d"], &["--disabled"], {
    @snap;
    @char 'b';
    @snap;
});

const LONG_INPUT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
insta_test!(insta_opt_wrap, [LONG_INPUT], &["--wrap"], {
    @snap;
});

insta_test!(insta_opt_multiple_flags_prompt, [""], &["--prompt", "a", "--prompt", "b", "-p", "c"], {
    @snap;
});

insta_test!(insta_opt_multiple_flags_cmd_prompt, @interactive, &["-i", "--cmd-prompt", "a", "--cmd-prompt", "c", "--cmd", "echo"], {
    @snap;
});

insta_test!(insta_opt_multiple_flags_cmd_query, @interactive, &["-i", "--cmd-query", "a", "--cmd-query", "b", "--cmd", "echo"], {
    @snap;
});

insta_test!(insta_opt_multiple_flags_interactive, @interactive, &["-i", "--interactive", "--interactive", "--cmd", "echo"], {
    @snap;
});

insta_test!(insta_opt_multiple_flags_reverse, [""], &["--reverse", "--reverse"], {
    @snap;
});

insta_test!(insta_opt_multiple_flags_combined_nth, ["a b c", "d e f"], &["--nth", "1,2"], {
    @snap;
    @char 'c';
    @snap;
});

insta_test!(insta_opt_multiple_flags_combined_with_nth, ["a b c", "d e f"], &["--with-nth", "1,2"], {
    @snap;
});

insta_test!(insta_opt_multiple_flags_reverse_and_layout, ["a b c", "d e f"], &["--reverse", "--layout", "default"], {
    @snap;
});

insta_test!(insta_opt_multiple_flags_layout_and_reverse, ["a b c", "d e f"], &["--layout", "default", "--reverse"], {
    @snap;
});
