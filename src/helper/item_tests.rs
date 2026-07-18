use super::*;

#[test]
fn test_strip_ansi() {
    // Test basic ANSI color codes
    // "\x1b[31mred\x1b[0m" has chars at positions: 0=ESC, 1=[, 2=3, 3=1, 4=m, 5=r, 6=e, 7=d, 8=ESC, 9=[, 10=0, 11=m
    let (text, mapping) = strip_ansi("\x1b[31mred\x1b[0m");
    assert_eq!(text, "red");
    assert_eq!(mapping, vec![(5, 5), (6, 6), (7, 7)]);

    let (text, mapping) = strip_ansi("\x1b[01;32mgreen\x1b[0m");
    assert_eq!(text, "green");
    assert_eq!(mapping, vec![(8, 8), (9, 9), (10, 10), (11, 11), (12, 12)]);

    let (text, mapping) = strip_ansi("\x1b[01;34mblue\x1b[0m");
    assert_eq!(text, "blue");
    assert_eq!(mapping, vec![(8, 8), (9, 9), (10, 10), (11, 11)]);

    // Test text without ANSI codes
    let (text, mapping) = strip_ansi("plain text");
    assert_eq!(text, "plain text");
    assert_eq!(
        mapping,
        vec![
            (0, 0),
            (1, 1),
            (2, 2),
            (3, 3),
            (4, 4),
            (5, 5),
            (6, 6),
            (7, 7),
            (8, 8),
            (9, 9)
        ]
    );

    // Test multiple ANSI sequences
    let (text, mapping) = strip_ansi("\x1b[31mred\x1b[0m and \x1b[32mgreen\x1b[0m");
    assert_eq!(text, "red and green");
    assert_eq!(
        mapping,
        vec![
            (5, 5),
            (6, 6),
            (7, 7),
            (12, 12),
            (13, 13),
            (14, 14),
            (15, 15),
            (16, 16),
            (22, 22),
            (23, 23),
            (24, 24),
            (25, 25),
            (26, 26)
        ]
    );

    // Test ANSI codes in the middle of text
    let (text, mapping) = strip_ansi("be\x1b[01;34mf\x1b[0more");
    assert_eq!(text, "before");
    assert_eq!(mapping, vec![(0, 0), (1, 1), (10, 10), (15, 15), (16, 16), (17, 17)]);

    // Test real ls --color output
    let (text, mapping) = strip_ansi("\x1b[01;32mbench.sh\x1b[0m");
    assert_eq!(text, "bench.sh");
    assert_eq!(
        mapping,
        vec![
            (8, 8),
            (9, 9),
            (10, 10),
            (11, 11),
            (12, 12),
            (13, 13),
            (14, 14),
            (15, 15)
        ]
    );

    let (text, mapping) = strip_ansi("\x1b[01;34mbin\x1b[0m");
    assert_eq!(text, "bin");
    assert_eq!(mapping, vec![(8, 8), (9, 9), (10, 10)]);

    // Test with multi-byte UTF-8 characters to verify byte vs char position difference
    // "😀" is 4 bytes but 1 char - when followed by ANSI codes, byte and char positions diverge
    let (text, mapping) = strip_ansi("😀\x1b[32mtext\x1b[0m");
    assert_eq!(text, "😀text");
    // Original: "😀\x1b[32mtext\x1b[0m"
    // byte positions: 😀=0-3, \x1b=4, [=5, 3=6, 2=7, m=8, t=9, e=10, x=11, t=12, \x1b=13, [=14, 0=15, m=16
    // char positions: 😀=0, \x1b=1, [=2, 3=3, 2=4, m=5, t=6, e=7, x=8, t=9, \x1b=10, [=11, 0=12, m=13
    // After stripping: "😀text"
    // stripped[0]='😀' -> (byte=0, char=0)
    // stripped[1]='t' -> (byte=9, char=6) <- Here byte and char positions differ!
    assert_eq!(mapping, vec![(0, 0), (9, 6), (10, 7), (11, 8), (12, 9)]);
}

#[test]
fn test_strip_ansi_osc_sequence_bel_terminated() {
    // OSC sequence (ESC ]) terminated by BEL (\x07) is fully stripped.
    let (text, _) = strip_ansi("\x1b]0;title\x07visible");
    assert_eq!(text, "visible");
}

#[test]
fn test_strip_ansi_osc_sequence_st_terminated() {
    // OSC sequence terminated by the ST string terminator (ESC \) is stripped.
    let (text, _) = strip_ansi("\x1b]8;;http://example.com\x1b\\link");
    assert_eq!(text, "link");
}

#[test]
fn test_strip_ansi_two_char_escape_sequences() {
    // ESC ( / ESC ) charset-selection sequences consume the two-char prefix.
    let (text, _) = strip_ansi("\x1b(Bplain");
    assert_eq!(text, "plain");
    let (text, _) = strip_ansi("\x1b)0plain");
    assert_eq!(text, "plain");
}

#[test]
fn test_strip_ansi_unknown_escape_consumes_one_char() {
    // An unrecognised escape (ESC followed by an unknown byte) drops that byte.
    let (text, _) = strip_ansi("\x1bXdata");
    assert_eq!(text, "data");
}

#[test]
fn test_strip_ansi_trailing_lone_escape() {
    // A trailing ESC with nothing after it is dropped without panicking.
    let (text, _) = strip_ansi("abc\x1b");
    assert_eq!(text, "abc");
}

#[test]
fn test_ansi_matching_and_display() {
    use crate::{DisplayContext, Matches, SkimItem};
    use ratatui::style::{Color, Style};
    use regex::Regex;

    // Create an item with ANSI codes
    let input = "\x1b[32mgreen\x1b[0m text";
    let delimiter = Regex::new(r"\s+").unwrap();
    let item = DefaultSkimItem::new(
        input,
        true, // ansi_enabled
        &[],
        &[],
        &delimiter,
    );

    // text() should return stripped text for matching
    assert_eq!(item.text(), "green text");

    // Verify we have ANSI info
    assert!(item.ansi_info().is_some());

    // Create a match context as if we matched "text" (positions 6-10 in stripped string)
    let context = DisplayContext {
        score: 100,
        matches: Matches::CharRange(6, 10),
        container_width: 80,
        base_style: Style::default(),
        matched_style: Style::default().fg(Color::Yellow),
    };

    // display() should map the match positions back to the original ANSI text
    let line = item.display(context);

    // The line should have the original ANSI codes intact
    // We can't easily verify the exact ANSI codes in the output, but we can check
    // that it's not empty and has multiple spans (original text + highlighted match)
    assert!(!line.spans.is_empty());
}

#[test]
fn test_ansi_char_indices_mapping() {
    use crate::{DisplayContext, Matches, SkimItem};
    use ratatui::style::{Color, Style};
    use regex::Regex;

    // Create an item with ANSI codes: "😀\x1b[32mtext\x1b[0m"
    let input = "😀\x1b[32mtext\x1b[0m";
    let delimiter = Regex::new(r"\s+").unwrap();
    let item = DefaultSkimItem::new(
        input,
        true, // ansi_enabled
        &[],
        &[],
        &delimiter,
    );

    // text() should return "😀text"
    assert_eq!(item.text(), "😀text");

    // Match indices 1,2 in stripped text (the 't' and 'e')
    let context = DisplayContext {
        score: 100,
        matches: Matches::CharIndices(vec![1, 2]),
        container_width: 80,
        base_style: Style::default(),
        matched_style: Style::default().fg(Color::Yellow),
    };

    // display() should map these to positions 6,7 in original text
    let line = item.display(context);
    assert!(!line.spans.is_empty());
}

#[test]
fn test_text_returns_stripped() {
    use crate::SkimItem;
    use regex::Regex;

    let delimiter = Regex::new(r"\s+").unwrap();

    // Test with ANSI enabled
    let item_ansi = DefaultSkimItem::new(
        "\x1b[31mred\x1b[0m",
        true, // ansi_enabled
        &[],
        &[],
        &delimiter,
    );
    assert_eq!(
        item_ansi.text(),
        "red",
        "text() should return stripped text when ANSI is enabled"
    );

    // Test with ANSI disabled
    let item_no_ansi = DefaultSkimItem::new(
        "\x1b[31mred\x1b[0m",
        false, // ansi_enabled
        &[],
        &[],
        &delimiter,
    );
    assert_eq!(
        item_no_ansi.text(),
        "?[31mred?[0m",
        "text() should return text with ? when ANSI is disabled"
    );
}

#[test]
fn test_highlighting_applied() {
    use crate::{DisplayContext, Matches, SkimItem};
    use ratatui::style::{Color, Style};
    use regex::Regex;

    let delimiter = Regex::new(r"\s+").unwrap();

    // Create item with ANSI codes: "\x1b[32mgreen\x1b[0m"
    let item = DefaultSkimItem::new(
        "\x1b[32mgreen\x1b[0m",
        true, // ansi_enabled
        &[],
        &[],
        &delimiter,
    );

    // Create display context with yellow background highlight for character 0 (the 'g')
    let context = DisplayContext {
        score: 100,
        matches: Matches::CharIndices(vec![0]),
        container_width: 80,
        base_style: Style::default(),
        matched_style: Style::default().bg(Color::Yellow),
    };

    let line = item.display(context);

    // The line should have spans with highlighting
    // At least one span should have the yellow background
    let has_highlight = line.spans.iter().any(|span| span.style.bg == Some(Color::Yellow));
    assert!(has_highlight, "Highlighted character should have yellow background");

    // The green foreground from ANSI should be preserved in at least one span
    let has_green_fg = line.spans.iter().any(|span| span.style.fg == Some(Color::Green));
    assert!(has_green_fg, "ANSI green foreground should be preserved");
}

#[test]
fn test_char_range_highlighting() {
    use crate::{DisplayContext, Matches, SkimItem};
    use ratatui::style::{Color, Style};
    use regex::Regex;

    let delimiter = Regex::new(r"\s+").unwrap();

    // Create item with ANSI codes: "\x1b[32mgreen\x1b[0m"
    let item = DefaultSkimItem::new(
        "\x1b[32mgreen\x1b[0m",
        true, // ansi_enabled
        &[],
        &[],
        &delimiter,
    );

    // Create display context with yellow background highlight for characters 1-3 ('re')
    let context = DisplayContext {
        score: 100,
        matches: Matches::CharRange(1, 3),
        container_width: 80,
        base_style: Style::default(),
        matched_style: Style::default().bg(Color::Yellow),
    };

    let line = item.display(context);

    // Should have multiple spans: before, highlighted, after
    assert!(line.spans.len() >= 2, "Should have multiple spans for highlighting");

    // At least one span should have the yellow background (the highlighted portion)
    let has_highlight = line.spans.iter().any(|span| span.style.bg == Some(Color::Yellow));
    assert!(has_highlight, "Highlighted characters should have yellow background");

    // The green foreground from ANSI should be preserved
    let has_green_fg = line.spans.iter().any(|span| span.style.fg == Some(Color::Green));
    assert!(has_green_fg, "ANSI green foreground should be preserved");
}

#[test]
fn test_byte_range_highlighting() {
    use crate::{DisplayContext, Matches, SkimItem};
    use ratatui::style::{Color, Style};
    use regex::Regex;

    let delimiter = Regex::new(r"\s+").unwrap();

    // Create item with ANSI codes: "\x1b[32mgreen\x1b[0m"
    let item = DefaultSkimItem::new(
        "\x1b[32mgreen\x1b[0m",
        true, // ansi_enabled
        &[],
        &[],
        &delimiter,
    );

    // Create display context with yellow background highlight for bytes 1-3 ('re' in stripped text)
    let context = DisplayContext {
        score: 100,
        matches: Matches::ByteRange(1, 3),
        container_width: 80,
        base_style: Style::default(),
        matched_style: Style::default().bg(Color::Yellow),
    };

    let line = item.display(context);

    // Should have multiple spans for highlighting
    assert!(!line.spans.is_empty(), "Should have spans");

    // At least one span should have the yellow background (the highlighted portion)
    let has_highlight = line.spans.iter().any(|span| span.style.bg == Some(Color::Yellow));
    assert!(has_highlight, "Highlighted bytes should have yellow background");

    // The green foreground from ANSI should be preserved
    let has_green_fg = line.spans.iter().any(|span| span.style.fg == Some(Color::Green));
    assert!(has_green_fg, "ANSI green foreground should be preserved");
}

#[test]
fn test_matching_with_ansi_basic() {
    use crate::SkimItem;
    use regex::Regex;

    let delimiter = Regex::new(r"\s+").unwrap();

    // Create item with ANSI codes: "\x1b[32mgreen_text\x1b[0m"
    let item = DefaultSkimItem::new(
        "\x1b[32mgreen_text\x1b[0m",
        true, // ansi_enabled
        &[],
        &[], // no matching fields restriction
        &delimiter,
    );

    // text() should return stripped text "green_text"
    assert_eq!(item.text(), "green_text");

    // With no matching_fields, get_matching_ranges should return None (match whole text)
    assert!(item.get_matching_ranges().is_none());

    // Verify the stripped_text and ansi_info are populated correctly
    assert!(item.stripped_text().is_some());
    assert!(item.ansi_info().is_some());
    assert_eq!(item.stripped_text().unwrap(), "green_text");
}

#[test]
fn test_null_delimiter_with_matching_fields() {
    use crate::SkimItem;
    use crate::field::FieldRange;
    use regex::Regex;

    // Test with null byte delimiter and matching_fields
    let delimiter = Regex::new("\x00").unwrap();
    let text = "a\x00b\x00c";

    // Create item with matching field 2
    let item = DefaultSkimItem::new(
        text,
        false,                    // no ansi
        &[],                      // no transform fields
        &[FieldRange::Single(2)], // match field 2
        &delimiter,
    );

    // text() should return text with null bytes stripped for display
    assert_eq!(item.text(), "abc");

    // get_matching_ranges should return the range for field 2 in the stripped text
    let ranges = item.get_matching_ranges().expect("Should have matching ranges");
    assert_eq!(ranges.len(), 1, "Should have one matching range");

    // Field 2 is "b" which is at position 1 in the stripped text "abc"
    assert_eq!(ranges[0], (1, 2), "Field 2 should be at position 1-2 in stripped text");

    // Verify the substring matches what we expect
    let stripped_text = item.text();
    let field_text = &stripped_text[ranges[0].0..ranges[0].1];
    assert_eq!(field_text, "b", "Field text should be 'b'");
}

#[test]
fn test_default_skim_item_from_string_and_display_text() {
    let item = DefaultSkimItem::from("plain text".to_string());
    assert_eq!(item.get_display_text(), "plain text");
    assert_eq!(item.text(), "plain text");
}

#[test]
fn test_transform_fields_with_ansi_enabled() {
    use regex::Regex;
    // Both a transform field and ANSI enabled exercises the (true, true) arm.
    let delimiter = Regex::new(r"\s+").unwrap();
    let item = DefaultSkimItem::new(
        "\x1b[32mone\x1b[0m two three",
        true,
        &[FieldRange::Single(2)],
        &[],
        &delimiter,
    );
    // The display text is the second field.
    assert!(item.text().contains("two"));
}

#[test]
fn test_matching_fields_with_ansi_uses_stripped_text() {
    use regex::Regex;
    // ANSI enabled with matching fields makes range computation use stripped text.
    let delimiter = Regex::new(r"\s+").unwrap();
    let item = DefaultSkimItem::new(
        "\x1b[32mone\x1b[0m two",
        true,
        &[],
        &[FieldRange::Single(2)],
        &delimiter,
    );
    assert_eq!(item.text(), "one two");
    let ranges = item.get_matching_ranges().expect("matching ranges present");
    assert_eq!(ranges.len(), 1);
}

#[test]
fn test_display_ansi_item_with_no_matches() {
    use crate::{DisplayContext, Matches, SkimItem};
    use ratatui::style::Style;
    use regex::Regex;

    // An ANSI-enabled item displayed with `Matches::None` keeps the parsed
    // ANSI spans unchanged (the `Matches::None` arm of the ANSI branch).
    let delimiter = Regex::new(r"\s+").unwrap();
    let item = DefaultSkimItem::new("\x1b[31mred\x1b[0m text", true, &[], &[], &delimiter);
    let context = DisplayContext {
        score: 0,
        matches: Matches::None,
        container_width: 80,
        base_style: Style::default(),
        matched_style: Style::default(),
    };
    let line = item.display(context);
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
    assert!(text.contains("red"));
    assert!(text.contains("text"));
}

#[test]
fn test_normalize_ranges_sorts_and_merges() {
    // Overlapping and touching ranges are merged; empty ranges dropped; result sorted.
    assert_eq!(normalize_ranges(&[(5, 8), (0, 3)]), vec![(0, 3), (5, 8)]);
    assert_eq!(normalize_ranges(&[(0, 4), (2, 6)]), vec![(0, 6)]);
    assert_eq!(normalize_ranges(&[(0, 3), (3, 6)]), vec![(0, 6)]);
    assert_eq!(normalize_ranges(&[(2, 2), (0, 1)]), vec![(0, 1)]);
    assert!(normalize_ranges(&[]).is_empty());
}

#[test]
fn test_project_visible_text_removes_hidden_ranges() {
    // Hide bytes 6..10 ("RED ") from "apple RED 001".
    let (visible, map) = project_visible_text("apple RED 001", &[(6, 10)]);
    assert_eq!(visible, "apple 001");
    // Chars 0..6 ("apple ") map to themselves, 6..10 ("RED ") are hidden,
    // and the trailing "001" is shifted left by 4 positions.
    assert_eq!(map[0], Some(0)); // 'a'
    assert_eq!(map[5], Some(5)); // ' '
    assert_eq!(map[6], None); // 'R'
    assert_eq!(map[9], None); // ' '
    assert_eq!(map[10], Some(6)); // '0'
    assert_eq!(map[12], Some(8)); // '1'
}

#[test]
fn test_project_match_indices_drops_hidden_and_remaps() {
    use crate::Matches;
    let (_visible, map) = project_visible_text("apple RED 001", &[(6, 10)]);
    // A match spanning both a visible char ('e' at 4) and hidden chars (7,8) keeps
    // only the visible one, remapped into visible coordinates (unchanged here).
    let indices = project_match_indices("apple RED 001", &Matches::CharIndices(vec![4, 7, 8]), &map);
    assert_eq!(indices, vec![4]);
    // A byte range covering "001" (bytes 10..13) maps to visible chars 6,7,8.
    let indices = project_match_indices("apple RED 001", &Matches::ByteRange(10, 13), &map);
    assert_eq!(indices, vec![6, 7, 8]);
    // A match entirely inside the hidden field yields nothing.
    let indices = project_match_indices("apple RED 001", &Matches::CharRange(6, 9), &map);
    assert!(indices.is_empty());
}

#[test]
fn test_hidden_ranges_keep_text_searchable() {
    use crate::field::FieldRange;
    use regex::Regex;
    let delimiter = Regex::new(r"\s+").unwrap();
    // Hide field 2 ("RED") but keep it searchable.
    let item = DefaultSkimItem::new("apple RED 001", false, &[], &[], &delimiter)
        .hidden_fields(&[FieldRange::Single(2)], &delimiter);
    // text() (used for matching) retains the hidden field, so it stays searchable.
    assert_eq!(item.text(), "apple RED 001");
    // hidden_ranges exposes the field's byte range (including its trailing delimiter).
    assert_eq!(item.hidden_ranges(), Some(&[(6, 10)][..]));
}

#[test]
fn test_hidden_field_removed_from_display() {
    use crate::field::FieldRange;
    use crate::{DisplayContext, Matches, SkimItem};
    use ratatui::style::Style;
    use regex::Regex;
    let delimiter = Regex::new(r"\s+").unwrap();
    let item = DefaultSkimItem::new("apple RED 001", false, &[], &[], &delimiter)
        .hidden_fields(&[FieldRange::Single(2)], &delimiter);
    let context = DisplayContext {
        score: 0,
        matches: Matches::None,
        container_width: 80,
        base_style: Style::default(),
        matched_style: Style::default(),
    };
    let line = item.display(context);
    let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    // The hidden field is gone from what is displayed, but the rest remains.
    assert_eq!(rendered, "apple 001");
    assert!(!rendered.contains("RED"));
}

#[test]
fn test_hidden_field_match_not_highlighted() {
    use crate::field::FieldRange;
    use crate::{DisplayContext, Matches, SkimItem};
    use ratatui::style::{Color, Style};
    use regex::Regex;
    let delimiter = Regex::new(r"\s+").unwrap();
    let item = DefaultSkimItem::new("apple RED 001", false, &[], &[], &delimiter)
        .hidden_fields(&[FieldRange::Single(2)], &delimiter);
    // Simulate a match on the hidden "RED" (chars 6,7,8 in the full text).
    let context = DisplayContext {
        score: 0,
        matches: Matches::CharIndices(vec![6, 7, 8]),
        container_width: 80,
        base_style: Style::default(),
        matched_style: Style::default().bg(Color::Yellow),
    };
    let line = item.display(context);
    let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(rendered, "apple 001");
    // No span carries the highlight background, since the matched chars are hidden.
    assert!(line.spans.iter().all(|span| span.style.bg != Some(Color::Yellow)));
}

#[test]
fn test_hidden_field_preserves_ansi_colors() {
    use crate::field::FieldRange;
    use crate::{DisplayContext, Matches, SkimItem};
    use ratatui::style::{Color, Style};
    use regex::Regex;
    let delimiter = Regex::new(r"\s+").unwrap();
    // Two colored, space-separated fields: green "one" and red "two".
    let item = DefaultSkimItem::new(
        "\x1b[32mone\x1b[0m \x1b[31mtwo\x1b[0m",
        true, // ansi_enabled
        &[],
        &[],
        &delimiter,
    )
    .hidden_fields(&[FieldRange::Single(1)], &delimiter); // hide the first (green) field
    // The hidden field is still part of the matchable text.
    assert_eq!(item.text(), "one two");
    let context = DisplayContext {
        score: 0,
        matches: Matches::None,
        container_width: 80,
        base_style: Style::default(),
        matched_style: Style::default(),
    };
    let line = item.display(context);
    let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    // "one " (field 1 plus its trailing delimiter) is gone; "two" remains.
    assert_eq!(rendered, "two");
    // The surviving field keeps its ANSI red foreground; the hidden green is gone.
    assert!(
        line.spans.iter().any(|span| span.style.fg == Some(Color::Red)),
        "surviving field should keep its ANSI red foreground"
    );
    assert!(
        line.spans.iter().all(|span| span.style.fg != Some(Color::Green)),
        "hidden field's ANSI green foreground should not appear"
    );
}

#[test]
fn test_hidden_field_ansi_highlight_remapped() {
    use crate::field::FieldRange;
    use crate::{DisplayContext, Matches, SkimItem};
    use ratatui::style::{Color, Style};
    use regex::Regex;
    let delimiter = Regex::new(r"\s+").unwrap();
    let item = DefaultSkimItem::new("\x1b[32mone\x1b[0m \x1b[31mtwo\x1b[0m", true, &[], &[], &delimiter)
        .hidden_fields(&[FieldRange::Single(1)], &delimiter); // hide green "one"
    // Match "two" — chars 4,5,6 in the full stripped text "one two".
    let context = DisplayContext {
        score: 0,
        matches: Matches::CharIndices(vec![4, 5, 6]),
        container_width: 80,
        base_style: Style::default(),
        matched_style: Style::default().bg(Color::Yellow),
    };
    let line = item.display(context);
    let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(rendered, "two");
    // The match on the visible field is highlighted (remapped to visible coords 0..3)
    // while its ANSI red foreground is preserved alongside the highlight background.
    assert!(line.spans.iter().any(|span| span.style.bg == Some(Color::Yellow)));
    assert!(line.spans.iter().any(|span| span.style.fg == Some(Color::Red)));
}
