//! Insta snapshot tests for `--multiline` mode and the flags it interacts with.
//!
//! Run once with `INSTA_UPDATE=always cargo nextest run multiline` (or use
//! `cargo insta review` afterwards) to generate / review the initial snapshots.
//!
//! Existing basic multiline tests (navigation, scrolling, custom `|` separator)
//! live in `tests/options.rs`; this file focuses on combinatorial coverage.

#[allow(dead_code)]
#[macro_use]
mod common;

// ============================================================================
// Section 1: Core multiline display
// ============================================================================

// An item that spans three display lines (two separators).
insta_test!(multiline_three_lines, ["a\\nb\\nc"], &["--multiline"], {
    @snap;
});

// An item whose middle segment is empty, producing a blank display line.
insta_test!(multiline_empty_segment, ["before\\n\\nafter"], &["--multiline"], {
    @snap;
});

// A single multiline item with nothing else in the list.
insta_test!(multiline_single_item, ["line1\\nline2"], &["--multiline"], {
    @snap;
});

// Mix of single-line and multiline items; navigate through them.
insta_test!(multiline_mixed_navigation, ["single", "first\\nsecond", "only"], &["--multiline"], {
    @snap;
    @key Up;
    @snap;
    @key Up;
    @snap;
});

// ============================================================================
// Section 2: Custom separator
// ============================================================================

// Comma used as the multiline separator (pipe is already covered in options.rs).
insta_test!(multiline_custom_sep_comma, ["x", "p,q,r", "y"], &["--multiline", ","], {
    @snap;
});

// Colon used as the multiline separator.
insta_test!(multiline_custom_sep_colon, ["a", "key:value:extra", "z"], &["--multiline", ":"], {
    @snap;
});

// ============================================================================
// Section 3: Tabstop interactions
// ============================================================================

// Tab character in a multiline item with the default tabstop (8).
// "a" is at column 0; the tab expands to 7 spaces so "X" lands at column 8.
// Line 2 has the same structure: "b" + tab + "Y" → "b       Y".
insta_test!(multiline_tabstop_default, ["a\tX\\nb\tY"], &["--multiline"], {
    @snap;
});

// Tab characters with tabstop=4.
// Item 1: "a" at col 0 → tab to col 4 (3 spaces) → "a   X" / "b   Y".
// Item 2: "xy" ends at col 2 → tab to col 4 (2 spaces) → "xy  Z" / "w   V".
insta_test!(multiline_tabstop_4, ["a\tX\\nb\tY", "xy\tZ\\nw\tV"], &["--multiline", "--tabstop", "4"], {
    @snap;
});

// Tab characters with tabstop=1: every column is a stop, each tab becomes a single space.
// Item 1: "a" + tab + "X" → "a X" / "b" + tab + "Y" → "b Y".
// Item 2: "xy" + tab + "Z" → "xy Z" / "w" + tab + "V" → "w V".
insta_test!(multiline_tabstop_1, ["a\tX\\nb\tY", "xy\tZ\\nw\tV"], &["--multiline", "--tabstop", "1"], {
    @snap;
});

// Multiline separator appears after a tab-expanded segment, combining both features.
insta_test!(multiline_tabstop_midline, ["col1\tcol2\\ncol3\tcol4"], &["--multiline", "--tabstop", "6"], {
    @snap;
});

// ============================================================================
// Section 4: Layout interactions
// ============================================================================

// Reverse layout: prompt at top, items displayed top-to-bottom.
insta_test!(multiline_layout_reverse, ["a", "b1\\nb2", "c"], &["--multiline", "--layout", "reverse"], {
    @snap;
    @key Down;
    @snap;
});

// Reverse-list layout: prompt at bottom but items listed top-to-bottom.
insta_test!(multiline_layout_reverse_list, ["a", "b1\\nb2", "c"], &["--multiline", "--layout", "reverse-list"], {
    @snap;
    @key Up;
    @snap;
});

// Border around the entire UI with multiline items.
insta_test!(multiline_border, ["a", "b1\\nb2", "c"], &["--multiline", "--border"], {
    @snap;
});

// Reverse layout combined with a border.
insta_test!(multiline_layout_reverse_border, ["a", "b1\\nb2", "c"], &["--multiline", "--layout", "reverse", "--border"], {
    @snap;
});

// ============================================================================
// Section 5: Ordering
// ============================================================================

// --tac reverses the input order; multiline items should still display correctly.
insta_test!(multiline_tac, ["a", "b1\\nb2", "c"], &["--multiline", "--tac"], {
    @snap;
});

// --no-sort preserves insertion order after filtering; check the filtered view.
insta_test!(multiline_no_sort, ["c1\\nc2", "a1\\na2", "b1\\nb2"], &["--multiline", "--no-sort"], {
    @snap;
    @char 'a';
    @snap;
});

// ============================================================================
// Section 6: Multi-selection
// ============================================================================

// Toggle multi-selection on multiline items; both display rows belong to the same item.
insta_test!(multiline_multi, ["a", "b1\\nb2", "c"], &["--multiline", "--multi"], {
    @snap;
    @shift Tab;
    @snap;
    @key Up;
    @snap;
    @shift Tab;
    @snap;
});

// Pre-select the first N items (some of which are multiline).
insta_test!(multiline_pre_select_n, ["a1\\na2", "b1\\nb2", "c"], &["--multiline", "-m", "--pre-select-n", "2"], {
    @snap;
});

// Pre-select items matching a pattern that happens to target a multiline item.
insta_test!(multiline_pre_select_pat, ["a", "match1\\nmatch2", "b"], &["--multiline", "-m", "--pre-select-pat", "match"], {
    @snap;
});

// ============================================================================
// Section 7: Header interactions
// ============================================================================

// Static --header text above a list of multiline items.
insta_test!(multiline_with_header, ["a", "b1\\nb2", "c"], &["--multiline", "--header", "my header"], {
    @snap;
});

// --header-lines 1: the first item itself is multiline and becomes the header.
insta_test!(multiline_header_lines, ["h1\\nh2", "a1\\na2", "b1\\nb2"], &["--multiline", "--header-lines", "1"], {
    @snap;
});

// Multiline header text combined with multiline items.
insta_test!(multiline_header_multiline_text, ["a", "b1\\nb2", "c"], &["--multiline", "--header", "line1\nline2"], {
    @snap;
});

// ============================================================================
// Section 8: Searching / filtering
// ============================================================================

// Query term that only appears in the second display segment; the item must still match.
insta_test!(multiline_search_second_line, ["first\\nsecond", "other"], &["--multiline", "-q", "second"], {
    @snap;
});

// Query matches neither line of any item: the result list must be empty.
insta_test!(multiline_no_match, ["a\\nb", "c\\nd"], &["--multiline", "-q", "xyz"], {
    @snap;
});

// Navigate after a partial match to make sure the cursor lands on the right item.
insta_test!(multiline_search_and_navigate, ["aa\\nbb", "cc\\ndd", "aa\\ncc"], &["--multiline", "-q", "aa"], {
    @snap;
    @key Up;
    @snap;
});

// ============================================================================
// Section 9: Wrap interaction
// ============================================================================

// A multiline item whose second segment is wider than the terminal; --wrap folds it.
insta_test!(multiline_wrap, [
    "short\\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
], &["--multiline", "--wrap"], {
    @snap;
});

// Both lines of an item are long enough to wrap independently.
insta_test!(multiline_wrap_both_lines, [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\\nbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
], &["--multiline", "--wrap"], {
    @snap;
});

// ============================================================================
// Section 10: Cycle interaction
// ============================================================================

// With --cycle, pressing Down from the first item wraps to the last multiline item.
insta_test!(multiline_cycle, ["a", "b1\\nb2", "c"], &["--multiline", "--cycle"], {
    @snap;
    @key Down;
    @snap;
    @key Up;
    @snap;
});

// ============================================================================
// Section 11: read0 — null-byte-delimited input
// ============================================================================

// With --read0 the items are NUL-delimited, so the multiline separator becomes
// an actual newline (not a literal "\n").
// Input bytes:  a NUL  b1 LF b2  NUL  c NUL  →  items: ["a", "b1\nb2", "c"]
insta_test!(multiline_read0, @bytes b"a\x00b1\nb2\x00c\x00", &["--read0", "--multiline"], {
    @snap;
    @key Up;
    @snap;
});

// read0 with a multiline item that contains three actual newlines.
insta_test!(multiline_read0_three_lines, @bytes b"x\x00one\ntwo\nthree\x00y\x00", &["--read0", "--multiline"], {
    @snap;
    @key Up;
    @snap;
});

// ============================================================================
// Section 12: with-nth / field display
// ============================================================================

// --with-nth hides the leading field; the remaining display text still contains
// the multiline separator and is split for display.
insta_test!(multiline_with_nth, [
    "id1 line1\\nline2",
    "id2 line3\\nline4",
], &["--multiline", "--with-nth", "2..", "--delimiter", " "], {
    @snap;
    @key Up;
    @snap;
});

// --nth restricts matching to a specific field of multiline items.
insta_test!(multiline_nth, [
    "aaa bbb\\nccc ddd",
    "eee fff\\nggg hhh",
], &["--multiline", "--nth", "2", "--delimiter", " "], {
    @snap;
    @char 'b';
    @snap;
    @ctrl 'w';
    @snap;
    @char 'f';
    @snap;
});
