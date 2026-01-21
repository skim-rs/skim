#[allow(dead_code)]
#[macro_use]
mod common;

insta_test!(insta_keys_basic, @cmd "seq 1 100000", &[], {
    @snap;
    @type "99";
    @snap;
});

// Input navigation keys

insta_test!(insta_keys_arrows, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @key Left;
    @char '|';
    @snap;
    @key Right;
    @char '|';
    @snap;
});

insta_test!(insta_keys_ctrl_arrows, [""], &["-q", "foo bar foo-bar"], {
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

insta_test!(insta_keys_ctrl_a, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @ctrl 'a';
    @char '|';
    @snap;
});

insta_test!(insta_keys_ctrl_b, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @ctrl 'a';
    @char '|';
    @snap;
    @ctrl 'f';
    @char '|';
    @snap;
});

insta_test!(insta_keys_ctrl_e, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @ctrl 'a';
    @char '|';
    @snap;
    @ctrl 'e';
    @char '|';
    @snap;
});

insta_test!(insta_keys_ctrl_f, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @ctrl 'a';
    @char '|';
    @snap;
    @ctrl 'f';
    @char '|';
    @snap;
});

insta_test!(insta_keys_ctrl_h, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @ctrl 'h';
    @char '|';
    @snap;
});

insta_test!(insta_keys_alt_b, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @alt 'b';
    @char '|';
    @snap;
});

insta_test!(insta_keys_alt_f, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @ctrl 'a';
    @char '|';
    @snap;
    @alt 'f';
    @char '|';
    @snap;
});

// Input manipulation keys

insta_test!(insta_keys_bspace, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @key Backspace;
    @char '|';
    @snap;
});

insta_test!(insta_keys_ctrl_d, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @ctrl 'a';
    @char '|';
    @snap;
    @ctrl 'd';
    @char '|';
    @snap;
});

insta_test!(insta_keys_ctrl_u, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @ctrl 'u';
    @char '|';
    @snap;
});

insta_test!(insta_keys_ctrl_w, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @ctrl 'w';
    @char '|';
    @snap;
});

insta_test!(insta_keys_ctrl_y, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @alt Backspace;
    @char '|';
    @snap;
    @ctrl 'y';
    @char '|';
    @snap;
});

insta_test!(insta_keys_alt_d, [""], &["-q", "foo bar foo-bar"], {
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

insta_test!(insta_keys_alt_bspace, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @alt Backspace;
    @char '|';
    @snap;
});

// Results navigation keys

insta_test!(insta_keys_ctrl_k, @cmd "seq 1 100000", &[], {
    @snap;
    @ctrl 'k';
    @snap;
});

insta_test!(insta_keys_tab, @cmd "seq 1 100000", &[], {
    @snap;
    @ctrl 'k';
    @snap;
    @key Tab;
    @snap;
});

insta_test!(insta_keys_btab, @cmd "seq 1 100000", &[], {
    @snap;
    @key BackTab;
    @snap;
});

insta_test!(insta_keys_tab_empty, [""], &[], {
    @snap;
    @key Tab;
    @snap;
    @char 'a';
    @snap;
});
