#[allow(dead_code)]
#[macro_use]
mod common;

const PREVIEW: &str = "printf \"=%.0s\\n\" $(seq 1 1000)";

insta_test!(preview_preserve_quotes, ["'\"ABC\"'"], &["--preview", "echo X{}X"], {
    @snap;
});

insta_test!(preview_nul_char, ["a\0b"], &["--preview", "printf \"{}\" | hexdump -C"], {
    @snap;
});

insta_test!(preview_window_left, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "left"], {
    @snap;
});

insta_test!(preview_window_down, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "down"], {
    @snap;
});

insta_test!(preview_window_up, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "up"], {
    @snap;
});

insta_test!(preview_offset_fixed, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "left:+123"], {
    @snap;
});

insta_test!(preview_offset_expr, ["123 321"], &["--preview", PREVIEW, "--preview-window", "left:+{2}"], {
    @snap;
});

insta_test!(preview_offset_fixed_and_expr, ["123 321"], &["--preview", PREVIEW, "--preview-window", "left:+{2}-2"], {
    @snap;
});

insta_test!(preview_nowrap, ["x"], &["--preview", "echo a bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "--preview-window", "up"], {
    @snap;
});

insta_test!(preview_wrap, ["x"], &["--preview", "echo a      bbbbbbbb", "--preview-window", "left:10:wrap"], {
    @snap;
});

// Test that preview updates when navigating between items
insta_test!(preview_navigation, ["a", "b", "c"], &["--preview", "echo {}"], {
    @snap;
    @key Up;
    @snap;
});

insta_test!(preview_plus, ["a", "b", "c"], &["--preview", "echo {+}", "-m"], {
    @snap;
    @key Up;
    @snap;
    @shift Tab;
    @snap;
    @shift Tab;
    @snap;
    @action DeselectAll;
    @snap;
});

#[cfg(target_os = "linux")]
insta_test!(preview_no_pty_linux, ["x"], &["--preview", "tty -s && echo YES || echo NO", "--preview-window", "wrap"], {
    @snap;
});

#[cfg(target_os = "linux")]
mod preview_pty {
    use super::PREVIEW;

    insta_test!(preview_pty_flag, ["x"], &["--preview", "tty -s && echo YES || echo NO", "--preview-window", ":pty"], {
        @snap;
    });
    insta_test!(preview_pty_preserve_quotes, ["'\"ABC\"'"], &["--preview", "echo X{}X", "--preview-window", ":pty"], {
        @snap;
    });

    insta_test!(preview_pty_nul_char, ["a\0b"], &["--preview", "printf \"{}\" | hexdump -C", "--preview-window", ":pty"], {
        @snap;
    });

    insta_test!(preview_pty_window_left, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "left:pty"], {
        @snap;
    });

    insta_test!(preview_pty_window_down, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "down:pty"], {
        @snap;
    });

    insta_test!(preview_pty_window_up, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "up:pty"], {
        @snap;
    });

    insta_test!(preview_pty_offset_fixed, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "left:+123:pty"], {
        @snap;
    });

    insta_test!(preview_pty_offset_expr, ["123 321"], &["--preview", PREVIEW, "--preview-window", "left:+{2}:pty"], {
        @snap;
    });

    insta_test!(preview_pty_offset_fixed_and_expr, ["123 321"], &["--preview", PREVIEW, "--preview-window", "left:+{2}-2:pty"], {
        @snap;
    });

    insta_test!(preview_pty_nowrap, ["x"], &["--preview", "echo a bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "--preview-window", "up:pty"], {
        @snap;
    });

    insta_test!(preview_pty_wrap, ["x"], &["--preview", "echo a      bbbbbbbbbb", "--preview-window", "left:10:pty:wrap"], {
        @snap;
    });

    // Test that preview updates when navigating between items
    insta_test!(preview_pty_navigation, ["a", "b", "c"], &["--preview", "echo {}", "--preview-window", ":pty"], {
        @snap;
        @key Up;
        @snap;
    });

    insta_test!(preview_pty_plus, ["a", "b", "c"], &["--preview", "echo {+}", "-m", "--preview-window", ":pty"], {
        @snap;
        @key Up;
        @snap;
        @shift Tab;
        @snap;
        @shift Tab;
        @snap;
        @action DeselectAll;
        @snap;
    });
}
