use super::*;

/// The concatenated text of every span in a line.
fn line_text(line: &Line) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

/// The concatenated text of only the highlighted spans.
fn highlighted_text(line: &Line, matched: Style) -> String {
    line.spans
        .iter()
        .filter(|s| s.style == matched)
        .map(|s| s.content.as_ref())
        .collect()
}

fn ctx(matches: Matches) -> DisplayContext {
    let matched_style = Style::default().fg(ratatui::style::Color::Red);
    DisplayContext {
        score: 0,
        matches,
        container_width: 80,
        base_style: Style::default(),
        matched_style,
    }
}

#[test]
fn to_line_char_indices_highlights_individual_chars() {
    let context = ctx(Matches::CharIndices(vec![0, 2]));
    let matched = context.base_style.patch(context.matched_style);
    let line = context.clone().to_line(Cow::Borrowed("abcd"));
    assert_eq!(line_text(&line), "abcd");
    assert_eq!(highlighted_text(&line, matched), "ac");
}

#[test]
fn to_line_char_range_highlights_span() {
    let context = ctx(Matches::CharRange(1, 3));
    let matched = context.base_style.patch(context.matched_style);
    let line = context.clone().to_line(Cow::Borrowed("abcd"));
    assert_eq!(line_text(&line), "abcd");
    assert_eq!(highlighted_text(&line, matched), "bc");
}

#[test]
fn to_line_byte_range_highlights_span() {
    let context = ctx(Matches::ByteRange(1, 3));
    let matched = context.base_style.patch(context.matched_style);
    let line = context.clone().to_line(Cow::Borrowed("abcd"));
    assert_eq!(line_text(&line), "abcd");
    assert_eq!(highlighted_text(&line, matched), "bc");
}

#[test]
fn to_line_none_has_no_highlight() {
    let context = ctx(Matches::None);
    let line = context.to_line(Cow::Borrowed("abcd"));
    assert_eq!(line_text(&line), "abcd");
    assert_eq!(line.spans.len(), 1);
}

#[test]
fn typos_from_usize() {
    assert_eq!(Typos::from(0), Typos::Disabled);
    assert_eq!(Typos::from(3), Typos::Fixed(3));
}

#[test]
fn match_result_range_char_indices_variants() {
    let byte = MatchResult {
        rank: Rank::default(),
        matched_range: MatchRange::ByteRange(1, 3),
    };
    assert_eq!(byte.range_char_indices("abcd"), vec![1, 2]);

    let char_range = MatchResult {
        rank: Rank::default(),
        matched_range: MatchRange::CharRange(1, 3),
    };
    assert_eq!(char_range.range_char_indices("abcd"), vec![1, 2]);

    let chars = MatchResult {
        rank: Rank::default(),
        matched_range: MatchRange::Chars(vec![0, 3]),
    };
    assert_eq!(chars.range_char_indices("abcd"), vec![0, 3]);
}

#[test]
fn as_any_downcasts_mutably() {
    let mut value: String = "hello".to_string();
    // Immutable downcast via the blanket `AsAny` impl.
    assert!(value.as_any().downcast_ref::<String>().is_some());
    // Mutable downcast exercises `as_any_mut`.
    let s = value.as_any_mut().downcast_mut::<String>().unwrap();
    s.push_str(" world");
    assert_eq!(value, "hello world");
}
