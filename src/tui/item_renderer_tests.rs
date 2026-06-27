use std::borrow::Cow;
use std::sync::Arc;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{List, Widget};

use super::*;
use crate::item::RankBuilder;
use crate::{Rank, SkimItem};

fn renderer(theme: &ColorTheme) -> ItemRenderer<'_> {
    ItemRenderer {
        theme,
        selector_icon: ">",
        multi_select_icon: "*",
        ellipsis: "..",
        container_width: 6,
        wrap: false,
        multiline: None,
        show_score: false,
        show_index: false,
        multi_select: false,
        tabstop: 4,
        no_hscroll: false,
        keep_right: false,
        manual_hscroll: 0,
        skip_to_pattern: None,
        reverse_sub_lines: false,
        highlight_line: false,
    }
}

fn matched_item(text: &str, matched_range: Option<MatchRange>) -> MatchedItem {
    MatchedItem::new(
        Arc::new(text.to_owned()) as Arc<dyn SkimItem>,
        Rank::default(),
        matched_range,
        &RankBuilder::default(),
    )
}

fn line_text(line: &Line<'_>) -> String {
    line.spans.iter().map(|span| span.content.as_ref()).collect()
}

fn spans_text(spans: &[Span<'_>]) -> String {
    spans.iter().map(|span| span.content.as_ref()).collect()
}

fn rendered_row_text(item: ListItem<'static>, width: u16) -> String {
    let mut buf = Buffer::empty(Rect::new(0, 0, width, 1));
    List::new(vec![item]).render(buf.area, &mut buf);
    (0..width)
        .map(|x| buf.cell((x, 0)).expect("cell should be inside buffer").symbol())
        .collect::<String>()
}

struct DisplayLongerThanText;

impl SkimItem for DisplayLongerThanText {
    fn text(&self) -> Cow<'_, str> {
        Cow::Borrowed("ab")
    }

    fn display(&self, _context: DisplayContext) -> Line<'_> {
        Line::from("abcdefghi")
    }
}

#[test]
fn split_sub_lines_uses_configured_separator() {
    let theme = ColorTheme::default();
    let mut renderer = renderer(&theme);

    assert_eq!(renderer.split_sub_lines("alpha|beta"), vec!["alpha|beta"]);

    renderer.multiline = Some("|");
    assert_eq!(
        renderer.split_sub_lines("alpha|beta|gamma"),
        vec!["alpha", "beta", "gamma"]
    );
}

#[test]
fn matched_range_as_char_range_normalizes_supported_ranges() {
    assert_eq!(
        ItemRenderer::matched_range("aébc", Some(&MatchRange::ByteRange(1, 3))),
        (1, 2)
    );
    assert_eq!(
        ItemRenderer::matched_range("abcdef", Some(&MatchRange::Chars(vec![1, 3, 4]))),
        (1, 5)
    );
    assert_eq!(
        ItemRenderer::matched_range("abcdef", Some(&MatchRange::Chars(vec![]))),
        (0, 0)
    );
    assert_eq!(
        ItemRenderer::matched_range("abcdef", Some(&MatchRange::CharRange(2, 4))),
        (2, 4)
    );
    assert_eq!(ItemRenderer::matched_range("abcdef", None), (0, 0));
}

#[test]
fn display_matches_preserves_original_match_kind() {
    assert!(matches!(
        ItemRenderer::display_matches(Some(&MatchRange::ByteRange(1, 3))),
        crate::Matches::ByteRange(1, 3)
    ));
    assert!(matches!(
        ItemRenderer::display_matches(Some(&MatchRange::CharRange(2, 4))),
        crate::Matches::CharRange(2, 4)
    ));
    assert!(matches!(
        ItemRenderer::display_matches(Some(&MatchRange::Chars(vec![0, 2]))),
        crate::Matches::CharIndices(indices) if indices == vec![0, 2]
    ));
    assert!(matches!(ItemRenderer::display_matches(None), crate::Matches::None));
}

#[test]
fn prefix_spans_uses_state_for_icons_and_first_line_fields() {
    let theme = ColorTheme::default();
    let mut renderer = renderer(&theme);
    renderer.multi_select = true;
    renderer.show_score = true;
    renderer.show_index = true;

    let mut item = matched_item("alpha", None);
    item.rank.score = 42;
    item.rank.index = 7;

    let current_selected = SubLineState {
        is_current: true,
        is_selected: true,
        is_first: true,
        is_first_sub_line: true,
        needs_ellipsis: false,
    };
    let continuation = SubLineState {
        is_current: true,
        is_selected: true,
        is_first: false,
        is_first_sub_line: false,
        needs_ellipsis: false,
    };

    assert_eq!(
        spans_text(&renderer.prefix_spans(&item, &current_selected)),
        ">*[42] [7] "
    );
    assert_eq!(spans_text(&renderer.prefix_spans(&item, &continuation)), "           ");
}

#[test]
fn prefix_spans_with_highlight_line_resets_prefix_background() {
    // With --highlight-line, the current row's line-level background fills the
    // whole row; the selector/marker prefix columns must reset their bg so they
    // are not painted with the cursor/selected background.
    let theme = ColorTheme::default();
    let mut renderer = renderer(&theme);
    renderer.highlight_line = true;
    renderer.multi_select = true;

    let item = matched_item("alpha", None);
    let current = SubLineState {
        is_current: true,
        is_selected: true,
        is_first: true,
        is_first_sub_line: true,
        needs_ellipsis: false,
    };

    // The icon/marker text is unchanged; only the background style differs.
    let spans = renderer.prefix_spans(&item, &current);
    assert_eq!(spans_text(&spans), ">*");
    // The selector prefix uses the cursor style with its background reset.
    assert_eq!(spans[0].style.bg, Some(ratatui::style::Color::Reset));
}

#[test]
fn trim_with_ellipsis_reserves_space_for_marker() {
    let theme = ColorTheme::default();
    let renderer = renderer(&theme);
    let line = Line::from(vec![
        Span::styled("abc", Style::default()),
        Span::styled("def", Style::default()),
    ]);

    let trimmed = renderer.trim_with_ellipsis(line, false);

    assert_eq!(spans_text(&trimmed), "abcd..");
}

#[test]
fn continuation_sub_line_content_applies_hscroll() {
    let theme = ColorTheme::default();
    let mut renderer = renderer(&theme);
    renderer.manual_hscroll = 2;

    let line = renderer.continuation_sub_line_content("abcdefgh", false);

    assert_eq!(line_text(&line), "..cdef");
}

#[test]
fn first_sub_line_content_keeps_display_longer_than_text() {
    let theme = ColorTheme::default();
    let renderer = renderer(&theme);
    let item = MatchedItem::new(
        Arc::new(DisplayLongerThanText) as Arc<dyn SkimItem>,
        Rank::default(),
        None,
        &RankBuilder::default(),
    );

    let line = renderer.first_sub_line_content(&item, "ab", false, 0, 0);

    assert_eq!(line_text(&line), "abcd..");
}

#[test]
fn calc_hscroll_container_narrower_than_ellipsis_returns_no_shift() {
    let theme = ColorTheme::default();
    let mut renderer = renderer(&theme);
    // ellipsis ".." is width 2; a 1-cell container can't fit it.
    renderer.container_width = 1;
    let (shift, full_width, has_left, has_right) = renderer.calc_hscroll_for_width("hello world", 0, 0, 11);
    assert_eq!(shift, 0);
    assert_eq!(full_width, 11);
    assert!(!has_left);
    assert!(!has_right);
}

#[test]
fn calc_hscroll_keep_right_shifts_to_end() {
    let theme = ColorTheme::default();
    let mut renderer = renderer(&theme);
    renderer.container_width = 6;
    renderer.keep_right = true;
    // No match (0,0) + keep_right → shift so the right edge is visible.
    let (shift, _full, _l, _r) = renderer.calc_hscroll_for_width("hello world", 0, 0, 11);
    // full_width(11) - available_width(6) = 5.
    assert_eq!(shift, 5);
}

#[test]
fn calc_hscroll_match_wider_than_available_anchors_at_match_start() {
    let theme = ColorTheme::default();
    let mut renderer = renderer(&theme);
    renderer.container_width = 4;
    // Match spans chars 2..10 (width 8) which exceeds the 4-cell container,
    // so the shift anchors at the match start width.
    let (shift, _full, _l, _r) = renderer.calc_hscroll_for_width("abcdefghijkl", 2, 10, 12);
    assert_eq!(shift, 2);
}

#[test]
fn render_item_with_multiline_skip_starts_at_skipped_sub_line() {
    let theme = ColorTheme::default();
    let mut renderer = renderer(&theme);
    renderer.multiline = Some("|");
    renderer.container_width = 20;

    let item = matched_item("first|second|third", None);
    let mut out = Vec::new();

    let added = renderer.render_item(&item, false, false, 1, 10, 0, &mut out);

    assert_eq!(added, 2);
    assert_eq!(rendered_row_text(out.remove(0), 8), "  second");
    assert_eq!(rendered_row_text(out.remove(0), 8), "  third ");
    assert!(out.is_empty());
}
