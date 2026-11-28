//! This module contains the matching coordinator
use crate::thread_pool::{self, ThreadPool};
use crate::tui::item_list::{AllItems, ProcessedItems};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::engine::normalized::NormalizedEngineFactory;
use crate::engine::split::SplitMatchEngineFactory;
use crate::item::{ItemPool, MatchEntry, RankBuilder};
use crate::prelude::{AndOrEngineFactory, ExactOrFuzzyEngineFactory, RegexEngineFactory};
use crate::{CaseMatching, MatchEngine, MatchEngineFactory, SkimItem, SkimOptions};

/// How a batch joins the previous results for the same query.
#[derive(Clone, Copy)]
pub(crate) enum MergeStrategy {
    Replace,
    SortedMerge,
    Append,
    Prepend,
}

type LatestMatches = Mutex<Option<(usize, Arc<Vec<MatchEntry>>)>>;

fn input_index(tac: bool, start: usize, batch_len: usize, batch_index: usize) -> usize {
    debug_assert!(batch_index < batch_len);
    if tac {
        start + batch_len - 1 - batch_index
    } else {
        start + batch_index
    }
}

#[allow(clippy::too_many_arguments)]
fn merge_worker_results(
    worker_results: Vec<Vec<MatchEntry>>,
    all: Option<AllItems>,
    pool: Arc<ItemPool>,
    engine: Arc<dyn MatchEngine>,
    no_sort: bool,
    latest: &LatestMatches,
    processed_items: &Mutex<Option<ProcessedItems>>,
    merge_strategy: MergeStrategy,
    generation: usize,
    current_generation: &AtomicUsize,
    needs_render: &AtomicBool,
) {
    if current_generation.load(Ordering::Acquire) != generation {
        return;
    }

    let mut batch = Vec::with_capacity(worker_results.iter().map(Vec::len).sum());
    for chunk in worker_results {
        batch.extend(chunk);
    }
    if !no_sort {
        // Stable sort detects the sorted runs produced by the workers.
        batch.sort();
    }
    let base = if matches!(merge_strategy, MergeStrategy::Replace) {
        None
    } else {
        latest
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .filter(|(batch_generation, _)| *batch_generation == generation)
            .map(|(_, items)| Arc::clone(items))
    };
    let mut prepended = if matches!(merge_strategy, MergeStrategy::Prepend) {
        batch.len()
    } else {
        0
    };
    let items = match base {
        Some(base) if batch.is_empty() => base,
        Some(base) if !base.is_empty() => Arc::new(match merge_strategy {
            MergeStrategy::SortedMerge => MatchEntry::merge_sorted(&base, &batch),
            MergeStrategy::Append => [base.as_slice(), &batch].concat(),
            MergeStrategy::Prepend => [batch.as_slice(), &base].concat(),
            MergeStrategy::Replace => unreachable!(),
        }),
        _ => Arc::new(batch),
    };

    trace!(
        "matcher stop, total matched: {}",
        all.map_or(items.len(), |all| all.len)
    );

    // No list is built with either lock held. Validate again before publishing.
    let mut latest = latest.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut pending = processed_items
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if current_generation.load(Ordering::Acquire) != generation {
        return;
    }
    if !matches!(merge_strategy, MergeStrategy::Replace)
        && let Some(previous) = pending.as_ref().filter(|previous| previous.generation == generation)
    {
        prepended += previous.prepended;
    }
    let old_latest = latest.replace((generation, Arc::clone(&items)));
    let old_pending = pending.replace(ProcessedItems {
        items,
        all,
        pool,
        prepended,
        generation,
        engine,
    });
    drop(pending);
    drop(latest);
    needs_render.store(true, Ordering::Relaxed);
    drop((old_latest, old_pending));
}

//==============================================================================
/// Control handle for a running matcher operation.
///
/// Provides methods to check status, retrieve results, and stop the matcher.
pub struct MatcherControl {
    stopped: Arc<AtomicBool>,
    interrupt: Arc<AtomicBool>,
    processed: Arc<AtomicUsize>,
    matched: Arc<AtomicUsize>,
}

impl Default for MatcherControl {
    fn default() -> Self {
        Self {
            stopped: Arc::new(AtomicBool::new(true)),
            interrupt: Arc::new(AtomicBool::new(false)),
            processed: Default::default(),
            matched: Default::default(),
        }
    }
}

impl MatcherControl {
    /// Returns the number of items that have been processed so far.
    #[must_use]
    pub fn get_num_processed(&self) -> usize {
        self.processed.load(Ordering::Relaxed)
    }

    /// Returns the number of items that have matched so far.
    #[must_use]
    pub fn get_num_matched(&self) -> usize {
        self.matched.load(Ordering::Relaxed)
    }

    /// Signals the matcher to stop processing.
    pub fn kill(&mut self) {
        self.interrupt.store(true, Ordering::Relaxed);
    }

    /// Returns true if the matcher has stopped (either completed or killed).
    #[must_use]
    pub fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }
}

impl Drop for MatcherControl {
    fn drop(&mut self) {
        self.kill();
    }
}

//==============================================================================
/// The main matcher that coordinates fuzzy/exact matching of items against a query.
pub struct Matcher {
    engine_factory: Rc<dyn MatchEngineFactory>,
    case_matching: CaseMatching,
    /// The rank builder used by the engines and to build match entry sort keys.
    pub rank_builder: Arc<RankBuilder>,
    latest: Arc<LatestMatches>,
    /// The engine factory matches every item for the empty query, with ranks
    /// that only differ by index: the list can show the pool as is.
    empty_query_matches_all: bool,
}

impl Matcher {
    /// Creates a new Matcher builder with the given engine factory.
    pub fn builder(engine_factory: Rc<dyn MatchEngineFactory>) -> Self {
        Self {
            engine_factory,
            case_matching: CaseMatching::default(),
            rank_builder: Arc::new(RankBuilder::default()),
            latest: Arc::default(),
            empty_query_matches_all: false,
        }
    }

    /// Sets the case matching mode (smart, ignore, or respect).
    #[must_use]
    pub fn case(mut self, case_matching: CaseMatching) -> Self {
        self.case_matching = case_matching;
        self
    }

    /// Sets the rank builder (carries tiebreak criteria).
    #[must_use]
    pub fn rank_builder(mut self, rank_builder: Arc<RankBuilder>) -> Self {
        self.rank_builder = rank_builder;
        self
    }

    /// Finalizes the builder and returns the configured Matcher.
    #[must_use]
    pub fn build(self) -> Self {
        self
    }

    /// Creates a `MatchEngineFactory` from the given options.
    ///
    /// This is useful when you need the factory directly (e.g., for filter mode)
    /// without creating a full Matcher instance.
    #[must_use]
    pub fn create_engine_factory(options: &SkimOptions) -> Rc<dyn MatchEngineFactory> {
        Self::create_engine_factory_with_builder(options).0
    }

    /// Creates a `MatchEngineFactory` and the associated `RankBuilder` from the given options.
    ///
    /// Use the builder to construct sorted match entries from ranks.
    #[must_use]
    pub fn create_engine_factory_with_builder(options: &SkimOptions) -> (Rc<dyn MatchEngineFactory>, Arc<RankBuilder>) {
        if options.regex {
            let rank_builder = Arc::new(RankBuilder::new(options.tiebreak.clone()).tac(options.tac));
            let regex_factory = RegexEngineFactory::builder().rank_builder(rank_builder.clone());
            let factory: Rc<dyn MatchEngineFactory> = if options.normalize {
                Rc::new(NormalizedEngineFactory::new(regex_factory))
            } else {
                Rc::new(regex_factory)
            };
            (factory, rank_builder)
        } else {
            let rank_builder = Arc::new(RankBuilder::new(options.tiebreak.clone()).tac(options.tac));
            log::debug!("Creating matcher for algo {:?}", options.algorithm);
            let fuzzy_engine_factory = ExactOrFuzzyEngineFactory::builder()
                .fuzzy_algorithm(options.algorithm)
                .exact_mode(options.exact)
                .typos(options.typos)
                .filter_mode(options.filter.is_some())
                .last_match(options.last_match)
                .rank_builder(rank_builder.clone())
                .build();

            let mut factory: Box<dyn MatchEngineFactory> = Box::new(fuzzy_engine_factory);

            // If split_match is enabled, wrap the fuzzy factory with SplitMatchEngineFactory
            if let Some(delimiter) = options.split_match {
                factory = Box::new(SplitMatchEngineFactory::new(factory, delimiter));
            }

            // Wrap with AndOrEngineFactory so that queries like "foo:bar baz:qux" work
            factory = Box::new(AndOrEngineFactory::new(factory));

            // Wrap with NormalizedEngineFactory if normalization is requested
            if options.normalize {
                factory = Box::new(NormalizedEngineFactory::new(factory));
            }

            let factory: Rc<dyn MatchEngineFactory> = Rc::new(factory);
            (factory, rank_builder)
        }
    }

    /// Creates a Matcher configured from the given `SkimOptions`.
    #[must_use]
    pub fn from_options(options: &SkimOptions) -> Self {
        let (engine_factory, rank_builder) = Self::create_engine_factory_with_builder(options);
        Matcher {
            // Skim's factories use `MatchAllEngine` for it; the regex one does not.
            empty_query_matches_all: !options.regex,
            ..Matcher::builder(engine_factory)
                .case(options.case)
                .rank_builder(rank_builder)
                .build()
        }
    }

    /// Returns the case matching setting for this matcher.
    #[must_use]
    pub fn case_matching(&self) -> CaseMatching {
        self.case_matching
    }

    /// Returns a reference to the engine factory.
    #[must_use]
    pub fn engine_factory(&self) -> &Rc<dyn MatchEngineFactory> {
        &self.engine_factory
    }

    /// Whether the results of `query` are all the pool items in index order:
    /// `Some(descending)`, or `None` if they must be matched and sorted. Then
    /// the matcher only counts the items and the list shows the pool as is.
    fn lists_pool(&self, query: &str, no_sort: bool, tac: bool) -> Option<bool> {
        if !self.empty_query_matches_all || !query.is_empty() {
            None
        } else if no_sort {
            Some(tac)
        } else {
            self.rank_builder.empty_query_order()
        }
    }

    pub(crate) fn clear_results(&self) {
        let old = self
            .latest
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        drop(old);
    }

    /// Runs the matcher on items from the pool in a background thread.
    ///
    /// When matching completes, the coordinator publishes the full result list,
    /// then signals
    /// `needs_render` so the UI picks up the new data on its next tick.
    ///
    /// Returns a `MatcherControl` that can be used to monitor progress or
    /// stop the matcher.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(crate) fn run(
        &self,
        query: &str,
        item_pool: &Arc<ItemPool>,
        thread_pool: &Arc<ThreadPool>,
        processed_items: Arc<Mutex<Option<ProcessedItems>>>,
        merge_strategy: MergeStrategy,
        no_sort: bool,
        tac: bool,
        generation: usize,
        current_generation: Arc<AtomicUsize>,
        needs_render: Arc<AtomicBool>,
    ) -> MatcherControl {
        let matcher_engine = self.engine_factory.create_engine_with_case(query, self.case_matching);
        debug!("engine: {matcher_engine}");
        let stopped = Arc::new(AtomicBool::new(false));
        let stopped_clone = stopped.clone();
        let interrupt = Arc::new(AtomicBool::new(false));
        let interrupt_clone = interrupt.clone();
        let processed = Arc::new(AtomicUsize::new(0));
        let processed_clone = processed.clone();
        let matched = Arc::new(AtomicUsize::new(0));
        let matched_clone = matched.clone();
        let rank_builder = self.rank_builder.clone();
        if matches!(merge_strategy, MergeStrategy::Replace) {
            self.clear_results();
        }
        let latest = Arc::clone(&self.latest);

        // Take items synchronously before spawning to avoid a race condition:
        // if we took items inside the spawned closure, a subsequent restart_matcher()
        // could call kill() + reset() before the old closure runs, causing the old
        // closure to re-take items that should belong to the new matcher.
        let start = item_pool.num_taken();
        let items = item_pool.take();
        let total = items.len();
        trace!("matcher start, total: {total}");

        let all_descending = self.lists_pool(query, no_sort, tac);
        let all = all_descending.map(|descending| AllItems {
            len: start + total,
            descending,
        });
        let pool = Arc::clone(item_pool);

        // The coordinator runs on a dedicated OS thread so it does not occupy
        // a pool slot while waiting for workers.  All pool threads are
        // therefore available for the parallel matching work.
        let num_workers = thread_pool.num_threads();
        let pool_for_work = Arc::clone(thread_pool);

        std::thread::spawn(move || {
            // Each worker keeps taking chunks from the shared queue until it is empty.
            const CHUNK_SIZE: usize = 1 << 12;

            // Share the batch with the workers. `Arc<Vec>`, not `Arc<[_]>`: that
            // conversion would copy the whole batch.
            let shared_items: Arc<Vec<Arc<dyn SkimItem>>> = Arc::new(items);

            // Clones for the process_chunk closure.
            let matcher_engine: Arc<dyn MatchEngine> = Arc::from(matcher_engine);
            let engine_for_merge = Arc::clone(&matcher_engine);
            let interrupt_for_work = Arc::clone(&interrupt);
            let processed_for_work = Arc::clone(&processed);
            let matched_for_work = Arc::clone(&matched);

            thread_pool::parallel_work_queue(
                &pool_for_work,
                num_workers,
                &shared_items,
                CHUNK_SIZE,
                no_sort,
                // identity – seed value for each worker's local accumulator
                Vec::<MatchEntry>::new,
                // process_chunk – called for each chunk; returns a Vec of matches
                move |chunk_start, chunk: &[Arc<dyn crate::SkimItem>]| {
                    // Check interrupt before processing this chunk.
                    if interrupt_for_work.load(Ordering::Relaxed) {
                        return Vec::new();
                    }

                    let mut local_matches = Vec::new();
                    let chunk_matched = if all_descending.is_some() {
                        chunk.len() // empty query: every item matches, only count them
                    } else {
                        for (i, item) in chunk.iter().enumerate() {
                            // Only rank here; the UI computes highlights for the displayed items.
                            if let Some(mut rank) = matcher_engine.match_rank(item.as_ref()) {
                                let batch_index = chunk_start + i;
                                // `take()` reverses each tac batch, so recover the
                                // item's stable ordinal in the original input stream.
                                let index = input_index(tac, start, total, batch_index);
                                rank.index = i32::try_from(index).expect("input index fits in i32");
                                local_matches.push(MatchEntry::new(&rank, &rank_builder));
                            }
                        }
                        local_matches.len()
                    };

                    // Flush counters for this chunk so the UI sees progress.
                    processed_for_work.fetch_add(chunk.len(), Ordering::Relaxed);
                    if chunk_matched > 0 {
                        matched_for_work.fetch_add(chunk_matched, Ordering::Relaxed);
                    }

                    local_matches
                },
                // reduce – accumulate chunk matches into the worker-local Vec.
                // No sorting here — that would be O(m²/chunk_size) per worker.
                |acc: &mut Vec<MatchEntry>, mut partial: Vec<MatchEntry>| {
                    if acc.len() >= partial.len() {
                        acc.extend(partial);
                    } else {
                        partial.append(acc);
                        *acc = partial;
                    }
                },
                // prepare – sort each worker's accumulator **on the worker
                // thread** so that sorting runs in parallel across all workers.
                // A single O((m/k)·log(m/k)) sort per worker is far cheaper
                // than sorting during reduce.
                // sort_unstable is used here because the worker's accumulator
                // has no pre-existing sorted runs (items were appended in
                // chunk order), so driftsort's run-detection overhead is pure
                // cost.  The final merge uses sort() so that driftsort can
                // exploit the k sorted runs produced by the workers.
                |acc: &mut Vec<MatchEntry>| acc.sort_unstable(),
                |worker_results: Vec<Vec<MatchEntry>>| {
                    if interrupt.load(Ordering::SeqCst) {
                        return;
                    }

                    merge_worker_results(
                        worker_results,
                        all,
                        pool,
                        engine_for_merge,
                        no_sort,
                        &latest,
                        &processed_items,
                        merge_strategy,
                        generation,
                        &current_generation,
                        &needs_render,
                    );
                },
            );
            // A caller that sees a stopped matcher must also see its final counts.
            stopped.store(true, Ordering::Release);
        });

        MatcherControl {
            stopped: stopped_clone,
            interrupt: interrupt_clone,
            matched: matched_clone,
            processed: processed_clone,
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    use super::*;
    use crate::Rank;
    use crate::item::RankBuilder;
    use crate::options::SkimOptionsBuilder;

    #[test]
    fn from_options_exposes_case_and_factory() {
        let options = SkimOptionsBuilder::default()
            .case(CaseMatching::Ignore)
            .build()
            .unwrap();
        let matcher = Matcher::from_options(&options);
        assert_eq!(matcher.case_matching(), CaseMatching::Ignore);
        // The factory builds a working engine.
        let engine = matcher.engine_factory().create_engine("foo");
        assert!(engine.match_item(&"foobar".to_string()).is_some());
    }

    #[test]
    fn create_engine_factory_builds_fuzzy_engine() {
        let options = SkimOptions::default();
        let factory = Matcher::create_engine_factory(&options);
        let engine = factory.create_engine("fb");
        assert!(engine.match_item(&"foobar".to_string()).is_some());
    }

    #[test]
    fn create_engine_factory_regex_with_normalize() {
        let options = SkimOptionsBuilder::default()
            .regex(true)
            .normalize(true)
            .build()
            .unwrap();
        let (factory, _rank) = Matcher::create_engine_factory_with_builder(&options);
        let engine = factory.create_engine("ba.");
        assert!(engine.match_item(&"foobar".to_string()).is_some());
    }

    #[test]
    fn regex_factory_uses_configured_tiebreak() {
        let options = SkimOptionsBuilder::default()
            .regex(true)
            .tiebreak(vec![crate::RankCriteria::Length])
            .build()
            .unwrap();
        let (factory, rank_builder) = Matcher::create_engine_factory_with_builder(&options);
        let engine = factory.create_engine("a");
        let input = ["aaaa", "a", "aaa"];
        let mut matches: Vec<_> = input
            .into_iter()
            .enumerate()
            .map(|(index, text)| {
                let mut rank = engine.match_rank(&text.to_string()).unwrap();
                rank.index = i32::try_from(index).unwrap();
                MatchEntry::new(&rank, &rank_builder)
            })
            .collect();
        matches.sort();

        let output: Vec<_> = matches.iter().map(|item| input[item.index as usize]).collect();
        assert_eq!(output, ["a", "aaa", "aaaa"]);
    }

    fn entry(index: i32) -> MatchEntry {
        MatchEntry::new(
            &Rank {
                index,
                ..Rank::default()
            },
            &RankBuilder::default(),
        )
    }

    #[test]
    fn stale_generation_cannot_publish_results() {
        let processed = Mutex::new(None);
        let latest = LatestMatches::default();
        let needs_render = AtomicBool::new(false);
        merge_worker_results(
            vec![vec![entry(0)]],
            None,
            Arc::default(),
            Arc::new(crate::engine::all::MatchAllEngine::builder().build()),
            false,
            &latest,
            &processed,
            MergeStrategy::Replace,
            1,
            &AtomicUsize::new(2),
            &needs_render,
        );
        assert!(processed.lock().unwrap().is_none());
        assert!(latest.lock().unwrap().is_none());
        assert!(!needs_render.load(Ordering::Relaxed));
    }

    #[test]
    fn replace_ignores_latest_of_old_generation() {
        for strategy in [MergeStrategy::Replace, MergeStrategy::SortedMerge] {
            let old = Arc::new(vec![entry(0)]);
            let pool = Arc::new(ItemPool::default());
            let engine = Arc::new(crate::engine::all::MatchAllEngine::builder().build());
            let processed = Mutex::new(Some(ProcessedItems {
                items: Arc::clone(&old),
                prepended: 3,
                generation: 0,
                all: None,
                pool: Arc::clone(&pool),
                engine: engine.clone(),
            }));
            let latest = LatestMatches::new(Some((0, old)));
            let needs_render = AtomicBool::new(false);
            merge_worker_results(
                vec![vec![entry(2)], vec![entry(1)]],
                None,
                pool,
                engine,
                false,
                &latest,
                &processed,
                strategy,
                1,
                &AtomicUsize::new(1),
                &needs_render,
            );
            let result = processed.into_inner().unwrap().unwrap();
            assert_eq!(result.generation, 1);
            assert_eq!(result.prepended, 0);
            assert_eq!(*result.items, [entry(1), entry(2)]);
            assert!(Arc::ptr_eq(&result.items, &latest.into_inner().unwrap().unwrap().1));
            assert!(needs_render.load(Ordering::Relaxed));
        }
    }

    #[test]
    fn incremental_runs_publish_full_lists_equal_to_one_full_run() {
        use crate::theme::ColorTheme;
        use crate::tui::App;
        use crate::tui::widget::SkimWidget;
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use std::time::{Duration, Instant};

        let batches = [["ba", "skip", "a"], ["ab", "cdaef", "no"], ["abc", "xyza", "a"]];
        let run = |app: &mut App, force| {
            app.restart_matcher(force);
            let start = Instant::now();
            while !app.matcher_control.stopped() {
                assert!(start.elapsed() < Duration::from_secs(5), "matcher did not stop");
                std::thread::yield_now();
            }
        };
        let append = |app: &App, batch: &[&str]| {
            app.item_pool.append(
                batch
                    .iter()
                    .map(|&text| Arc::new(text.to_owned()) as Arc<dyn SkimItem>)
                    .collect(),
            )
        };
        let area = Rect::new(0, 0, 20, 5);
        for (no_sort, tac) in [(false, false), (true, false), (false, true), (true, true)] {
            for render_each_batch in [false, true] {
                let app = || {
                    let options = SkimOptionsBuilder::default()
                        .no_sort(no_sort)
                        .tac(tac)
                        .query("a")
                        .build()
                        .unwrap();
                    App::from_options(options, Arc::new(ColorTheme::default()), String::new())
                };
                let mut incremental = app();
                let mut full = app();
                let mut prepended = 0;
                for (i, batch) in batches.iter().enumerate() {
                    append(&incremental, batch);
                    append(&full, batch);
                    run(&mut incremental, i == 0);
                    run(&mut full, true);
                    let expected = full.item_list.take_processed().unwrap();
                    let pending = incremental.item_list.processed_items.lock().unwrap();
                    let actual = pending.as_ref().unwrap();
                    assert_eq!(actual.items, expected.items, "no_sort={no_sort}, tac={tac}, batch={i}");
                    if no_sort && tac {
                        if i > 0 {
                            prepended += incremental.matcher_control.get_num_matched();
                        }
                        assert_eq!(actual.prepended, prepended);
                    }
                    drop(pending);
                    if render_each_batch || i == batches.len() - 1 {
                        prepended = 0;
                        incremental.item_list.render(area, &mut Buffer::empty(area));
                        full.item_list.apply_processed(expected);
                        let texts = |app: &App| {
                            app.item_list
                                .resolve_all()
                                .into_iter()
                                .map(|item| item.text().into_owned())
                                .collect::<Vec<_>>()
                        };
                        assert_eq!(texts(&incremental), texts(&full));
                    }
                }
            }
        }
    }

    #[test]
    fn tac_input_index_spans_incremental_batches() {
        let first: Vec<_> = (0..3).map(|index| input_index(true, 0, 3, index)).collect();
        let second: Vec<_> = (0..2).map(|index| input_index(true, 3, 2, index)).collect();
        assert_eq!(first, [2, 1, 0]);
        assert_eq!(second, [4, 3]);
        assert_eq!(input_index(false, 3, 2, 0), 3);
        assert_eq!(input_index(false, 3, 2, 1), 4);
    }
}
