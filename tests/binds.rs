#![allow(missing_docs, clippy::pedantic)]

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

// Test start event: fires once when skim starts up, running its bound action.
insta_test!(bind_start, ["a", "b", "c"], &["--bind", "start:set-query(started)"], {
    @assert(|h: &common::insta::TestHarness| h.skim.app().input.value == "started");
    @snap;
});

// Test load event: fires once the reader has finished AND the read items have
// been rendered into the list, so a `load` binding can safely act on the
// fully-populated list (here it jumps to the last item).
insta_test!(bind_load, ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"], &["--bind", "load:last"], {
    @snap;
    @assert(|h: &common::insta::TestHarness| h.skim.app().item_list.selected().unwrap().text() == "10");
});

// Any action can be bound as if it were an event: `first:last` runs `last`
// right after `first`, so pressing the key ends on the last item.
insta_test!(bind_action_followup, ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"], &["--bind", "ctrl-a:first", "--bind", "first:last"], {
    @ctrl 'a';
    @assert(|h: &common::insta::TestHarness| h.skim.app().item_list.selected().unwrap().text() == "10");
    @snap;
});

// `act-<name>` targets the *action* even when the name is also a key: `act-up`
// binds the Up action (not the up key). Bound to `last`, running the Up action
// appends a jump to the last item.
insta_test!(bind_act_prefix, ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"], &["--bind", "act-up:last"], {
    @action Up(1);
    @assert(|h: &common::insta::TestHarness| h.skim.app().item_list.selected().unwrap().text() == "10");
    @snap;
});

// `suppress` cancels the triggering action's own effect. Here the First action
// is remapped: instead of jumping to the first item it only sets the header, so
// the cursor stays put (on the last item) and the header is updated.
insta_test!(bind_suppress, ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"], &["--bind", "act-first:suppress+set-header(suppressed)"], {
    @action Last;
    @action First;
    @assert(|h: &common::insta::TestHarness| h.skim.app().header.header == "suppressed");
    @assert(|h: &common::insta::TestHarness| h.skim.app().item_list.selected().unwrap().text() == "10");
    @snap;
});

// Test result event: fires when filtering completes and the list is ready.
insta_test!(bind_result, ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"], &["--bind", "result:last"], {
    @snap;
    @assert(|h: &common::insta::TestHarness| h.skim.app().item_list.selected().unwrap().text() == "10");
});

// Test focus event: fires when the focused item changes (here, initial focus).
insta_test!(bind_focus, ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"], &["--bind", "focus:set-header(focused)"], {
    @snap;
    @assert(|h: &common::insta::TestHarness| h.skim.app().header.header == "focused");
});

// Test zero event: fires when a completed search has no matches.
insta_test!(bind_zero, ["a", "b", "c"], &["--bind", "zero:set-header(none)"], {
    @char 'z';
    @snap;
    @assert(|h: &common::insta::TestHarness| h.skim.app().header.header == "none");
});

// Test one event: fires when a completed search has exactly one match.
insta_test!(bind_one, ["apple", "banana", "cherry"], &["--bind", "one:set-header(single)"], {
    @type "app";
    @snap;
    @assert(|h: &common::insta::TestHarness| h.skim.app().header.header == "single");
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

insta_test!(bind_toggle_interactive, @interactive, &["--bind", "ctrl-a:toggle-interactive", "-i", "--cmd", "true"], {
    @snap;
    @ctrl 'a';
    @snap;
});

insta_test!(bind_toggle_interactive_queries, @interactive, &["--bind", "ctrl-a:toggle-interactive", "-i", "--cmd", "true", "--query", "normal", "--cmd-query", "interactive"], {
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

insta_test!(bind_set_preview_cmd, ["a", "b", "c"], &["--preview", "echo initial {}", "--bind", "ctrl-a:set-preview-cmd(echo new {})"], {
    @snap;
    @ctrl 'a';
    @snap;
    @key Up;
    @snap;
});

insta_test!(bind_set_header_from_empty, ["a", "b", "c"], &["--bind", "ctrl-a:set-header(foo)"], {
    @snap;
    @ctrl 'a';
    @snap;
});

insta_test!(bind_set_header_to_empty, ["a", "b", "c"], &["--bind", "ctrl-a:set-header", "--header", "foo"], {
    @snap;
    @ctrl 'a';
    @snap;
});

insta_test!(bind_set_header_change, ["a", "b", "c"], &["--bind", "ctrl-a:set-header(bar)", "--header", "foo"], {
    @snap;
    @ctrl 'a';
    @snap;
});
