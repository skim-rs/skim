use super::*;

fn item(text: &str) -> Arc<dyn SkimItem> {
    Arc::new(text.to_string())
}

fn entry(index: i32, score: i32) -> MatchEntry {
    let rank = Rank {
        score,
        index,
        ..Default::default()
    };
    MatchEntry::new(&rank, &RankBuilder::default())
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
    let rb = RankBuilder::new(vec![RankCriteria::NegPathName]);
    let rank = rb.build_rank(50, 4, 7, "src/lib/foo.rs");
    assert_eq!(rank.score, 50);
    assert_eq!(rank.begin, 4);
    assert_eq!(rank.end, 7);
    assert_eq!(rank.length, i32::try_from("src/lib/foo.rs".len()).unwrap());
    // path_name_offset points just past the last separator.
    assert_eq!(rank.path_name_offset, i32::try_from("src/lib/".len()).unwrap());

    // Without a PathName tiebreak it is not computed.
    let rank = RankBuilder::default().build_rank(50, 4, 7, "src/lib/foo.rs");
    assert_eq!(rank.path_name_offset, 0);
}

#[test]
fn path_name_offset_counts_chars_not_bytes() {
    // `path_name_offset` is subtracted from `Rank::begin`, which is a char index,
    // so a multi-byte directory component must not inflate it.
    let rb = RankBuilder::new(vec![RankCriteria::PathName]);
    // "ééé/" is 4 chars but 7 bytes; the filename starts at char index 4.
    let rank = rb.build_rank(0, 4, 5, "ééé/a");
    assert_eq!(rank.path_name_offset, 4);

    // With the match sitting on the filename, PathName must score it as 0 (best).
    // A byte-based offset would give 7 - 4 = 3 and rank it below a plain match.
    assert_eq!(rank.sort_key(&[RankCriteria::PathName])[0], 0);
}

#[test]
fn pathname_tiebreak_prefers_filename_match_with_non_ascii_dir() {
    // "ééééé/a" matches in the filename (best), "a/xxxxx" matches in the dir part.
    let rb = RankBuilder::new(vec![RankCriteria::PathName, RankCriteria::Index]);
    let in_filename = MatchEntry::new(&rb.build_rank(0, 6, 7, "ééééé/a"), &rb);
    let in_dir = MatchEntry::new(&rb.build_rank(0, 0, 1, "a/xxxxx"), &rb);

    assert!(
        in_filename < in_dir,
        "a filename match must outrank a directory match even when the directory is non-ASCII"
    );
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
fn match_entry_ordering_prefers_higher_score() {
    assert!(entry(0, 100) < entry(1, 10));
}

#[test]
fn match_entry_ordering_reverses_stable_input_index_for_tac() {
    let old = Rank::default();
    let new = Rank {
        index: 1,
        ..Rank::default()
    };
    let by_index = RankBuilder::new(vec![RankCriteria::Index]);
    assert!(MatchEntry::new(&old, &by_index) < MatchEntry::new(&new, &by_index));
    for builder in [by_index.tac(true), RankBuilder::default().tac(true)] {
        let (a, b) = (MatchEntry::new(&old, &builder), MatchEntry::new(&new, &builder));
        assert!(b < a);
        assert_eq!(b.index, 1);
        assert_eq!(MatchEntry::merge_sorted(&[a], &[b]), [b, a]);
    }
}

#[test]
fn match_entry_comparison_agrees_with_equality_for_equal_sort_keys() {
    use RankCriteria::{Begin, End, Index, Length, PathName, Score};

    let builder = RankBuilder::new(vec![Begin, End, Index, Length, PathName, Score]);
    let a = MatchEntry::new(&Rank::default(), &builder);
    let b = MatchEntry::new(
        &Rank {
            score: 1,
            ..Rank::default()
        },
        &builder,
    );
    assert_eq!(a.sort_key, b.sort_key);
    assert_ne!(a, b);
    assert_ne!(a.cmp(&b), std::cmp::Ordering::Equal);
}

#[test]
fn merge_sorted_equals_concat_sort() {
    use rand::rngs::StdRng;
    use rand::{RngExt as _, SeedableRng as _};

    let mut rng = StdRng::seed_from_u64(42);
    let sizes = [0, 1, 255, 256, 257, 4096];
    for a_len in sizes {
        for b_len in sizes {
            // Interleave scores, then exercise each fast path with disjoint scores.
            for shift in [0, -100, 100] {
                let mut a: Vec<_> = (0..a_len).map(|i| entry(i, rng.random_range(0..100))).collect();
                let mut b: Vec<_> = (a_len..a_len + b_len)
                    .map(|i| entry(i, rng.random_range(0..100) + shift))
                    .collect();
                a.sort_unstable();
                b.sort_unstable();
                let mut expected = [&a[..], &b[..]].concat();
                expected.sort_unstable();
                assert_eq!(MatchEntry::merge_sorted(&a, &b), expected);
            }
        }
    }
}

#[test]
fn matched_item_identity_uses_text_and_index_not_score() {
    let m = MatchedItem::new(item("hello"), 3, 42);
    assert_eq!(m, MatchedItem::new(item("hello"), 3, 0));
    assert_ne!(m, MatchedItem::new(item("other"), 3, 42));
    assert_ne!(m, MatchedItem::new(item("hello"), 4, 42));
    assert!(format!("{m:?}").contains("hello"));
    assert_eq!(m.downcast_item::<String>().map(String::as_str), Some("hello"));
}

#[test]
fn manually_constructed_item_has_no_match_details() {
    let item = MatchedItem::new(item("hello"), 3, 42);
    assert!(item.rank().is_none());
    assert!(item.match_result().is_none());
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
    let _ = pool.take();
    let old = pool.clear();
    assert_eq!(old.iter().map(|item| item.text()).collect::<Vec<_>>(), ["a", "b"]);
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

#[test]
fn empty_query_order_follows_index_criteria() {
    assert_eq!(RankBuilder::default().empty_query_order(), Some(false));
    assert_eq!(RankBuilder::default().tac(true).empty_query_order(), Some(true));
    assert_eq!(
        RankBuilder::new(vec![RankCriteria::NegIndex]).empty_query_order(),
        Some(true)
    );
    assert_eq!(
        RankBuilder::new(vec![RankCriteria::NegIndex])
            .tac(true)
            .empty_query_order(),
        Some(false)
    );
    // The order also depends on the item text: no shortcut.
    assert_eq!(RankBuilder::new(vec![RankCriteria::Length]).empty_query_order(), None);
    assert_eq!(
        RankBuilder::new(vec![RankCriteria::NegPathName]).empty_query_order(),
        None
    );
}
