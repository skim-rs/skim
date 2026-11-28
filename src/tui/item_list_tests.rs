#![allow(clippy::field_reassign_with_default)]

use super::*;
use crate::Rank;
use crate::item::RankBuilder;

fn entry(index: i32, score: i32) -> MatchEntry {
    MatchEntry::new(
        &Rank {
            score,
            index,
            ..Rank::default()
        },
        &RankBuilder::default(),
    )
}

fn list(n: usize) -> ItemList {
    let texts: Vec<_> = (0..n).map(|i| format!("item{i}")).collect();
    let mut il = ItemList::from_texts(&texts.iter().map(String::as_str).collect::<Vec<_>>());
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

    let pool = Arc::new(ItemPool::new());
    pool.append(vec![Arc::new(Disabled("foo")), Arc::new("bar".to_owned())]);
    let mut il = ItemList::default();
    il.append(&pool, &mut vec![entry(0, 0), entry(1, 0)]);
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
    assert!(il.is_empty());
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
    let pool = pool_of(3);
    let mut il = ItemList::default();
    il.append(&pool, &mut vec![entry(0, 0), entry(1, 0)]);
    il.showing_stale_items = true;
    il.append(&pool, &mut vec![entry(2, 0)]);
    assert_eq!(il.count(), 3);
}

fn render_list(il: &mut ItemList, w: u16, h: u16) {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    let area = Rect::new(0, 0, w, h);
    let mut buf = Buffer::empty(area);
    il.render(area, &mut buf);
}

fn processed(pool: Arc<ItemPool>, indices: &[i32]) -> ProcessedItems {
    ProcessedItems {
        items: Arc::new(indices.iter().map(|&index| entry(index, 0)).collect()),
        pool,
        prepended: 0,
        generation: 0,
        engine: Arc::new(crate::engine::all::MatchAllEngine::builder().build()),
        all: None,
    }
}

#[test]
fn render_swaps_full_list() {
    let mut il = list(2);
    let next = processed(pool_of(3), &[2, 0, 1]);
    let entries = Arc::clone(&next.items);
    *il.processed_items.lock().unwrap() = Some(next);
    render_list(&mut il, 20, 5);
    assert!(Arc::ptr_eq(&entries, &il.items));
    assert_eq!(texts(&il), ["item2", "item0", "item1"]);
}

#[test]
fn get_resolves_entry_through_pool() {
    let pool = pool_of(3);
    let mut il = ItemList::default();
    il.append(&pool, &mut vec![entry(2, 42)]);
    let item = il.get(0).unwrap();
    assert_eq!(item.text(), "item2");
    assert_eq!(item.index, 2);
    assert_eq!(item.score, 42);
}

#[test]
fn selected_item_keeps_its_match_after_query_change() {
    use crate::engine::exact::ExactEngine;

    let pool = pool_of(2);
    let mut il = ItemList::default();
    il.apply_processed(ProcessedItems {
        engine: Arc::new(ExactEngine::builder("item0", Default::default()).build()),
        ..processed(Arc::clone(&pool), &[0])
    });
    il.select_row(0);
    il.apply_processed(ProcessedItems {
        engine: Arc::new(ExactEngine::builder("item1", Default::default()).build()),
        generation: 1,
        ..processed(pool, &[1])
    });

    let old = &il.selection[0];
    assert_eq!(old.text(), "item0");
    assert_eq!(old.rank().unwrap().index, 0);
    assert_eq!(
        old.match_result().unwrap().range_char_indices(&old.text()),
        [0, 1, 2, 3, 4]
    );
    assert_eq!(il.selected().unwrap().rank().unwrap().index, 1);
}

/// Highlights are computed at render time, only for the visible items.
#[test]
fn render_computes_matched_range_for_visible_items_only() {
    let mut il = ItemList::default();
    let engine = crate::engine::exact::ExactEngine::builder("item", Default::default()).build();
    *il.processed_items.lock().unwrap() = Some(ProcessedItems {
        engine: Arc::new(engine),
        ..processed(pool_of(10), &(0..10).collect::<Vec<_>>())
    });
    render_list(&mut il, 20, 3);
    let mut cached: Vec<_> = il.highlights.keys().copied().collect();
    cached.sort_unstable();
    assert_eq!(cached, [0, 1, 2]);
    assert!(il.highlights.values().all(Option::is_some));

    // Scrolling keeps only the visible rows.
    il.current = 9;
    render_list(&mut il, 20, 3);
    let mut cached: Vec<_> = il.highlights.keys().copied().collect();
    cached.sort_unstable();
    assert_eq!(cached, [7, 8, 9]);
}

#[test]
fn render_discards_results_from_stale_generation() {
    let mut il = list(2);
    *il.processed_items.lock().unwrap() = Some(processed(pool_of(1), &[0]));
    il.matcher_generation.store(1, Ordering::Release);
    render_list(&mut il, 20, 5);
    assert_eq!(texts(&il), ["item0", "item1"]);
}

#[test]
fn render_prepend_follows_head_or_preserves_focus_away_from_head() {
    let pool = pool_of(5);
    for (current, expected) in [(0, 0), (1, 3)] {
        let mut il = ItemList::default();
        il.apply_processed(processed(Arc::clone(&pool), &[2, 1, 0]));
        il.current = current;
        il.offset = current;
        *il.processed_items.lock().unwrap() = Some(ProcessedItems {
            prepended: 2,
            ..processed(Arc::clone(&pool), &[4, 3, 2, 1, 0])
        });
        render_list(&mut il, 20, 5);
        assert_eq!(il.current, expected);
        assert_eq!(il.offset, expected);
        assert_eq!(texts(&il), ["item4", "item3", "item2", "item1", "item0"]);
    }
}

#[test]
fn render_empty_list_does_not_panic() {
    let mut il = ItemList::default();
    il.height = 5;
    render_list(&mut il, 20, 5);
    assert!(il.is_empty());
}

#[test]
fn render_scrolled_list_shows_lower_items() {
    let mut il = list(30);
    il.current = 20;
    // Rendering a tall list with the cursor far down exercises the scroll path.
    render_list(&mut il, 20, 6);
    assert!(il.current < il.items.len());
}

fn pool_of(n: usize) -> Arc<ItemPool> {
    let pool = Arc::new(ItemPool::new());
    pool.append(
        (0..n)
            .map(|i| Arc::new(format!("item{i}")) as Arc<dyn SkimItem>)
            .collect(),
    );
    pool
}

fn texts(il: &ItemList) -> Vec<String> {
    il.resolve_all().iter().map(|i| i.text().into_owned()).collect()
}

#[test]
fn empty_query_lists_pool_in_index_order() {
    let pool = pool_of(4);
    for (descending, expected) in [
        (false, ["item0", "item1", "item2"]),
        (true, ["item2", "item1", "item0"]),
    ] {
        let mut il = ItemList::default();
        *il.processed_items.lock().unwrap() = Some(ProcessedItems {
            all: Some(AllItems { len: 3, descending }),
            ..processed(Arc::clone(&pool), &[])
        });
        render_list(&mut il, 20, 5);
        assert!(il.items.is_empty());
        assert_eq!(texts(&il), expected);
        let first = il.get(0).unwrap();
        assert_eq!(first.index, if descending { 2 } else { 0 });
        assert_eq!(first.score, 0);
        assert!(il.get(3).is_none());
    }
}

#[test]
fn empty_query_descending_growth_keeps_focus() {
    let pool = pool_of(5);
    let mut il = ItemList::default();
    il.apply_processed(ProcessedItems {
        all: Some(AllItems {
            len: 3,
            descending: true,
        }),
        ..processed(Arc::clone(&pool), &[])
    });
    il.current = 1;
    *il.processed_items.lock().unwrap() = Some(ProcessedItems {
        all: Some(AllItems {
            len: 5,
            descending: true,
        }),
        ..processed(Arc::clone(&pool), &[])
    });
    render_list(&mut il, 20, 5);
    assert_eq!(il.len(), 5);
    assert_eq!(il.get(il.current).unwrap().text(), "item1");
    *il.processed_items.lock().unwrap() = Some(processed(pool, &[2]));
    render_list(&mut il, 20, 5);
    assert!(il.all.is_none());
    assert_eq!(texts(&il), ["item2"]);
}

#[test]
fn detach_keeps_old_items_after_pool_clear() {
    for all in [
        None,
        Some(AllItems {
            len: 2,
            descending: true,
        }),
    ] {
        let pool = pool_of(2);
        let mut il = ItemList::default();
        il.apply_processed(ProcessedItems {
            all,
            ..processed(Arc::clone(&pool), &[1, 0])
        });
        il.detach_from_pool(&pool, pool.clear());
        pool.append(vec![Arc::new("new0".to_owned()), Arc::new("new1".to_owned())]);
        assert_eq!(texts(&il), ["item1", "item0"]);
        il.detach_from_pool(&pool, pool.clear());
        assert_eq!(texts(&il), ["item1", "item0"]);
    }
}
