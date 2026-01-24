#[allow(dead_code)]
#[macro_use]
mod common;

// Test if-non-matched action: deletes character when no match
insta_test!(bind_if_non_matched, ["a", "b"], &["--bind", "enter:if-non-matched(backward-delete-char)", "-q", "ab"], {
    @snap;
    @key Enter;
    @snap;
    @key Enter;
    @char 'c';
    @snap;
});

// Test append-and-select action: appends query to item and selects it
insta_test!(bind_append_and_select, ["a", "", "b", "c"], &["-m", "--bind", "ctrl-f:append-and-select"], {
    @snap;
    @type "xyz";
    @snap;
    @ctrl 'f';
    @snap;
});

// Test first/last actions: jump to first and last items
insta_test!(bind_first_last, ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"], &["--bind", "ctrl-f:first,ctrl-l:last"], {
    @snap;
    @ctrl 'f';
    @snap;
    @ctrl 'l';
    @snap;
    @ctrl 'f';
    @snap;
});

// Test top alias: top is an alias for first
insta_test!(bind_top_alias, ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"], &["--bind", "ctrl-t:top,ctrl-l:last"], {
    @snap;
    @ctrl 'l';
    @snap;
    @ctrl 't';
    @snap;
});

// Test change event: triggers on query change
insta_test!(bind_change, ["1", "12", "13", "14", "15", "16", "17", "18", "19", "10"], &["--bind", "change:first"], {
    @snap;
    @key Up;
    @key Up;
    @snap;
    @char '1';
    @snap;
});

insta_test!(bind_set_query_basic, ["a", "b", "c"], &["--bind", "ctrl-a:set-query(foo)"], {
    @snap;
    @ctrl 'a';
    @snap;
});

insta_test!(bind_set_query_expand, ["a", "b", "c"], &["--bind", "ctrl-a:set-query({})"], {
    @snap;
    @ctrl 'a';
    @snap;
});

insta_test!(bind_set_query_fields, ["a.1", "b.2", "c.3"], &["--bind", "ctrl-a:set-query({1})", "-d", "\\."], {
    @snap;
    @ctrl 'a';
    @snap;
});

insta_test!(bind_set_query_to_itself, ["a", "b", "c"], &["--bind", "ctrl-a:set-query({q})"], {
    @snap;
    @ctrl 'a';
    @snap;
    @char 'a';
    @snap;
    @ctrl 'a';
    @snap;
});

insta_test!(bind_toggle_interactive, @interactive, &["--bind", "ctrl-a:toggle-interactive", "-i"], {
    @snap;
    @ctrl 'a';
    @snap;
});

insta_test!(bind_toggle_interactive_queries, @interactive, &["--bind", "ctrl-a:toggle-interactive", "-i", "--query", "normal", "--cmd-query", "interactive"], {
    @snap;
    @ctrl 'a';
    @snap;
    @key Left;
    @char '|';
    @snap;
    @ctrl 'a';
    @snap;
    @ctrl 'a';
    @snap;
});
