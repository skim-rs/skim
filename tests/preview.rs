#[allow(dead_code)]
#[macro_use]
mod common;

const PREVIEW: &str = "printf \"=%.0s\\n\" $(seq 1 1000)";

insta_test!(insta_preview_preserve_quotes, ["'\"ABC\"'"], &["--preview", "echo X{}X"], {
    @snap;
});

insta_test!(insta_preview_nul_char, ["a\0b"], &["--preview", "printf \"{}\" | hexdump -C"], {
    @snap;
});

insta_test!(insta_preview_window_left, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "left"], {
    @snap;
});

insta_test!(insta_preview_window_down, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "down"], {
    @snap;
});

insta_test!(insta_preview_window_up, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "up"], {
    @snap;
});

insta_test!(insta_preview_offset_fixed, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "left:+123"], {
    @snap;
});

insta_test!(insta_preview_offset_expr, ["123 321"], &["--preview", PREVIEW, "--preview-window", "left:+{2}"], {
    @snap;
});

insta_test!(insta_preview_offset_fixed_and_expr, ["123 321"], &["--preview", PREVIEW, "--preview-window", "left:+{2}-2"], {
    @snap;
});

insta_test!(insta_preview_nowrap, ["x"], &["--preview", "echo a bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "--preview-window", "up"], {
    @snap;
});

insta_test!(insta_preview_wrap, ["x"], &["--preview", "echo a      bbbbbbbb", "--preview-window", "left:10:wrap"], {
    @snap;
});
