# Breaking changes: two-pass matching

Skim now matches in two passes:

1. The matcher ranks **every** item with the new `MatchEngine::match_rank`.
   This gives only the rank (score, begin, end, ...) and does not compute
   the matched ranges.
2. The item list calls `MatchEngine::match_item` only for the items it
   **displays**, to get the highlights.

The `Skim::*` entry points and `SkimOptions` do not change. The ranking,
the sort order and the `--filter` output do not change either.

## `MatchedItem::matched_range` is `None` until the item is displayed

Before, all matched items had `matched_range: Some(..)`. Now the matcher
creates `MatchedItem`s with `matched_range: None`. The item list fills it in
when it renders the item.

Effect for library users: in `SkimOutput::selected_items`, items that skim
never displayed (for example after `select-all`) have `matched_range: None`.
In `--filter` mode, no item has it.

Migration: if you need the range, compute it with the engine:

```rust
let engine = Matcher::create_engine_factory(&options).create_engine_with_case(query, options.case);
let range = engine.match_item(item.item.as_ref()).map(|r| r.matched_range);
```

## `MatchEngine` has a new method, `match_rank`

```rust
fn match_rank(&self, item: &dyn SkimItem) -> Option<Rank>;
```

It has a default implementation (`match_item(item).map(|r| r.rank)`), so
existing engines still compile and work. For a custom engine:

- Skim now calls `match_rank` for every item. It calls `match_item` only for
  the displayed items, and it calls it from the UI thread.
- `match_rank` must return the same rank as `match_item`, and `match_item`
  must give the same result when it is called again.
- Override `match_rank` if your matched ranges are expensive to compute.
  Otherwise you get no speedup.

## `Rank::path_name_offset` is `0` without a `pathname` tiebreak

It costs a scan of the text for every matched item. `RankBuilder::build_rank`
now computes it only when the tiebreak includes `pathname` or `-pathname`,
because only these criteria read it. The sort order does not change.

## Arinae: `fuzzy_match` and `fuzzy_match_range` now agree with `fuzzy_indices`

`ArinaeMatcher::fuzzy_match_range` and `ArinaeMatcher::fuzzy_match` now use a
new, faster DP. Its results are equal to `fuzzy_indices`: the same items match,
with the same score. `begin` and `end` are the first and last index of
`fuzzy_indices`. Before:

- `fuzzy_match_range` could give a lower score in exact mode, and a `begin` or
  `end` that was not a matched index.
- `fuzzy_match` in typo mode did not apply the "at least half of the pattern
  matches" rule of `fuzzy_indices`, so it could match more items.

Skim used `fuzzy_indices` to rank before, so skim's ranking does not change.
Only direct callers of these two methods can see a difference.
