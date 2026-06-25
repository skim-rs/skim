#![allow(clippy::field_reassign_with_default)]

use super::*;
use crate::Rank;
use crate::item::RankBuilder;

fn matched(text: &str, index: i32) -> MatchedItem {
    let item: std::sync::Arc<dyn crate::SkimItem> = std::sync::Arc::new(text.to_string());
    MatchedItem::new(
        item,
        Rank {
            index,
            ..Default::default()
        },
        None,
        &RankBuilder::default(),
    )
}

fn list(n: usize) -> ItemList {
    let mut il = ItemList::default();
    let mut items: Vec<MatchedItem> = (0..n)
        .map(|i| matched(&format!("item{i}"), i32::try_from(i).unwrap()))
        .collect();
    il.append(&mut items);
    il.height = 10;
    il
}

#[test]
fn selection_and_selected_skip_disabled_items() {
    use std::borrow::Cow;

    // An item flagged disabled (as `--disable-pattern` does) cannot become the
    // selection, so accepting it yields no output.
    struct Disabled(&'static str);
    impl crate::SkimItem for Disabled {
        fn text(&self) -> Cow<'_, str> {
            Cow::Borrowed(self.0)
        }
        fn disabled(&self) -> bool {
            true
        }
    }

    let rb = RankBuilder::default();
    let disabled = MatchedItem::new(
        std::sync::Arc::new(Disabled("foo")) as std::sync::Arc<dyn crate::SkimItem>,
        Rank::default(),
        None,
        &rb,
    );
    let mut il = ItemList::default();
    il.append(&mut vec![disabled, matched("bar", 1)]);
    il.height = 10;

    // Cursor on the disabled row: `selected()` returns nothing.
    il.select_row(0); // no-op for a disabled item
    assert!(il.selected().is_none());
    assert!(il.selection.is_empty());

    // Toggling the disabled row also adds nothing.
    il.toggle_at(0);
    assert!(il.selection.is_empty());

    // The enabled row behaves normally.
    il.toggle_at(1);
    assert_eq!(il.selection.len(), 1);
    assert_eq!(il.selection[0].text(), "bar");
}

#[test]
fn scroll_by_clamps_within_bounds() {
    let mut il = list(5);
    il.scroll_by(2);
    assert_eq!(il.current, 2);
    // Can't go below 0.
    il.scroll_by(-10);
    assert_eq!(il.current, 0);
    // Can't exceed last index.
    il.scroll_by(100);
    assert_eq!(il.current, 4);
}

#[test]
fn scroll_by_cycles_when_enabled() {
    let mut il = list(3);
    il.cycle = true;
    il.current = 2;
    il.scroll_by(1); // wraps to 0
    assert_eq!(il.current, 0);
    il.scroll_by(-1); // wraps to 2
    assert_eq!(il.current, 2);
}

#[test]
fn scroll_by_rows_moves_cursor() {
    let mut il = list(10);
    il.scroll_by_rows(3);
    assert_eq!(il.current, 3);
    il.scroll_by_rows(-2);
    assert_eq!(il.current, 1);
    // Zero is a no-op.
    il.scroll_by_rows(0);
    assert_eq!(il.current, 1);
}

#[test]
fn select_next_previous() {
    let mut il = list(4);
    il.select_next();
    assert_eq!(il.current, 1);
    il.select_previous();
    assert_eq!(il.current, 0);
}

#[test]
fn jump_to_first_and_last() {
    let mut il = list(6);
    il.jump_to_last();
    assert_eq!(il.current, 5);
    il.jump_to_first();
    assert_eq!(il.current, 0);
}

#[test]
fn item_at_visual_row_top_to_bottom() {
    let mut il = list(5);
    il.direction = ListDirection::TopToBottom;
    assert_eq!(il.item_at_visual_row(0), Some(0));
    assert_eq!(il.item_at_visual_row(2), Some(2));
    // Row beyond available height.
    assert_eq!(il.item_at_visual_row(100), None);
}

#[test]
fn item_at_visual_row_bottom_to_top() {
    let mut il = list(10);
    il.direction = ListDirection::BottomToTop;
    // Bottom row (height-1) is the first item.
    assert_eq!(il.item_at_visual_row(il.height as usize - 1), Some(0));
}

#[test]
fn toggle_and_toggle_all() {
    let mut il = list(3);
    il.multi_select = true;
    il.toggle(); // toggle current (0)
    assert_eq!(il.selection.len(), 1);
    il.toggle(); // untoggle
    assert_eq!(il.selection.len(), 0);
    il.toggle_all();
    assert_eq!(il.selection.len(), 3);
}

#[test]
fn select_and_select_all_and_clear() {
    let mut il = list(3);
    il.select();
    assert_eq!(il.selection.len(), 1);
    il.select_all();
    assert_eq!(il.selection.len(), 3);
    il.clear_selection();
    assert!(il.selection.is_empty());
}

#[test]
fn clear_resets_state() {
    let mut il = list(3);
    il.current = 2;
    il.select_all();
    il.clear();
    assert!(il.items.is_empty());
    assert!(il.selection.is_empty());
    assert_eq!(il.current, 0);
}

#[test]
fn count_zero_when_showing_stale() {
    let mut il = list(3);
    assert_eq!(il.count(), 3);
    il.showing_stale_items = true;
    assert_eq!(il.count(), 0);
}

#[test]
fn selected_returns_current_item() {
    let il = list(3);
    let sel = il.selected().expect("an item is selected");
    assert_eq!(sel.text(), "item0");
}

#[test]
fn select_row_adds_specific_item() {
    let mut il = list(4);
    il.select_row(2);
    assert_eq!(il.selection.len(), 1);
    let only = il.selection.iter().next().unwrap();
    assert_eq!(only.text(), "item2");
}

#[test]
fn processed_items_default_replaces() {
    let pi = ProcessedItems::default();
    assert!(pi.items.is_empty());
    assert!(matches!(pi.merge, MergeStrategy::Replace));
}

#[test]
fn toggle_at_on_empty_list_is_noop() {
    let mut il = ItemList::default();
    // No items → early return without panicking.
    il.toggle_at(0);
    assert!(il.selection.is_empty());
}

#[test]
fn item_at_visual_row_out_of_range_is_none() {
    let il = list(3);
    // A row far beyond the items maps to nothing.
    assert!(il.item_at_visual_row(999).is_none());
}

#[test]
fn append_clears_stale_flag() {
    let mut il = list(2);
    il.showing_stale_items = true;
    let mut more = vec![matched("extra", 2)];
    il.append(&mut more);
    assert!(!il.showing_stale_items);
    assert_eq!(il.items.len(), 3);
}

fn render_list(il: &mut ItemList, w: u16, h: u16) {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    let area = Rect::new(0, 0, w, h);
    let mut buf = Buffer::empty(area);
    il.render(area, &mut buf);
}

fn set_processed(il: &ItemList, items: Vec<MatchedItem>, merge: MergeStrategy) {
    *il.processed_items.lock() = Some(ProcessedItems { items, merge });
}

#[test]
fn render_applies_replace_strategy() {
    let mut il = list(2);
    set_processed(&il, vec![matched("new", 0)], MergeStrategy::Replace);
    render_list(&mut il, 20, 5);
    // Replace swaps the whole list.
    assert_eq!(il.items.len(), 1);
    assert_eq!(il.items[0].text(), "new");
}

#[test]
fn render_applies_append_strategy() {
    let mut il = list(2);
    set_processed(&il, vec![matched("extra", 2)], MergeStrategy::Append);
    render_list(&mut il, 20, 5);
    // Append grows the existing list.
    assert_eq!(il.items.len(), 3);
}

#[test]
fn render_applies_sorted_merge_strategy() {
    let mut il = ItemList::default();
    il.height = 10;
    let mut base = vec![matched("a", 0)];
    il.append(&mut base);
    set_processed(&il, vec![matched("b", 1)], MergeStrategy::SortedMerge);
    render_list(&mut il, 20, 5);
    assert_eq!(il.items.len(), 2);
}

#[test]
fn render_empty_list_does_not_panic() {
    let mut il = ItemList::default();
    il.height = 5;
    render_list(&mut il, 20, 5);
    assert!(il.items.is_empty());
}

#[test]
fn render_scrolled_list_shows_lower_items() {
    let mut il = list(30);
    il.current = 20;
    // Rendering a tall list with the cursor far down exercises the scroll path.
    render_list(&mut il, 20, 6);
    assert!(il.current < il.items.len());
}
