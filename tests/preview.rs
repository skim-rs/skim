#[allow(dead_code)]
#[macro_use]
mod common;

const PREVIEW: &str = "printf \"=%.0s\\n\" $(seq 1 1000)";

insta_test!(preview_preserve_quotes, ["'\"ABC\"'"], &["--preview", "echo X{}X", "--flags", "no-preview-pty"], {
    @snap;
});

insta_test!(preview_nul_char, ["a\0b"], &["--preview", "printf \"{}\" | hexdump -C", "--flags", "no-preview-pty"], {
    @snap;
});

insta_test!(preview_window_left, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "left", "--flags", "no-preview-pty"], {
    @snap;
});

insta_test!(preview_window_down, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "down", "--flags", "no-preview-pty"], {
    @snap;
});

insta_test!(preview_window_up, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "up", "--flags", "no-preview-pty"], {
    @snap;
});

insta_test!(preview_offset_fixed, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "left:+123", "--flags", "no-preview-pty"], {
    @snap;
});

insta_test!(preview_offset_expr, ["123 321"], &["--preview", PREVIEW, "--preview-window", "left:+{2}", "--flags", "no-preview-pty"], {
    @snap;
});

insta_test!(preview_offset_fixed_and_expr, ["123 321"], &["--preview", PREVIEW, "--preview-window", "left:+{2}-2", "--flags", "no-preview-pty"], {
    @snap;
});

insta_test!(preview_nowrap, ["x"], &["--preview", "echo a bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "--preview-window", "up", "--flags", "no-preview-pty"], {
    @snap;
});

insta_test!(preview_wrap, ["x"], &["--preview", "echo a      bbbbbbbb", "--preview-window", "left:10:wrap", "--flags", "no-preview-pty"], {
    @snap;
});

// Test that preview updates when navigating between items
insta_test!(preview_navigation, ["a", "b", "c"], &["--preview", "echo {}", "--flags", "no-preview-pty"], {
    @snap;
    @key Up;
    @snap;
});

insta_test!(preview_plus, ["a", "b", "c"], &["--preview", "echo {+}", "-m", "--flags", "no-preview-pty"], {
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
insta_test!(preview_pty_linux, ["x"], &["--preview", "tty -s && echo YES || echo NO", "--flags", "no-preview-pty"], {
    @snap;
});

#[cfg(target_os = "linux")]
mod preview_no_pty {
    use super::PREVIEW;

    insta_test!(preview_no_pty_flag, ["x"], &["--preview", "tty -s && echo YES || echo NO", "--flags", "no-preview-pty", "--flags", "no-preview-pty"], {
        @snap;
    });
    insta_test!(preview_no_pty_preserve_quotes, ["'\"ABC\"'"], &["--preview", "echo X{}X", "--flags", "no-preview-pty"], {
        @snap;
    });

    insta_test!(preview_no_pty_nul_char, ["a\0b"], &["--preview", "printf \"{}\" | hexdump -C", "--flags", "no-preview-pty"], {
        @snap;
    });

    insta_test!(preview_no_pty_window_left, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "left", "--flags", "no-preview-pty"], {
        @snap;
    });

    insta_test!(preview_no_pty_window_down, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "down", "--flags", "no-preview-pty"], {
        @snap;
    });

    insta_test!(preview_no_pty_window_up, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "up", "--flags", "no-preview-pty"], {
        @snap;
    });

    insta_test!(preview_no_pty_offset_fixed, ["a", "b"], &["--preview", PREVIEW, "--preview-window", "left:+123", "--flags", "no-preview-pty"], {
        @snap;
    });

    insta_test!(preview_no_pty_offset_expr, ["123 321"], &["--preview", PREVIEW, "--preview-window", "left:+{2}", "--flags", "no-preview-pty"], {
        @snap;
    });

    insta_test!(preview_no_pty_offset_fixed_and_expr, ["123 321"], &["--preview", PREVIEW, "--preview-window", "left:+{2}-2", "--flags", "no-preview-pty"], {
        @snap;
    });

    insta_test!(preview_no_pty_nowrap, ["x"], &["--preview", "echo a bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "--preview-window", "up", "--flags", "no-preview-pty"], {
        @snap;
    });

    insta_test!(preview_no_pty_wrap, ["x"], &["--preview", "echo a      bbbbbbbb", "--preview-window", "left:10:wrap", "--flags", "no-preview-pty"], {
        @snap;
    });

    // Test that preview updates when navigating between items
    insta_test!(preview_no_pty_navigation, ["a", "b", "c"], &["--preview", "echo {}", "--flags", "no-preview-pty"], {
        @snap;
        @key Up;
        @snap;
    });

    insta_test!(preview_no_pty_plus, ["a", "b", "c"], &["--preview", "echo {+}", "-m", "--flags", "no-preview-pty"], {
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
