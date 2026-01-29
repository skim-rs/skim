#[allow(dead_code)]
#[macro_use]
mod common;

// Using 100 items to test filtering and navigation (representative of larger datasets)
insta_test!(keys_basic, [
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "10",
    "11", "12", "13", "14", "15", "16", "17", "18", "19", "20",
    "21", "22", "23", "24", "25", "26", "27", "28", "29", "30",
    "31", "32", "33", "34", "35", "36", "37", "38", "39", "40",
    "41", "42", "43", "44", "45", "46", "47", "48", "49", "50",
    "51", "52", "53", "54", "55", "56", "57", "58", "59", "60",
    "61", "62", "63", "64", "65", "66", "67", "68", "69", "70",
    "71", "72", "73", "74", "75", "76", "77", "78", "79", "80",
    "81", "82", "83", "84", "85", "86", "87", "88", "89", "90",
    "91", "92", "93", "94", "95", "96", "97", "98", "99", "100"
], &[], {
    @snap;
    @type "99";
    @snap;
});

// Input navigation keys

insta_test!(keys_arrows, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @key Left;
    @char '|';
    @snap;
    @key Right;
    @char '|';
    @snap;
});

insta_test!(keys_ctrl_arrows, [""], &["-q", "foo bar foo-bar"], {
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

insta_test!(keys_ctrl_a, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @ctrl 'a';
    @char '|';
    @snap;
});

insta_test!(keys_ctrl_b, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @ctrl 'a';
    @char '|';
    @snap;
    @ctrl 'f';
    @char '|';
    @snap;
});

insta_test!(keys_ctrl_e, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @ctrl 'a';
    @char '|';
    @snap;
    @ctrl 'e';
    @char '|';
    @snap;
});

insta_test!(keys_ctrl_f, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @ctrl 'a';
    @char '|';
    @snap;
    @ctrl 'f';
    @char '|';
    @snap;
});

insta_test!(keys_ctrl_h, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @ctrl 'h';
    @char '|';
    @snap;
});

insta_test!(keys_alt_b, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @alt 'b';
    @char '|';
    @snap;
});

insta_test!(keys_alt_f, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @ctrl 'a';
    @char '|';
    @snap;
    @alt 'f';
    @char '|';
    @snap;
});

// Input manipulation keys

insta_test!(keys_bspace, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @key Backspace;
    @char '|';
    @snap;
});

insta_test!(keys_ctrl_c, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @ctrl 'c';
    @exited 130;
});
insta_test!(keys_ctrl_d, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @ctrl 'd';
    @exited 130;
});

insta_test!(keys_ctrl_u, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @ctrl 'u';
    @char '|';
    @snap;
});

insta_test!(keys_ctrl_w, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @ctrl 'w';
    @char '|';
    @snap;
});

insta_test!(keys_ctrl_y, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @alt Backspace;
    @char '|';
    @snap;
    @ctrl 'y';
    @char '|';
    @snap;
});

insta_test!(keys_alt_d, [""], &["-q", "foo bar foo-bar"], {
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

insta_test!(keys_alt_bspace, [""], &["-q", "foo bar foo-bar"], {
    @snap;
    @alt Backspace;
    @char '|';
    @snap;
});

// Results navigation keys

insta_test!(keys_ctrl_k, [
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "10",
    "11", "12", "13", "14", "15", "16", "17", "18", "19", "20"
], &[], {
    @snap;
    @ctrl 'k';
    @snap;
});

insta_test!(keys_tab, [
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "10",
    "11", "12", "13", "14", "15", "16", "17", "18", "19", "20"
], &[], {
    @snap;
    @ctrl 'k';
    @snap;
    @key Tab;
    @snap;
});

insta_test!(keys_btab, [
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "10",
    "11", "12", "13", "14", "15", "16", "17", "18", "19", "20"
], &[], {
    @snap;
    @key BackTab;
    @snap;
});

insta_test!(keys_tab_empty, [""], &[], {
    @snap;
    @key Tab;
    @snap;
    @char 'a';
    @snap;
});
