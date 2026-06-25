use super::*;

fn item(text: &str) -> Arc<dyn SkimItem> {
    Arc::new(text.to_string())
}

fn matched(text: &str, index: i32, score: i32) -> MatchedItem {
    let rank = Rank {
        score,
        index,
        ..Default::default()
    };
    MatchedItem::new(item(text), rank, None, &RankBuilder::default())
}

#[test]
fn rank_builder_inserts_score_criterion() {
    // Score is implicit and prepended when missing.
    let rb = RankBuilder::new(vec![RankCriteria::Begin]);
    assert_eq!(rb.criteria().first(), Some(&RankCriteria::Score));

    // Already present -> not duplicated.
    let rb = RankBuilder::new(vec![RankCriteria::NegScore, RankCriteria::Begin]);
    assert_eq!(rb.criteria().first(), Some(&RankCriteria::NegScore));
    assert!(!rb.criteria().contains(&RankCriteria::Score));
}

#[test]
fn build_rank_records_offsets_and_pathname() {
    let rb = RankBuilder::default();
    let rank = rb.build_rank(50, 4, 7, "src/lib/foo.rs");
    assert_eq!(rank.score, 50);
    assert_eq!(rank.begin, 4);
    assert_eq!(rank.end, 7);
    assert_eq!(rank.length, i32::try_from("src/lib/foo.rs".len()).unwrap());
    // path_name_offset points just past the last separator.
    assert_eq!(rank.path_name_offset, i32::try_from("src/lib/".len()).unwrap());
}

#[test]
fn sort_key_flips_score_sign() {
    let rank = Rank {
        score: 10,
        begin: 2,
        end: 5,
        ..Default::default()
    };
    let key = rank.sort_key(&[RankCriteria::Score, RankCriteria::Begin, RankCriteria::End]);
    assert_eq!(key[0], -10);
    assert_eq!(key[1], 2);
    assert_eq!(key[2], 5);
}

#[test]
fn matched_item_ordering_prefers_higher_score() {
    let high = matched("a", 0, 100);
    let low = matched("b", 1, 10);
    // Higher score sorts first (smaller sort key).
    assert!(high < low);
}

#[test]
fn sorted_merge_handles_empty_inputs() {
    let a = vec![matched("a", 0, 10)];
    assert_eq!(MatchedItem::sorted_merge(a.clone(), vec![]).len(), 1);
    assert_eq!(MatchedItem::sorted_merge(vec![], a.clone()).len(), 1);
}

#[test]
fn sorted_merge_interleaves_in_order() {
    let existing = vec![matched("a", 0, 100), matched("c", 2, 10)];
    let incoming = vec![matched("b", 1, 50)];
    let merged = MatchedItem::sorted_merge(existing, incoming);
    let scores: Vec<i32> = merged.iter().map(|m| m.rank.score).collect();
    assert_eq!(scores, vec![100, 50, 10]);
}

#[test]
fn sorted_merge_prepends_when_incoming_all_better() {
    // Incoming all rank ahead of existing → fast prepend path.
    let existing = vec![matched("c", 2, 10)];
    let incoming = vec![matched("a", 0, 100), matched("b", 1, 50)];
    let merged = MatchedItem::sorted_merge(existing, incoming);
    let scores: Vec<i32> = merged.iter().map(|m| m.rank.score).collect();
    assert_eq!(scores, vec![100, 50, 10]);
}

#[test]
fn sorted_merge_appends_when_existing_all_better() {
    // Existing all rank ahead of incoming → fast append path (existing.last() <= incoming.first()).
    let existing = vec![matched("a", 0, 100), matched("b", 1, 50)];
    let incoming = vec![matched("c", 2, 10)];
    let merged = MatchedItem::sorted_merge(existing, incoming);
    let scores: Vec<i32> = merged.iter().map(|m| m.rank.score).collect();
    assert_eq!(scores, vec![100, 50, 10]);
}

#[test]
fn sorted_merge_drains_incoming_tail() {
    // Interleave so the merge loop runs and `existing` exhausts first,
    // leaving an incoming tail to drain via the (None, _) arm.
    let existing = vec![matched("a", 0, 100), matched("c", 2, 40)];
    let incoming = vec![matched("b", 1, 50), matched("d", 3, 10)];
    let merged = MatchedItem::sorted_merge(existing, incoming);
    let scores: Vec<i32> = merged.iter().map(|m| m.rank.score).collect();
    assert_eq!(scores, vec![100, 50, 40, 10]);
}

#[test]
fn matched_item_debug_includes_text_and_rank() {
    let s = format!("{:?}", matched("hello", 3, 42));
    assert!(s.contains("MatchedItem"));
    assert!(s.contains("hello"));
}

#[test]
fn merge_into_sorted_small_insert() {
    let mut existing = vec![matched("a", 0, 100), matched("c", 2, 10)];
    MatchedItem::merge_into_sorted(&mut existing, vec![matched("b", 1, 50)]);
    let scores: Vec<i32> = existing.iter().map(|m| m.rank.score).collect();
    assert_eq!(scores, vec![100, 50, 10]);
}

#[test]
fn merge_into_sorted_large_uses_backwards_merge() {
    // Force the > SMALL_INSERT_THRESHOLD branch with interleaving order.
    let mut existing: Vec<MatchedItem> = (0i32..300).map(|i| matched("x", i, 1000 - i * 2)).collect();
    let incoming: Vec<MatchedItem> = (0i32..300).map(|i| matched("y", 1000 + i, 1000 - i * 2 - 1)).collect();
    let total = existing.len() + incoming.len();
    MatchedItem::merge_into_sorted(&mut existing, incoming);
    assert_eq!(existing.len(), total);
    // Result must be sorted ascending by sort key.
    assert!(existing.windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn merge_into_sorted_large_with_incoming_holding_best() {
    // >256 incoming forces the backwards in-place merge. Give incoming the
    // highest scores so `existing` exhausts first, leaving an incoming run
    // that is block-copied to the front (the `bi > 0` branch).
    let mut existing: Vec<MatchedItem> = (0i32..300).map(|i| matched("x", i, 500 - i * 2)).collect();
    let incoming: Vec<MatchedItem> = (0i32..300).map(|i| matched("y", 1000 + i, 2000 - i * 2)).collect();
    let total = existing.len() + incoming.len();
    MatchedItem::merge_into_sorted(&mut existing, incoming);
    assert_eq!(existing.len(), total);
    assert!(existing.windows(2).all(|w| w[0] <= w[1]));
    // The top-scoring item came from the incoming batch.
    assert_eq!(existing[0].rank.score, 2000);
}

#[test]
fn downcast_item_recovers_concrete_type() {
    let m = matched("hello", 0, 1);
    let s: Option<&String> = m.downcast_item::<String>();
    assert_eq!(s.map(String::as_str), Some("hello"));
}

#[test]
fn item_pool_append_take_and_counters() {
    let pool = ItemPool::new();
    assert!(pool.is_empty());

    pool.append(vec![item("a"), item("b"), item("c")]);
    assert_eq!(pool.len(), 3);
    assert_eq!(pool.num_not_taken(), 3);
    assert_eq!(pool.num_taken(), 0);

    let taken = pool.take();
    assert_eq!(taken.len(), 3);
    assert_eq!(pool.num_taken(), 3);
    assert_eq!(pool.num_not_taken(), 0);

    // A second take yields nothing new.
    assert!(pool.take().is_empty());
}

#[test]
fn item_pool_reset_replays_items() {
    let pool = ItemPool::new();
    pool.append(vec![item("a"), item("b")]);
    let _ = pool.take();
    assert_eq!(pool.num_not_taken(), 0);
    pool.reset();
    assert_eq!(pool.num_not_taken(), 2);
    assert_eq!(pool.take().len(), 2);
}

#[test]
fn item_pool_clear_empties_everything() {
    let pool = ItemPool::new();
    pool.append(vec![item("a"), item("b")]);
    pool.clear();
    assert!(pool.is_empty());
    assert_eq!(pool.num_taken(), 0);
}

#[test]
fn item_pool_reserves_header_lines() {
    let mut options = crate::SkimOptions::default();
    options.header_lines = 2;
    let pool = ItemPool::from_options(&options);
    pool.append(vec![item("h1"), item("h2"), item("body1"), item("body2")]);
    let reserved = pool.reserved();
    assert_eq!(reserved.len(), 2);
    assert_eq!(reserved[0].text(), "h1");
    // Reserved header items are not part of the main matchable pool.
    assert_eq!(pool.len(), 2);
}

#[test]
fn item_pool_tac_reverses_take_order() {
    let mut options = crate::SkimOptions::default();
    options.tac = true;
    let pool = ItemPool::from_options(&options);
    pool.append(vec![item("a"), item("b"), item("c")]);
    let taken: Vec<String> = pool.take().iter().map(|i| i.text().into_owned()).collect();
    assert_eq!(taken, vec!["c", "b", "a"]);
}
