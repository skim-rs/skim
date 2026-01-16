#[allow(dead_code)]
#[macro_use]
mod common;

const PREVIEW: &'static str = "'printf \"=%.0s\\n\" $(seq 1 1000)'";

sk_test!(preview_preserve_quotes, @cmd "echo \"'\\\"ABC\\\"'\"", &["--preview", "\"echo X{}X\""], {
    @capture[*] contains("X'\"ABC\"'X");
});

sk_test!(preview_nul_char, @cmd "echo -ne 'a\\0b'", &["--preview", "'echo -en \"{}\" | hexdump -C'"], {
    @capture[0] starts_with(">");
    @capture[*] contains("61 00 62");
});

sk_test!(preview_window_left, @cmd "echo -ne 'a\\nb'", &["--preview", PREVIEW, "--preview-window", "left"], {
    @capture[*] contains(">");
    @capture[0] starts_with("=");
});

sk_test!(preview_window_down, @cmd "echo -ne 'a\\nb'", &["--preview", PREVIEW, "--preview-window", "down"], {
    @capture[0] starts_with("=");
});

sk_test!(preview_window_up, @cmd "echo -ne 'a\\nb'", &["--preview", PREVIEW, "--preview-window", "up"], {
    @capture[0] starts_with(">");
    @capture[-1] eq("=");
});

sk_test!(preview_offset_fixed, @cmd "echo -ne 'a\\nb'", &["--preview", PREVIEW, "--preview-window", "left:+123"], {
    @capture[-1] starts_with("123");
    @capture[-1] contains("123/1000");
});

sk_test!(preview_offset_expr, @cmd "echo -ne '123 321'", &["--preview", PREVIEW, "--preview-window", "left:+{2}"], {
    @capture[-1] starts_with("321");
    @capture[-1] contains("321/1000");
});

sk_test!(preview_offset_fixed_and_expr, @cmd "echo -ne '123 321'", &["--preview", PREVIEW, "--preview-window", "left:+{2}-2"], {
    @capture[-1] starts_with("319");
    @capture[-1] contains("319/1000");
});

sk_test!(preview_nowrap, "x", &["--preview", "'echo a bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'", "--preview-window", "up"], {
    @capture[-1] starts_with("a b");
});

sk_test!(preview_wrap, "x", &["--preview", "'echo a      bbbbbbbb'", "--preview-window", "left:10:wrap"], {
    @capture[-1] trim().starts_with("a");
    @capture[-1] trim().matches("b").count().eq(&0);
    @capture[-2] trim().starts_with("bbbbbbbb");
});
