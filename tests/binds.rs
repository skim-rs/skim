#[allow(dead_code)]
#[macro_use]
mod common;

// Test if-non-matched action: deletes character when no match
insta_test!(insta_bind_if_non_matched, ["a", "b"], &["--bind", "enter:if-non-matched(backward-delete-char)", "-q", "ab"], {
    @snap;
    @key Enter;
    @snap;
    @key Enter;
    @char 'c';
    @snap;
});

// Test append-and-select action: appends query to item and selects it
insta_test!(insta_bind_append_and_select, ["a", "", "b", "c"], &["-m", "--bind", "ctrl-f:append-and-select"], {
    @snap;
    @type "xyz";
    @snap;
    @ctrl 'f';
    @snap;
});

// Test first/last actions: jump to first and last items
insta_test!(insta_bind_first_last, ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"], &["--bind", "ctrl-f:first,ctrl-l:last"], {
    @snap;
    @ctrl 'f';
    @snap;
    @ctrl 'l';
    @snap;
    @ctrl 'f';
    @snap;
});

// Test top alias: top is an alias for first
insta_test!(insta_bind_top_alias, ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"], &["--bind", "ctrl-t:top,ctrl-l:last"], {
    @snap;
    @ctrl 'l';
    @snap;
    @ctrl 't';
    @snap;
});

// Test change event: triggers on query change
insta_test!(insta_bind_change, ["1", "12", "13", "14", "15", "16", "17", "18", "19", "10"], &["--bind", "change:first"], {
    @snap;
    @key Up;
    @key Up;
    @snap;
    @char '1';
    @snap;
});
