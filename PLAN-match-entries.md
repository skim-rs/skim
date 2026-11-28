# Plan: store an index, not the item pointer, in the result list

Status: proposal. Delete this file before the release.

## Problem

When items stream in with a non-empty query, the UI thread merges each matcher
batch into all the matches in `ItemList::render` (`MatchedItem::sorted_merge`).
Each merge is O(N) on 64-byte `MatchedItem`s that hold an `Arc<dyn SkimItem>`.
With millions of matches, one merge takes tens of ms and the UI thread does one
per render: about 62% of the UI-thread samples were in `sorted_merge`, and
wheel input waited for 200 ms or more (p90).

## Goal

1. The result list stores a 32-byte `Copy` entry: the sort key, the pool
   index and the score. There is no refcount and no `Drop`.
2. With the entry type, the matcher can build the full sorted list from a
   shared base list without refcount traffic. The UI only swaps an `Arc`.

Not in scope (see phase 4, these are gated by measurements): a segmented item
pool, coalescing of `Render` events, LSM runs with a lazy merge.

## Decisions (defaults, change them before you start)

| # | Decision | Default | Alternative |
|---|---|---|---|
| D1 | Public `MatchedItem` | `{ item: Arc<dyn SkimItem>, index: usize, score: i32 }`. `rank`, `sort_key`, `Ord` and `PartialOrd` are removed. | Keep `rank: Rank` with only `score` and `index` set. This loses data with no warning, so it is not recommended. |
| D2 | Entry layout | `MatchEntry { sort_key: [i32; 6], index: u32, score: i32 }`, 32 bytes | 24 bytes (index decoded from `sort_key[5]`, no score). Then `--show-score` and `--print-score` must match again, so it is rejected. |
| D3 | Visibility | `pub struct MatchEntry` in `skim::item` (the benches need it) | `#[doc(hidden)] pub` |
| D4 | Index limit | `i32::MAX` items, as today (the sort key slot is `i32`) | none |

`SkimItem`, `SkimOutput`, `Skim::*` and `SkimOptions` do not change, except for
the `MatchedItem` fields in `SkimOutput::selected_items` and `SkimOutput::current`.

## Phase 0: baseline (no behavior change, commit 1)

0.1 In `src/tui/app.rs`, `App::handle_event`, `Event::Render` arm: record an
`Instant` before `tui.draw` and after it log
`trace!("render done in {:?}", started.elapsed())`. `benches/ui_latency.py`
uses this line to get exact render ends.

0.2 Add `benches/merge.rs` (criterion, `harness = false`, add the
`[[bench]]` entry to `Cargo.toml`). Write it with the **current** API, then
keep the same bench IDs in phase 1 so that criterion compares them:

```rust
// IDs: merge/1M+4K, merge/3M+64K
// setup (not timed): N items sorted by random score, k new items with indices N..N+k, sorted
group.bench_function(id, |b| b.iter_batched(
    || (existing.clone(), incoming.clone()),
    |(a, b)| MatchedItem::sorted_merge(a, b),   // phase 1: MatchEntry::sorted_merge
    BatchSize::LargeInput));
```

0.3 In `benches/partial.rs`, add `ui_render_pending_batch/{100K,1M}`. Use
`iter_custom` so that only the render is timed:

```rust
// one Skim<TestBackend> with --query a, fixture loaded, wait_until_done()
b.iter_custom(|iters| {
    let mut total = Duration::ZERO;
    for _ in 0..iters {
        skim.app_mut().item_pool.append(next_10k_items());   // not timed
        skim.app_mut().restart_matcher(false);
        wait_until_done(&mut skim, timeout);
        let (app, tui) = skim.app_and_tui();
        let t = Instant::now();
        app.handle_event(tui, &Event::Render).unwrap();      // timed
        total += t.elapsed();
    }
    total
});
```

0.4 Get the baselines on one machine, with no other load:

```sh
cargo bench --bench merge          -- --save-baseline before
cargo bench --bench partial        -- ui_render --save-baseline before
cargo bench --bench read_and_match -- --save-baseline before
cargo bench --bench filter         -- --save-baseline before
cargo build --release && cp target/release/sk /tmp/sk-before
python3 benches/ui_latency.py --bin /tmp/sk-before --query a  | tee /tmp/lat-before-a.txt
python3 benches/ui_latency.py --bin /tmp/sk-before --query '' | tee /tmp/lat-before-empty.txt
command time -v /tmp/sk-before --filter a < benches/fixtures/10M.txt > /dev/null 2> /tmp/rss-before.txt
```

Check: `cargo nextest run` stays green. No snapshot changes.

## Phase 1: `MatchEntry` (commit 2, breaking)

### 1.1 `src/item.rs`

- Add:
  ```rust
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
  pub struct MatchEntry { sort_key: [i32; 6], pub index: u32, pub score: i32 }
  const _: () = assert!(std::mem::size_of::<MatchEntry>() == 32);
  impl Ord for MatchEntry { /* self.sort_key.cmp(&other.sort_key) */ }
  impl MatchEntry { pub fn new(rank: &Rank, rank_builder: &RankBuilder) -> Self }
  ```
  `sort_key[5]` holds ±index, so equal keys mean the same item. This keeps
  `Eq` and `Ord` consistent. Write this invariant in the doc comment.
- Move `sorted_merge`, `merge_into_sorted` and `merge_backwards` to
  `impl MatchEntry`. Because the type is `Copy`, rewrite `merge_backwards` with
  indices and `existing.resize(new_len, MatchEntry::default())`. This removes
  the `unsafe` block.
- `MatchedItem` (D1): the fields are `item`, `index: usize` and `score: i32`.
  `new(item, index, score)`. Keep `Hash` and `Eq` on (index, text): the
  selection must stay correct across reloads, when the same index can point
  to a different item. Keep `Deref` and `downcast_item`. Remove `sort_key`,
  `Ord` and `PartialOrd`.
- `ItemPool::clear(&self) -> Vec<Arc<dyn SkimItem>>`: return the old
  storage, so that the caller decides where it is dropped.
- Add `ItemPool::with_items<R>(&self, f: impl FnOnce(&[Arc<dyn SkimItem>]) -> R) -> R`.
  It takes one lock for bulk resolution (filter mode, `toggle_all`, pre-selection).

### 1.2 `src/tui/item_list.rs`

- Add
  `enum ItemSource { Pool(Arc<ItemPool>), Detached(Arc<Vec<Arc<dyn SkimItem>>>) }`
  with `get(index)` and `with_items(..)`. The default is an empty `Detached`.
- `ItemList`: `items: Vec<MatchEntry>`, `all: Option<AllItems>`, and the new
  field `source: ItemSource`.
- `AllItems`: only `{ len, descending }`. It resolves through `source`, with
  score 0. Remove `pool` and `rank_builder`.
- `listed_get` builds `MatchedItem::new(source.get(e.index)?, e.index as usize, e.score)`.
- `ProcessedItems`: `items: Vec<MatchEntry>`, plus a new field `pool: Arc<ItemPool>`.
  When the UI accepts a result, it sets `source = ItemSource::Pool(pool)`.
- `into_items()` becomes `resolve_all(&self) -> Vec<MatchedItem>` and uses `with_items`.
- `detach_from_pool(&mut self, old: Vec<Arc<dyn SkimItem>>)` becomes O(1):
  `source = Detached(Arc::new(old))`. With this, it does not build items
  for the empty query any more.
- `append(&mut self, &mut Vec<MatchEntry>)`.
- The highlights map key is `MatchedItem::index`.
- Add the test helper
  `#[cfg(test)] fn from_texts(texts: &[&str]) -> ItemList`. It makes a pool
  and entries, for the many tests that set `items` directly.

### 1.3 `src/matcher.rs`

- The workers push `MatchEntry::new(&rank, &rank_builder)` and do not call
  `Arc::clone(item)`. All the result `Vec`s become `Vec<MatchEntry>`.
- `merge_worker_results` gets `pool: Arc<ItemPool>` and puts it into `ProcessedItems`.

### 1.4 Other call sites

| File | Change |
|---|---|
| `src/tui/app.rs` `AppendAndSelect` | append an entry to the list; insert `MatchedItem::new(item, index, 0)` into the selection |
| `src/tui/app.rs` `results()` | filter mode: `item_list.resolve_all()` and filter out disabled items |
| `src/tui/app.rs` `ClearItems` | `let old = self.item_pool.clear(); self.item_list.detach_from_pool(old);` |
| `src/tui/app.rs` preview ctx (~l.620) | `i.index` instead of `rank.index.unsigned_abs() as usize` |
| `src/skim.rs` `handle_reload` | `let old = item_pool.clear();` detach only when the old display stays, else drop `old` |
| `src/skim.rs` filter mode (~l.595–670) | keep the entries and the pool in the list; `results()` resolves them |
| `src/util.rs` `printf` | `i.index` |
| `src/output.rs` `--print-score` | `item.score` |
| `src/tui/item_renderer.rs` | `item.score`, `item.index` |
| `src/popup/mod.rs` | `MatchedItem::new(item, 0, score)` |
| `examples/base.rs` | `item.index` |
| `BREAKING_CHANGES.md` | new section: `MatchedItem` fields, `MatchedItem::new`, `MatchEntry`, `ItemPool::clear` return value; migration to `engine.match_rank(item)` for the full rank |

Tests that you must port: `src/item_tests.rs`, `src/tui/item_list_tests.rs`,
`src/tui/app_tests.rs`, `src/skim_tests.rs`, `src/util_tests.rs`,
`src/tui/item_renderer_tests.rs`, and the tests in `src/output.rs` and `src/matcher.rs`.

### 1.5 New tests

- `item_tests`: `sorted_merge_equals_concat_sort` and
  `merge_into_sorted_equals_concat_sort`. Use a seeded `rand` (already a dev
  dependency), side sizes {0, 1, 255, 256, 257, 4096}, and both fast paths
  (all less, all greater). This covers the rewrite of the unsafe merge.
- `item_list_tests`: `get_resolves_entry_through_pool` (text, index, score).
  `detach_keeps_old_items_after_pool_clear` (port
  `detach_from_pool_survives_pool_clear`; also check that items appended to the
  new pool do not show in the detached list).
- `matcher` tests: `incremental_runs_equal_one_full_run` for {sort, no-sort,
  tac, no-sort + tac}. Append 3 batches, run 3 incremental matches, apply them
  through `ItemList::render`, and compare the order of the texts with one full
  run. This test protects the tac index mapping (`input_index`).

### 1.6 Checks

```sh
cargo +nightly fmt --check && cargo clippy --all-targets
INSTA_UPDATE=no cargo nextest run                # no snapshot changes are allowed
cargo nextest run --profile valgrind             # the unsafe merge is removed
cargo build --examples
cargo bench --bench merge          -- --baseline before   # target: >= 1.8x faster at 3M+64K
cargo bench --bench partial        -- ui_render --baseline before   # target: >= 1.5x faster at 1M
cargo bench --bench read_and_match -- --baseline before   # target: no regression > 5%
cargo bench --bench filter         -- --baseline before   # target: no regression > 5%
command time -v target/release/sk --filter a < benches/fixtures/10M.txt > /dev/null
#   target: lower max RSS than /tmp/rss-before.txt (expect about 32 B less per match)
```

Expected effect: the merge moves half the bytes and does no refcount work. On a
query change, the UI drops the old list with one `free` instead of N `Arc`
decrements. The workers do not touch the refcount of each item. The UI merge
is still O(N), so phase 2 is necessary.

## Phase 2: the matcher builds the full list, the UI swaps (commit 3)

### 2.1 Design

- `Matcher` gets `latest: Arc<Mutex<Option<(usize, Arc<Vec<MatchEntry>>)>>>`:
  the last full list and its generation. `run()` gives a clone to the coordinator.
- `ProcessedItems.items: Arc<Vec<MatchEntry>>` is always the **full** list for
  its generation. Add `prepended: usize` (only for tac + no-sort). Remove the
  `merge` field. Move `MergeStrategy` to `matcher.rs` (it is a matcher input only).
- `merge_worker_results`:
  1. Check the generation, flatten and sort the batch (as today).
  2. `base` = `latest` if it has the same generation and the strategy is not
     `Replace`, else empty.
  3. Build `full` **with no lock held**: `MatchEntry::merge_sorted(&base, &batch) -> Vec`,
     or `base ++ batch` (Append), or `batch ++ base` (Prepend, `prepended += batch.len()`).
  4. Lock and check the generation again. Set `latest = (gen, full.clone())`. Set
     `pending = Some(ProcessedItems { items: full, prepended, .. })`. If an
     earlier pending result was not taken, add its `prepended` and drop it. The
     coordinator frees it with one `free`.
  5. Set `needs_render`.
- `ItemList::render`: `this.items = processed.items` (pointer swap), then
  `keep_focus_after_prepend(processed.prepended)`. The UI never merges.
- `ItemList.items: Arc<Vec<MatchEntry>>`. `append` (`AppendAndSelect`) uses
  `Arc::make_mut`. This is an O(N) copy, but only on that rare action.
- Delete `sorted_merge`, `merge_into_sorted` and `merge_backwards`. Only
  `merge_sorted(&[MatchEntry], &[MatchEntry]) -> Vec<MatchEntry>` stays. Port
  the phase 1.5 merge tests to it.

Memory: the UI and the matcher share one N × 32 B list. During a merge there
is one more N × 32 B. Today's peak is 2N × 64 B.

### 2.2 New tests

- `matcher`: `incremental_results_are_full_lists`: after each incremental run,
  `pending.items` has all the matches so far and equals one full run.
- `matcher`: `replace_ignores_latest_of_old_generation`, plus a port of the
  stale-generation tests.
- `item_list_tests`: `render_swaps_full_list` (`Arc::ptr_eq` before and after
  the render). Port the Prepend focus tests to `prepended`.

### 2.3 Checks

Run all the phase 1.6 checks, and also:

```sh
cargo bench --bench partial -- ui_render --baseline before
#   target: < 2 ms, and 100K and 1M give about the same time (no O(N) work on the UI)
python3 benches/ui_latency.py --bin target/release/sk --query a
#   target: e2e p90 < 50 ms (before: about 200 ms), peak_rss below /tmp/lat-before-a.txt
python3 benches/ui_latency.py --bin target/release/sk --query ''
#   target: no regression > 10% against /tmp/lat-before-empty.txt
# ThreadSanitizer: the threading changed
RUSTFLAGS="-Zsanitizer=thread" cargo +nightly build --tests -Zbuild-std --target x86_64-unknown-linux-gnu
TSAN_OPTIONS="detect_deadlocks=1" cargo +nightly nextest run --profile tsan --target x86_64-unknown-linux-gnu
```

## Phase 3: geometric batching (optional, commit 4)

Gate: do this only if `perf record` during `ui_latency.py --query a` shows the
coordinator busy for more than 50% of one core merging, or the matched count
in the status line lags the read count by more than about 1 s.

- In `App::restart_matcher(false)`, start an incremental run only if
  `num_not_taken >= max(4096, item_list.len() / 8)`, or
  `last_matcher_restart.elapsed() >= 100 ms`, or the reader is done. The total
  merge work then becomes O(N) amortized instead of O(N × batches). New items
  show after at most 100 ms.
- Check that the filter-mode loop in `skim.rs` and `check_reader` still empty
  the pool when the reader is done.
- Test (`app_tests`): if few items are pending and the last restart was recent,
  no run starts. When the reader is done, a run starts.

## Phase 4: follow-ups, each gated by a measurement

| Follow-up | Do it when |
|---|---|
| Segmented `ItemPool` (fixed-size chunks, so growth never moves items while the lock is held) | `perf` shows `SpinLock::lock` above 5% of the UI-thread samples, or `ui_latency.py --query ''` render max > 50 ms |
| Drop the old pool `Vec` from `ItemPool::clear` on a separate thread | the time of a reload with millions of items shows in the harness |
| Coalesce `Render` events (do not queue a `Render` when one is queued) | `handled` p90 > 2 × `render` p90 in the harness |
| LSM runs with a lazy merge (option 2b) | a coordinator merge at 10M matches takes > 100 ms |

## Risks

- `ItemList::get` takes the pool spinlock for each visible row. If the reader
  holds the lock while the pool `Vec` grows, the UI waits (see phase 4).
- `Detached` keeps the old pool in memory while the stale list shows (with
  `--no-clear-if-empty` / `--no-clear-start`). Today it builds a copy instead.
- Library users who read `MatchedItem::rank` or call `MatchedItem::new` must
  migrate (D1).
- Index limit: `i32::MAX` items, as today (D4).
