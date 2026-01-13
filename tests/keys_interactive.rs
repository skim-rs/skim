#[allow(dead_code)]
#[macro_use]
mod common;

use common::Keys::*;

sk_test!(keys_interactive_basic, @cmd "seq 1 100000", &["-i"], {
    @capture[0] starts_with("c>");
    @capture[1] starts_with("  100000");
    @keys Str("99");
    @capture[0] eq("c> 99");
    @capture[1] starts_with("  100000/100000");
    @capture[2] eq("> 1");
});

// Input navigation keys

sk_test!(keys_interactive_arrows, "", &["-i", "--cmd-query", "'foo bar foo-bar'"], {
    @capture[0] starts_with("c>");
    @keys Left, Key('|');
    @capture[0] eq("c> foo bar foo-ba|r");
    @keys Right, Key('|');
    @capture[0] eq("c> foo bar foo-ba|r|");
});

sk_test!(keys_interactive_ctrl_arrows, "", &["-i", "--cmd-query", "'foo bar foo-bar'"], {
    @capture[0] starts_with("c>");
    @keys Ctrl(&Left), Key('|');
    @capture[0] eq("c> foo bar foo-|bar");
    @keys Ctrl(&Left), Key('|');
    @capture[0] eq("c> foo bar |foo-|bar");
    @keys Ctrl(&Right), Key('|');
    @capture[0] eq("c> foo bar |foo-|bar|");
});

sk_test!(keys_interactive_ctrl_a, "", &["-i", "--cmd-query", "'foo bar foo-bar'"], {
    @capture[0] starts_with("c>");
    @keys Ctrl(&Key('a')), Key('|');
    @capture[0] eq("c> |foo bar foo-bar");
});

sk_test!(keys_interactive_ctrl_b, "", &["-i", "--cmd-query", "'foo bar foo-bar'"], {
    @capture[0] starts_with("c>");
    @keys Ctrl(&Key('a')), Key('|');
    @capture[0] eq("c> |foo bar foo-bar");
    @keys Ctrl(&Key('f')), Key('|');
    @capture[0] eq("c> |f|oo bar foo-bar");
});

sk_test!(keys_interactive_ctrl_e, "", &["-i", "--cmd-query", "'foo bar foo-bar'"], {
    @capture[0] starts_with("c>");
    @keys Ctrl(&Key('a')), Key('|');
    @capture[0] eq("c> |foo bar foo-bar");
    @keys Ctrl(&Key('e')), Key('|');
    @capture[0] eq("c> |foo bar foo-bar|");
});

sk_test!(keys_interactive_ctrl_f, "", &["-i", "--cmd-query", "'foo bar foo-bar'"], {
    @capture[0] starts_with("c>");
    @keys Ctrl(&Key('a')), Key('|');
    @capture[0] eq("c> |foo bar foo-bar");
    @keys Ctrl(&Key('f')), Key('|');
    @capture[0] eq("c> |f|oo bar foo-bar");
});

sk_test!(keys_interactive_ctrl_h, "", &["-i", "--cmd-query", "'foo bar foo-bar'"], {
    @capture[0] starts_with("c>");
    @keys Ctrl(&Key('h')), Key('|');
    @capture[0] eq("c> foo bar foo-ba|");
});

sk_test!(keys_interactive_alt_b, "", &["-i", "--cmd-query", "'foo bar foo-bar'"], {
    @capture[0] starts_with("c>");
    @keys Alt(&Key('b')), Key('|');
    @capture[0] eq("c> foo bar foo-|bar");
});

sk_test!(keys_interactive_alt_f, "", &["-i", "--cmd-query", "'foo bar foo-bar'"], {
    @capture[0] starts_with("c>");
    @keys Ctrl(&Key('a')), Key('|');
    @capture[0] eq("c> |foo bar foo-bar");
    @keys Alt(&Key('f')), Key('|');
    @capture[0] eq("c> |foo| bar foo-bar");
});

// Input manipulation keys

sk_test!(keys_interactive_bspace, "", &["-i", "--cmd-query", "'foo bar foo-bar'"], {
    @capture[0] starts_with("c>");
    @keys BSpace, Key('|');
    @capture[0] eq("c> foo bar foo-ba|");
});

sk_test!(keys_interactive_ctrl_d, "", &["-i", "--cmd-query", "'foo bar foo-bar'"], {
    @capture[0] starts_with("c>");
    @keys Ctrl(&Key('a')), Key('|');
    @capture[0] eq("c> |foo bar foo-bar");
    @keys Ctrl(&Key('d')), Key('|');
    @capture[0] eq("c> ||oo bar foo-bar");
});

sk_test!(keys_interactive_ctrl_u, "", &["-i", "--cmd-query", "'foo bar foo-bar'"], {
    @capture[0] starts_with("c>");
    @keys Ctrl(&Key('u')), Key('|');
    @capture[0] eq("c> |");
});

sk_test!(keys_interactive_ctrl_w, "", &["-i", "--cmd-query", "'foo bar foo-bar'"], {
    @capture[0] starts_with("c>");
    @keys Ctrl(&Key('w')), Key('|');
    @capture[0] eq("c> foo bar |");
});

sk_test!(keys_interactive_ctrl_y, "", &["-i", "--cmd-query", "'foo bar foo-bar'"], {
    @capture[0] starts_with("c>");
    @keys Alt(&BSpace), Key('|');
    @capture[0] eq("c> foo bar foo-|");
    @keys Ctrl(&Key('y')), Key('|');
    @capture[0] eq("c> foo bar foo-|bar|");
});

sk_test!(keys_interactive_alt_d, "", &["-i", "--cmd-query", "'foo bar foo-bar'"], {
    @capture[0] starts_with("c>");
    @keys Ctrl(&Left), Key('|');
    @capture[0] eq("c> foo bar foo-|bar");
    @keys Ctrl(&Left), Key('|');
    @capture[0] eq("c> foo bar |foo-|bar");
    @keys Alt(&Key('d')), Key('|');
    @capture[0] eq("c> foo bar ||-|bar");
});

sk_test!(keys_interactive_alt_bspace, "", &["-i", "--cmd-query", "'foo bar foo-bar'"], {
    @capture[0] starts_with("c>");
    @keys Alt(&BSpace), Key('|');
    @capture[0] eq("c> foo bar foo-|");
});

// Results navigation keys

sk_test!(keys_interactive_ctrl_k, @cmd "seq 1 100000", &["-i"], {
    @capture[0] starts_with("c>");
    @capture[1] starts_with("  100000");
    @keys Ctrl(&Key('k'));
    @capture[2] eq("  1");
    @capture[3] eq("> 2");
});

sk_test!(keys_interactive_tab, @cmd "seq 1 100000", &["-i"], {
    @capture[0] starts_with("c>");
    @capture[1] starts_with("  100000");
    @keys Ctrl(&Key('k'));
    @capture[2] eq("  1");
    @capture[3] eq("> 2");
    @keys Tab;
    @capture[2] eq("> 1");
    @capture[3] eq("  2");
});

sk_test!(keys_interactive_btab, @cmd "seq 1 100000", &["-i"], {
    @capture[0] starts_with("c>");
    @capture[1] starts_with("  100000");
    @keys BTab;
    @capture[2] eq("  1");
    @capture[3] eq("> 2");
});

sk_test!(keys_interactive_enter, @cmd "seq 1 100000", &["-i"], {
    @capture[0] starts_with("c>");
    @capture[1] starts_with("  100000");
    @keys Enter;
    @capture[0] ne("c>");
    @output[0] eq("1");
});

sk_test!(keys_interactive_ctrl_m, @cmd "seq 1 100000", &["-i"], {
    @capture[0] starts_with("c>");
    @capture[1] starts_with("  100000");
    @keys Ctrl(&Key('m'));
    @capture[0] ne("c>");
    @output[0] eq("1");
});
