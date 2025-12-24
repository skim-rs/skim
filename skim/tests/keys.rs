#[allow(dead_code)]
#[macro_use]
mod common;

use common::Keys::*;

sk_test!(keys_basic, @cmd "seq 1 100000", &[], {
    @lines |l| (l.len() >= 2 && l[0].starts_with(">"));
    @capture[1] starts_with("  100000");
    @keys Str("99");
    @capture[0] eq("> 99");
    @lines |l| (l.len() >= 3 && l[1].starts_with("  8146/100000"));
    @capture[2] eq("> 99");
});

// Input navigation keys

sk_test!(keys_arrows, "", &["-q", "'foo bar foo-bar'"], {
    @capture[0] starts_with(">");
    @keys Left, Key('|');
    @capture[0] eq("> foo bar foo-ba|r");
    @keys Right, Key('|');
    @capture[0] eq("> foo bar foo-ba|r|");
});

sk_test!(keys_ctrl_arrows, "", &["-q", "'foo bar foo-bar'"], {
    @capture[0] starts_with(">");
    @keys Ctrl(&Left), Key('|');
    @capture[0] eq("> foo bar foo-|bar");
    @keys Ctrl(&Left), Key('|');
    @capture[0] eq("> foo bar |foo-|bar");
    @keys Ctrl(&Right), Key('|');
    @capture[0] eq("> foo bar |foo-|bar|");
});

sk_test!(keys_ctrl_a, "", &["-q", "'foo bar foo-bar'"], {
    @capture[0] starts_with(">");
    @keys Ctrl(&Key('a')), Key('|');
    @capture[0] eq("> |foo bar foo-bar");
});

sk_test!(keys_ctrl_b, "", &["-q", "'foo bar foo-bar'"], {
    @capture[0] starts_with(">");
    @keys Ctrl(&Key('a')), Key('|');
    @capture[0] eq("> |foo bar foo-bar");
    @keys Ctrl(&Key('f')), Key('|');
    @capture[0] eq("> |f|oo bar foo-bar");
});

sk_test!(keys_ctrl_e, "", &["-q", "'foo bar foo-bar'"], {
    @capture[0] starts_with(">");
    @keys Ctrl(&Key('a')), Key('|');
    @capture[0] eq("> |foo bar foo-bar");
    @keys Ctrl(&Key('e')), Key('|');
    @capture[0] eq("> |foo bar foo-bar|");
});

sk_test!(keys_ctrl_f, "", &["-q", "'foo bar foo-bar'"], {
    @capture[0] starts_with(">");
    @keys Ctrl(&Key('a')), Key('|');
    @capture[0] eq("> |foo bar foo-bar");
    @keys Ctrl(&Key('f')), Key('|');
    @capture[0] eq("> |f|oo bar foo-bar");
});

sk_test!(keys_ctrl_h, "", &["-q", "'foo bar foo-bar'"], {
    @capture[0] starts_with(">");
    @keys Ctrl(&Key('h')), Key('|');
    @capture[0] eq("> foo bar foo-ba|");
});

sk_test!(keys_alt_b, "", &["-q", "'foo bar foo-bar'"], {
    @capture[0] starts_with(">");
    @keys Alt(&Key('b')), Key('|');
    @capture[0] eq("> foo bar foo-|bar");
});

sk_test!(keys_alt_f, "", &["-q", "'foo bar foo-bar'"], {
    @capture[0] starts_with(">");
    @keys Ctrl(&Key('a')), Key('|');
    @capture[0] eq("> |foo bar foo-bar");
    @keys Alt(&Key('f')), Key('|');
    @capture[0] eq("> |foo| bar foo-bar");
});

// Input manipulation keys

sk_test!(keys_bspace, "", &["-q", "'foo bar foo-bar'"], {
    @capture[0] starts_with(">");
    @keys BSpace, Key('|');
    @capture[0] eq("> foo bar foo-ba|");
});

sk_test!(keys_ctrl_d, "", &["-q", "'foo bar foo-bar'"], {
    @capture[0] starts_with(">");
    @keys Ctrl(&Key('a')), Key('|');
    @capture[0] eq("> |foo bar foo-bar");
    @keys Ctrl(&Key('d')), Key('|');
    @capture[0] eq("> ||oo bar foo-bar");
});

sk_test!(keys_ctrl_u, "", &["-q", "'foo bar foo-bar'"], {
    @capture[0] starts_with(">");
    @keys Ctrl(&Key('u')), Key('|');
    @capture[0] eq("> |");
});

sk_test!(keys_ctrl_w, "", &["-q", "'foo bar foo-bar'"], {
    @capture[0] starts_with(">");
    @keys Ctrl(&Key('w')), Key('|');
    @capture[0] eq("> foo bar |");
});

sk_test!(keys_ctrl_y, "", &["-q", "'foo bar foo-bar'"], {
    @capture[0] starts_with(">");
    @keys Alt(&BSpace), Key('|');
    @capture[0] eq("> foo bar foo-|");
    @keys Ctrl(&Key('y')), Key('|');
    @capture[0] eq("> foo bar foo-|bar|");
});

sk_test!(keys_alt_d, "", &["-q", "'foo bar foo-bar'"], {
    @capture[0] starts_with(">");
    @keys Ctrl(&Left), Key('|');
    @capture[0] eq("> foo bar foo-|bar");
    @keys Ctrl(&Left), Key('|');
    @capture[0] eq("> foo bar |foo-|bar");
    @keys Alt(&Key('d')), Key('|');
    @capture[0] eq("> foo bar ||-|bar");
});

sk_test!(keys_alt_bspace, "", &["-q", "'foo bar foo-bar'"], {
    @capture[0] starts_with(">");
    @keys Alt(&BSpace), Key('|');
    @capture[0] eq("> foo bar foo-|");
});

// Results navigation keys

sk_test!(keys_ctrl_k, @cmd "seq 1 100000", &[], {
    @capture[0] starts_with(">");
    @capture[1] starts_with("  100000");
    @keys Ctrl(&Key('k'));
    @capture[2] eq("  1");
    @capture[3] eq("> 2");
});

sk_test!(keys_tab, @cmd "seq 1 100000", &[], {
    @capture[0] starts_with(">");
    @capture[1] starts_with("  100000");
    @keys Ctrl(&Key('k'));
    @capture[2] eq("  1");
    @capture[3] eq("> 2");
    @keys Tab;
    @capture[2] eq("> 1");
    @capture[3] eq("  2");
});

sk_test!(keys_btab, @cmd "seq 1 100000", &[], {
    @capture[0] starts_with(">");
    @capture[1] starts_with("  100000");
    @keys BTab;
    @capture[2] eq("  1");
    @capture[3] eq("> 2");
});

sk_test!(keys_enter, @cmd "seq 1 100000", &[], {
    @capture[0] starts_with(">");
    @capture[1] starts_with("  100000");
    @keys Enter;
    @capture[0] ne(">");
    @output[0] eq("1");
});

sk_test!(keys_ctrl_m, @cmd "seq 1 100000", &[], {
    @capture[0] starts_with(">");
    @capture[1] starts_with("  100000");
    @keys Ctrl(&Key('m'));
    @capture[0] ne(">");
    @output[0] eq("1");
});
