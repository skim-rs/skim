use super::*;
use crate::options::SkimOptionsBuilder;
use crate::tui::options::PreviewLayout;

// A convenient 80×24 terminal area.
fn area() -> Rect {
    Rect::new(0, 0, 80, 24)
}

// ── helpers ────────────────────────────────────────────────────────────

/// Assert that `rect` covers the full `area` width.
fn assert_full_width(rect: Rect, area: Rect, label: &str) {
    assert_eq!(rect.x, area.x, "{label}: x");
    assert_eq!(rect.width, area.width, "{label}: width");
}

/// Assert that two rects are vertically adjacent (b starts immediately
/// below a).
fn assert_vertically_adjacent(a: Rect, b: Rect, label: &str) {
    assert_eq!(a.y + a.height, b.y, "{label}: b should start right after a");
}

/// Assert that two rects are horizontally adjacent (b starts immediately
/// to the right of a).
fn assert_horizontally_adjacent(a: Rect, b: Rect, label: &str) {
    assert_eq!(a.x + a.width, b.x, "{label}: b should start right after a");
}

fn assert_vertical_border_overlap(a: Rect, b: Rect, label: &str) {
    assert_eq!(a.y + a.height - 1, b.y, "{label}: borders should share one row");
}

fn assert_horizontal_border_overlap(a: Rect, b: Rect, label: &str) {
    assert_eq!(a.x + a.width - 1, b.x, "{label}: borders should share one column");
}

// Compute layout with no reserved-item header lines (header_height = 0
// unless the test needs something different).
fn compute(options: &SkimOptions) -> AppLayout {
    AppLayout::compute(area(), options, 0)
}

fn compute_with_header_height(options: &SkimOptions, header_height: u16) -> AppLayout {
    AppLayout::compute(area(), options, header_height)
}

fn opts() -> SkimOptionsBuilder {
    SkimOptionsBuilder::default()
}

// ── Default layout ─────────────────────────────────────────────────────

#[test]
fn default_no_preview_no_header() {
    // Simple case: just list + input (status line takes 1 extra row).
    let options = opts().build().unwrap();
    let layout = compute(&options);

    // input = 2 rows (1 prompt + 1 status)
    assert_eq!(layout.input_area.height, 2);
    // list fills the rest
    assert_eq!(layout.list_area.height, 24 - 2);
    assert!(layout.header_area.is_none());
    assert!(layout.preview_area.is_none());

    // list is above input
    assert_full_width(layout.list_area, area(), "list");
    assert_full_width(layout.input_area, area(), "input");
    assert_vertically_adjacent(layout.list_area, layout.input_area, "list→input");
}

#[test]
fn default_inline_info_no_header() {
    // InfoDisplay::Inline → input uses only 1 row.
    let options = opts().inline_info(true).build().unwrap();
    let layout = compute(&options);

    assert_eq!(layout.input_area.height, 1);
    assert_eq!(layout.list_area.height, 23);
}

#[test]
fn default_hidden_info_no_header() {
    // InfoDisplay::Hidden → input also uses only 1 row (just the prompt).
    let options = opts().no_info(true).build().unwrap();
    let layout = compute(&options);

    assert_eq!(layout.input_area.height, 1);
    assert_eq!(layout.list_area.height, 23);
}

#[test]
fn default_with_header() {
    let options = opts().header("My Header").build().unwrap();
    // header_height = 1 line of static header text
    let layout = compute_with_header_height(&options, 1);

    // non-list = input(2) + header(1) = 3
    assert_eq!(layout.list_area.height, 21);
    assert_eq!(layout.header_area.unwrap().height, 1);
    assert_eq!(layout.input_area.height, 2);

    // Order: list | header | input (top to bottom)
    let h = layout.header_area.unwrap();
    assert_vertically_adjacent(layout.list_area, h, "list→header");
    assert_vertically_adjacent(h, layout.input_area, "header→input");
}

#[test]
fn default_with_multiline_header() {
    let options = opts().header("line1\nline2\nline3").build().unwrap();
    let layout = compute_with_header_height(&options, 3);

    // non-list = input(2) + header(3) = 5
    assert_eq!(layout.list_area.height, 19);
    assert_eq!(layout.header_area.unwrap().height, 3);
}

#[test]
fn default_with_header_lines() {
    // header_lines > 0 also triggers show_header
    let options = opts().header_lines(2usize).build().unwrap();
    let layout = compute_with_header_height(&options, 2);

    assert_eq!(layout.header_area.unwrap().height, 2);
    assert_eq!(layout.list_area.height, 24 - 2 - 2);
}

// ── Template can be reused across different areas ───────────────────────

#[test]
fn template_apply_different_areas() {
    // A single template should produce consistent proportional results
    // regardless of which concrete area it is applied to.
    let options = opts().build().unwrap();
    let template = LayoutTemplate::from_options(&options, 0);

    let small = template.apply(Rect::new(0, 0, 40, 12));
    let large = template.apply(Rect::new(0, 0, 160, 48));

    // Both should have input = 2 rows, list = height - 2.
    assert_eq!(small.input_area.height, 2);
    assert_eq!(small.list_area.height, 10);
    assert_eq!(large.input_area.height, 2);
    assert_eq!(large.list_area.height, 46);
}

// ── Reverse layout ─────────────────────────────────────────────────────

#[test]
fn reverse_no_preview_no_header() {
    let options = opts().layout(TuiLayout::Reverse).build().unwrap();
    let layout = compute(&options);

    // input is at the top
    assert_eq!(layout.input_area.y, 0);
    assert_eq!(layout.input_area.height, 2);
    // list is below input
    assert_eq!(layout.list_area.y, 2);
    assert_eq!(layout.list_area.height, 22);

    assert_vertically_adjacent(layout.input_area, layout.list_area, "input→list");
}

#[test]
fn reverse_with_header() {
    // In Reverse layout: input on top, header below input, then list.
    let options = opts().layout(TuiLayout::Reverse).header("hdr").build().unwrap();
    let layout = compute_with_header_height(&options, 2);

    // non-list = input(2) + header(2) = 4
    assert_eq!(layout.list_area.height, 20);
    let h = layout.header_area.unwrap();
    assert_eq!(h.height, 2);

    // Order: input | header | list
    assert_vertically_adjacent(layout.input_area, h, "input→header");
    assert_vertically_adjacent(h, layout.list_area, "header→list");
}

// ── ReverseList layout ─────────────────────────────────────────────────

#[test]
fn reverse_list_no_preview_no_header() {
    // ReverseList: items top-to-bottom (same split as Default), input at
    // the bottom.
    let options = opts().layout(TuiLayout::ReverseList).build().unwrap();
    let layout = compute(&options);

    assert_eq!(layout.input_area.height, 2);
    assert_eq!(layout.list_area.height, 22);
    assert_vertically_adjacent(layout.list_area, layout.input_area, "list→input");
}

#[test]
fn reverse_list_with_header() {
    let options = opts().layout(TuiLayout::ReverseList).header("hdr").build().unwrap();
    let layout = compute_with_header_height(&options, 1);

    // non-list = 2 + 1 = 3
    assert_eq!(layout.list_area.height, 21);
    let h = layout.header_area.unwrap();
    assert_eq!(h.height, 1);
    // Order: list | header | input
    assert_vertically_adjacent(layout.list_area, h, "list→header");
    assert_vertically_adjacent(h, layout.input_area, "header→input");
}

// ── Preview: Left / Right ───────────────────────────────────────────────

#[test]
fn default_preview_right_50_percent() {
    let options = opts()
        .preview("cat {}")
        .preview_window(PreviewLayout::from("right:50%"))
        .build()
        .unwrap();
    let layout = compute(&options);

    let preview = layout.preview_area.unwrap();
    // Preview is on the right: work_area = left half, preview = right half.
    // 50% of 80 = 40.
    assert_eq!(preview.width, 40);
    assert_eq!(preview.x, 40);
    // list and input share the same work-column width (40, not summed).
    assert_eq!(layout.list_area.width, 40);
    assert_eq!(layout.input_area.width, 40);
    assert_horizontally_adjacent(layout.list_area, preview, "list→preview");
}

#[test]
fn default_preview_left_30_percent() {
    let options = opts()
        .preview("cat {}")
        .preview_window(PreviewLayout::from("left:30%"))
        .build()
        .unwrap();
    let layout = compute(&options);

    let preview = layout.preview_area.unwrap();
    // Preview on the left: 30% of 80 = 24.
    assert_eq!(preview.width, 24);
    assert_eq!(preview.x, 0);
    // work_area starts after preview
    assert_eq!(layout.list_area.x, 24);
    assert_horizontally_adjacent(preview, layout.list_area, "preview→list");
}

#[test]
fn default_preview_right_fixed_20() {
    let options = opts()
        .preview("cat {}")
        .preview_window(PreviewLayout::from("right:20"))
        .build()
        .unwrap();
    let layout = compute(&options);

    let preview = layout.preview_area.unwrap();
    assert_eq!(preview.width, 20);
    assert_eq!(layout.list_area.width, 60);
}

#[test]
fn reverse_preview_left() {
    let options = opts()
        .layout(TuiLayout::Reverse)
        .preview("cat {}")
        .preview_window(PreviewLayout::from("left:40%"))
        .build()
        .unwrap();
    let layout = compute(&options);

    let preview = layout.preview_area.unwrap();
    // 40% of 80 = 32
    assert_eq!(preview.width, 32);
    // Input is at the top of work_area (Reverse)
    assert_eq!(layout.input_area.y, 0);
    assert_eq!(layout.input_area.x, 32);
}

// ── Preview: Up / Down ──────────────────────────────────────────────────

#[test]
fn default_preview_up_50_percent() {
    let options = opts()
        .preview("cat {}")
        .preview_window(PreviewLayout::from("up:50%"))
        .build()
        .unwrap();
    let layout = compute(&options);

    let preview = layout.preview_area.unwrap();
    // Preview is carved from the full area first: 50% of 24 = 12 rows.
    assert_eq!(preview.height, 12);
    // Preview is at the top (y = 0).
    assert_eq!(preview.y, 0);
    // work_area starts right after the preview.
    assert_eq!(layout.list_area.y, 12);
    // work_area height = 24 - 12 = 12; input = 2; list = 10.
    assert_eq!(layout.list_area.height, 10);
    // input is at the bottom of the work area.
    assert_eq!(layout.input_area.y, 22);
}

#[test]
fn default_preview_down_50_percent() {
    let options = opts()
        .preview("cat {}")
        .preview_window(PreviewLayout::from("down:50%"))
        .build()
        .unwrap();
    let layout = compute(&options);

    let preview = layout.preview_area.unwrap();
    // Preview is carved from the full area: 50% of 24 = 12 rows at bottom.
    assert_eq!(preview.height, 12);
    // work_area is at the top (y = 0); preview starts after work_area.
    assert_eq!(layout.list_area.y, 0);
    assert_eq!(layout.list_area.height, 10);
    // input is at the bottom of work_area (y = 10).
    assert_eq!(layout.input_area.y, 10);
    // Preview starts right after work_area (y = 12).
    assert_eq!(preview.y, 12);
    assert_vertically_adjacent(layout.input_area, preview, "input→preview");
}

#[test]
fn default_preview_up_fixed_8() {
    let options = opts()
        .preview("cat {}")
        .preview_window(PreviewLayout::from("up:8"))
        .build()
        .unwrap();
    let layout = compute(&options);

    let preview = layout.preview_area.unwrap();
    // Preview = 8 rows at top; work_area = 24 - 8 = 16 rows; list = 14 rows.
    assert_eq!(preview.height, 8);
    assert_eq!(preview.y, 0);
    assert_eq!(layout.list_area.height, 14);
    assert_full_width(preview, area(), "preview");
}

// ── Preview hidden ─────────────────────────────────────────────────────

#[test]
fn preview_hidden_produces_no_preview_area() {
    let options = opts()
        .preview("cat {}")
        .preview_window(PreviewLayout::from("right:50%:hidden"))
        .build()
        .unwrap();
    let layout = compute(&options);

    assert!(layout.preview_area.is_none());
    // Full width available to widgets.
    assert_eq!(layout.list_area.width, 80);
}

#[test]
fn no_preview_command_produces_no_preview_area() {
    // preview is None → no preview area even if preview_window is set.
    let options = opts().preview_window(PreviewLayout::from("right:50%")).build().unwrap();
    let layout = compute(&options);

    assert!(layout.preview_area.is_none());
    assert_eq!(layout.list_area.width, 80);
}

// ── With borders ───────────────────────────────────────────────────────

#[test]
fn default_with_borders_no_header() {
    let options = opts().border(crate::tui::BorderType::Plain).build().unwrap();
    let layout = compute(&options);

    // input = 3 rows (1 content + 2 border), sharing one border row with the list.
    assert_eq!(layout.input_area.height, 3);
    assert_eq!(layout.list_area.height, 22);
    assert_vertical_border_overlap(layout.list_area, layout.input_area, "list→input");
    assert!(layout.header_area.is_none());
}

#[test]
fn default_with_borders_and_header() {
    let options = opts()
        .border(crate::tui::BorderType::Plain)
        .header("hdr")
        .build()
        .unwrap();
    let layout = compute_with_header_height(&options, 2);

    // input = 3, header = 2+2 = 4; both separators are shared.
    assert_eq!(layout.input_area.height, 3);
    let h = layout.header_area.unwrap();
    assert_eq!(h.height, 4);
    assert_eq!(layout.list_area.height, 19);
    assert_vertical_border_overlap(layout.list_area, h, "list→header");
    assert_vertical_border_overlap(h, layout.input_area, "header→input");
}

#[test]
fn reverse_with_borders() {
    let options = opts()
        .layout(TuiLayout::Reverse)
        .border(crate::tui::BorderType::Plain)
        .build()
        .unwrap();
    let layout = compute(&options);

    // input at top (y = 0)
    assert_eq!(layout.input_area.y, 0);
    assert_eq!(layout.input_area.height, 3);
    assert_eq!(layout.list_area.y, 2);
    assert_eq!(layout.list_area.height, 22);
    assert_vertical_border_overlap(layout.input_area, layout.list_area, "input→list");
}

#[test]
fn border_no_collapse_keeps_separate_widget_areas() {
    let options = opts()
        .border(crate::tui::BorderType::Plain)
        .border_no_collapse(true)
        .header("hdr")
        .build()
        .unwrap();
    let layout = compute_with_header_height(&options, 2);
    let header = layout.header_area.unwrap();

    assert_eq!(layout.list_area.height, 17);
    assert_vertically_adjacent(layout.list_area, header, "list→header");
    assert_vertically_adjacent(header, layout.input_area, "header→input");
}

// ── Coverage / edge cases ──────────────────────────────────────────────

#[test]
fn all_areas_non_overlapping_default() {
    // Ensure no area overlaps another for a complex configuration.
    let options = opts()
        .preview("cat {}")
        .preview_window(PreviewLayout::from("right:40%"))
        .header("hdr")
        .build()
        .unwrap();
    let layout = compute_with_header_height(&options, 2);

    let preview = layout.preview_area.unwrap();
    let header = layout.header_area.unwrap();

    // Preview and work area must not overlap horizontally.
    assert!(
        layout.list_area.x + layout.list_area.width <= preview.x || preview.x + preview.width <= layout.list_area.x,
        "list and preview overlap"
    );

    // Vertical areas within work column must not overlap.
    let rects = [layout.list_area, header, layout.input_area];
    for i in 0..rects.len() {
        for j in (i + 1)..rects.len() {
            let a = rects[i];
            let b = rects[j];
            let vertically_disjoint = a.y + a.height <= b.y || b.y + b.height <= a.y;
            assert!(vertically_disjoint, "rects[{i}] and rects[{j}] overlap vertically");
        }
    }
}

#[test]
fn collapsed_borders_overlap_in_reverse_layout() {
    let options = opts()
        .layout(TuiLayout::Reverse)
        .inline_info(true)
        .border(crate::tui::BorderType::Plain)
        .preview("cat {}")
        .preview_window(PreviewLayout::from("left:25"))
        .header("hdr")
        .build()
        .unwrap();
    let layout = compute_with_header_height(&options, 1);

    let preview = layout.preview_area.unwrap();
    let header = layout.header_area.unwrap();

    // Preview and work widgets share their touching border column.
    assert_eq!(preview.x, 0);
    assert_eq!(preview.width, 25);
    assert_eq!(layout.list_area.x, 24);
    assert_horizontal_border_overlap(preview, layout.list_area, "preview→list");

    // Vertical borders are shared too (Reverse order: input, header, list).
    assert_vertical_border_overlap(layout.input_area, header, "input→header");
    assert_vertical_border_overlap(header, layout.list_area, "header→list");
}

#[test]
fn total_height_is_area_height_default() {
    // Sum of all vertical regions must equal area.height.
    let options = opts().header("hdr").build().unwrap();
    let layout = compute_with_header_height(&options, 3);
    let total = layout.list_area.height + layout.header_area.unwrap().height + layout.input_area.height;
    assert_eq!(total, area().height);
}

#[test]
fn total_width_is_area_width_with_right_preview() {
    let options = opts()
        .preview("cat {}")
        .preview_window(PreviewLayout::from("right:50%"))
        .build()
        .unwrap();
    let layout = compute(&options);
    let total = layout.list_area.width + layout.preview_area.unwrap().width;
    assert_eq!(total, area().width);
}

#[test]
fn total_height_is_area_height_with_down_preview() {
    // With a Down preview the vertical space must still sum to area height.
    let options = opts()
        .inline_info(true)
        .preview("cat {}")
        .preview_window(PreviewLayout::from("down:6"))
        .build()
        .unwrap();
    let layout = compute(&options);
    let total = layout.preview_area.unwrap().height + layout.list_area.height + layout.input_area.height;
    assert_eq!(total, area().height);
}

#[test]
fn reverse_list_with_header_and_preview_right() {
    let options = opts()
        .layout(TuiLayout::ReverseList)
        .preview("cat {}")
        .preview_window(PreviewLayout::from("right:30%"))
        .header("hdr")
        .build()
        .unwrap();
    let layout = compute_with_header_height(&options, 1);

    let preview = layout.preview_area.unwrap();
    let header = layout.header_area.unwrap();

    // Preview on the right
    assert!(preview.x > 0);
    // ReverseList: same vertical order as Default (list | header | input)
    assert_vertically_adjacent(layout.list_area, header, "list→header");
    assert_vertically_adjacent(header, layout.input_area, "header→input");
    // All in the same x-column (work area left of preview)
    assert_eq!(layout.list_area.x, layout.input_area.x);
}

#[test]
fn very_small_area() {
    // Ensure the layout does not panic on a tiny terminal.
    let tiny = Rect::new(0, 0, 20, 5);
    let options = opts().header("hdr").build().unwrap();
    // Should not panic.
    let layout = AppLayout::compute(tiny, &options, 1);
    // input and header fit, list may have zero height but must exist.
    assert_eq!(layout.list_area.width, 20);
}
