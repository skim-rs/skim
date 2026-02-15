#[allow(dead_code)]
#[macro_use]
mod common;

// Basic interactive mode test
insta_test!(keys_interactive_basic, ["1", "2", "3", "4"], &["-i"], {
    @snap;
    @type "99";
    @snap;
});

// Input navigation keys

insta_test!(keys_interactive_arrows, @interactive, &["-i", "--cmd-query", "foo bar foo-bar"], {
    @snap;
    @key Left;
    @char '|';
    @snap;
    @key Right;
    @char '|';
    @snap;
});

insta_test!(keys_interactive_ctrl_arrows, @interactive, &["-i", "--cmd-query", "foo bar foo-bar"], {
    @snap;
    @ctrl Left;
    @char '|';
    @snap;
    @ctrl Left;
    @char '|';
    @snap;
    @ctrl Right;
    @char '|';
    @snap;
});

insta_test!(keys_interactive_ctrl_a, @interactive, &["-i", "--cmd-query", "foo bar foo-bar"], {
    @snap;
    @ctrl 'a';
    @char '|';
    @snap;
});

insta_test!(keys_interactive_ctrl_b, @interactive, &["-i", "--cmd-query", "foo bar foo-bar"], {
    @snap;
    @ctrl 'a';
    @char '|';
    @snap;
    @ctrl 'f';
    @char '|';
    @snap;
});

insta_test!(keys_interactive_ctrl_e, @interactive, &["-i", "--cmd-query", "foo bar foo-bar"], {
    @snap;
    @ctrl 'a';
    @char '|';
    @snap;
    @ctrl 'e';
    @char '|';
    @snap;
});

insta_test!(keys_interactive_ctrl_f, @interactive, &["-i", "--cmd-query", "foo bar foo-bar"], {
    @snap;
    @ctrl 'a';
    @char '|';
    @snap;
    @ctrl 'f';
    @char '|';
    @snap;
});

insta_test!(keys_interactive_ctrl_h, @interactive, &["-i", "--cmd-query", "foo bar foo-bar"], {
    @snap;
    @ctrl 'h';
    @char '|';
    @snap;
});

insta_test!(keys_interactive_alt_b, @interactive, &["-i", "--cmd-query", "foo bar foo-bar"], {
    @snap;
    @alt 'b';
    @char '|';
    @snap;
});

insta_test!(keys_interactive_alt_f, @interactive, &["-i", "--cmd-query", "foo bar foo-bar"], {
    @snap;
    @ctrl 'a';
    @char '|';
    @snap;
    @alt 'f';
    @char '|';
    @snap;
});

// Input manipulation keys

insta_test!(keys_interactive_bspace, @interactive, &["-i", "--cmd-query", "foo bar foo-bar"], {
    @snap;
    @key Backspace;
    @char '|';
    @snap;
});

insta_test!(keys_interactive_ctrl_c, @interactive, &["-i", "--cmd-query", "foo bar foo-bar"], {
    @snap;
    @ctrl 'c';
    @exited 130;
});
insta_test!(keys_interactive_ctrl_d, @interactive, &["-i", "--cmd-query", "foo bar foo-bar"], {
    @snap;
    @ctrl 'd';
    @exited 130;
});

insta_test!(keys_interactive_ctrl_u, @interactive, &["-i", "--cmd-query", "foo bar foo-bar"], {
    @snap;
    @ctrl 'u';
    @char '|';
    @snap;
});

insta_test!(keys_interactive_ctrl_w, @interactive, &["-i", "--cmd-query", "foo bar foo-bar"], {
    @snap;
    @ctrl 'w';
    @char '|';
    @snap;
});

insta_test!(keys_interactive_ctrl_y, @interactive, &["-i", "--cmd-query", "foo bar foo-bar"], {
    @snap;
    @alt Backspace;
    @char '|';
    @snap;
    @ctrl 'y';
    @char '|';
    @snap;
});

insta_test!(keys_interactive_alt_d, @interactive, &["-i", "--cmd-query", "foo bar foo-bar"], {
    @snap;
    @ctrl Left;
    @char '|';
    @snap;
    @ctrl Left;
    @char '|';
    @snap;
    @alt 'd';
    @char '|';
    @snap;
});

insta_test!(keys_interactive_alt_bspace, @interactive, &["-i", "--cmd-query", "foo bar foo-bar"], {
    @snap;
    @alt Backspace;
    @char '|';
    @snap;
});

// Results navigation keys

insta_test!(keys_interactive_ctrl_k, ["1", "2", "3", "4"], &["-i"], {
    @snap;
    @ctrl 'k';
    @snap;
});

insta_test!(keys_interactive_tab, ["1", "2", "3", "4"], &["-i"], {
    @snap;
    @ctrl 'k';
    @snap;
    @key Tab;
    @snap;
});

insta_test!(keys_interactive_btab, ["1", "2", "3", "4"], &["-i"], {
    @snap;
    @key BackTab;
    @snap;
});

// Tests Enter and Ctrl-M for accepting selection

insta_test!(keys_interactive_enter, ["1", "2", "3", "4"], &["-i"], {
    @snap;
    @key Enter;
    @assert(|h: &common::insta::TestHarness| h.skim.app().should_quit);
    @assert(|h: &common::insta::TestHarness| h.skim.app().item_list.selected().unwrap().text() == "1");
});
