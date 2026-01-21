#[allow(dead_code)]
#[macro_use]
mod common;

const PREVIEW: &str = "printf \"=%.0s\\n\" $(seq 1 1000)";

insta_test!(insta_preview_preserve_quotes, @cmd "echo \"'\\\"ABC\\\"'\"", &["--preview", "echo X{}X"], {
    @snap;
});

insta_test!(insta_preview_nul_char, @cmd "echo -ne 'a\\0b'", &["--preview", "echo -en \"{}\" | hexdump -C"], {
    @snap;
});

insta_test!(insta_preview_window_left, @cmd "echo -ne 'a\\nb'", &["--preview", PREVIEW, "--preview-window", "left"], {
    @snap;
});

insta_test!(insta_preview_window_down, @cmd "echo -ne 'a\\nb'", &["--preview", PREVIEW, "--preview-window", "down"], {
    @snap;
});

insta_test!(insta_preview_window_up, @cmd "echo -ne 'a\\nb'", &["--preview", PREVIEW, "--preview-window", "up"], {
    @snap;
});

insta_test!(insta_preview_offset_fixed, @cmd "echo -ne 'a\\nb'", &["--preview", PREVIEW, "--preview-window", "left:+123"], {
    @snap;
});

insta_test!(insta_preview_offset_expr, @cmd "echo -ne '123 321'", &["--preview", PREVIEW, "--preview-window", "left:+{2}"], {
    @snap;
});

insta_test!(insta_preview_offset_fixed_and_expr, @cmd "echo -ne '123 321'", &["--preview", PREVIEW, "--preview-window", "left:+{2}-2"], {
    @snap;
});

insta_test!(insta_preview_nowrap, ["x"], &["--preview", "echo a bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "--preview-window", "up"], {
    @snap;
});

insta_test!(insta_preview_wrap, ["x"], &["--preview", "echo a      bbbbbbbb", "--preview-window", "left:10:wrap"], {
    @snap;
});
