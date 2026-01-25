#[allow(dead_code)]
#[macro_use]
mod common;

fn args<'a>(extra: &[&'a str]) -> Vec<&'a str> {
    let base_args = &[
        "-q",
        "a",
        "--header",
        "header",
        "--header-lines",
        "2",
        "--prompt",
        "prompt ",
        "--selector",
        "sel ",
        "--multi-selector",
        "multi-sel ",
        "-m",
        "--pre-select-n",
        "2",
    ];
    [base_args, extra].concat()
}

insta_test!(layout_default, ["header line 1", "header line 2", "a", "b", "c", "ab", "ac"], &args(&[]), {
    @snap;
});

insta_test!(layout_border, ["header line 1", "header line 2", "a", "b", "c", "ab", "ac"], &args(&["--border"]), {
    @snap;
});

insta_test!(layout_reverse, ["header line 1", "header line 2", "a", "b", "c", "ab", "ac"], &args(&["--layout", "reverse"]), {
    @snap;
});

insta_test!(layout_reverse_border, ["header line 1", "header line 2", "a", "b", "c", "ab", "ac"], &args(&["--layout", "reverse", "--border"]), {
    @snap;
});

insta_test!(layout_reverse_list, ["header line 1", "header line 2", "a", "b", "c", "ab", "ac"], &args(&["--layout", "reverse-list"]), {
    @snap;
});

insta_test!(layout_reverse_list_border, ["header line 1", "header line 2", "a", "b", "c", "ab", "ac"], &args(&["--layout", "reverse-list", "--border"]), {
    @snap;
});
