//! This module contains the matching coordinator
use crate::thread_pool::{self, ThreadPool};
use crate::tui::item_list::{MergeStrategy, ProcessedItems};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::engine::normalized::NormalizedEngineFactory;
use crate::engine::split::SplitMatchEngineFactory;
use crate::item::{ItemPool, MatchedItem, RankBuilder};
use crate::prelude::{AndOrEngineFactory, ExactOrFuzzyEngineFactory, RegexEngineFactory};
use crate::spinlock::SpinLock;
use crate::{CaseMatching, MatchEngineFactory, SkimItem, SkimOptions};

/// Merges per-worker match results and writes them into `processed_items`.
///
/// When `no_sort` is false, concatenates the pre-sorted worker results into a
/// single contiguous `Vec` and calls `sort()`.  Rust's stable sort (driftsort
/// since 1.81, a `TimSort` variant before that) detects the k pre-sorted runs
/// and merges them in O(n log k) on contiguous memory — benchmarking shows
/// this consistently outperforms tree-based or fold-based merge strategies
/// due to driftsort's cache-friendly single-buffer merge passes.
///
/// When `no_sort` is true, the worker results are simply flattened.
///
/// Signals `needs_render` after writing so the UI picks up the new data.
fn merge_worker_results(
    worker_results: Vec<Vec<MatchedItem>>,
    no_sort: bool,
    processed_items: &SpinLock<Option<ProcessedItems>>,
    merge_strategy: MergeStrategy,
    needs_render: &AtomicBool,
) {
    let total_len: usize = worker_results.iter().map(Vec::len).sum();
    let mut items = Vec::with_capacity(total_len);
    for chunk in worker_results {
        items.extend(chunk);
    }

    // Each worker's sub-list is already sorted by `prepare`, so the
    // concatenated Vec consists of k sorted runs.  Rust's stable sort
    // (driftsort since 1.81, a TimSort variant before that) detects
    // pre-existing runs and merges them in O(n log k) for k workers,
    // all on contiguous memory with a single auxiliary buffer.
    if !no_sort {
        items.sort();
    }

    trace!("matcher stop, total matched: {}", items.len());

    // Single lock, single write into processed_items.
    let mut guard = processed_items.lock();
    if matches!(merge_strategy, MergeStrategy::Replace) {
        *guard = Some(ProcessedItems {
            items,
            merge: MergeStrategy::Replace,
        });
        drop(guard);
        needs_render.store(true, Ordering::Relaxed);
        return;
    }
    match &mut *guard {
        Some(existing) => {
            if no_sort {
                existing.items.extend(items);
            } else {
                // Both sides are fully sorted — one O(n+m) merge.
                MatchedItem::merge_into_sorted(&mut existing.items, items);
            }
        }
        None => {
            *guard = Some(ProcessedItems {
                items,
                merge: merge_strategy,
            });
        }
    }
    // Guard is dropped here, releasing the lock before we signal the render flag.

    needs_render.store(true, Ordering::Relaxed);
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
        self.stopped.load(Ordering::Relaxed)
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
    /// The rank builder shared with all engines; used to attach criteria to `MatchedItem`s.
    pub rank_builder: Arc<RankBuilder>,
}

impl Matcher {
    /// Creates a new Matcher builder with the given engine factory.
    pub fn builder(engine_factory: Rc<dyn MatchEngineFactory>) -> Self {
        Self {
            engine_factory,
            case_matching: CaseMatching::default(),
            rank_builder: Arc::new(RankBuilder::default()),
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
    /// Returns both so callers can attach the builder to `MatchedItem`s for lazy sort-key
    /// computation.
    #[must_use]
    pub fn create_engine_factory_with_builder(options: &SkimOptions) -> (Rc<dyn MatchEngineFactory>, Arc<RankBuilder>) {
        if options.regex {
            let regex_factory = RegexEngineFactory::builder();
            let factory: Rc<dyn MatchEngineFactory> = if options.normalize {
                Rc::new(NormalizedEngineFactory::new(regex_factory))
            } else {
                Rc::new(regex_factory)
            };
            (factory, Arc::new(RankBuilder::default()))
        } else {
            let rank_builder = Arc::new(RankBuilder::new(options.tiebreak.clone()));
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
        Matcher::builder(engine_factory)
            .case(options.case)
            .rank_builder(rank_builder)
            .build()
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

    /// Runs the matcher on items from the pool in a background thread.
    ///
    /// When matching completes, the coordinator merges results directly into
    /// `processed_items` according to `merge_strategy`, then signals
    /// `needs_render` so the UI picks up the new data on its next tick.
    ///
    /// Returns a `MatcherControl` that can be used to monitor progress or
    /// stop the matcher.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run(
        &self,
        query: &str,
        item_pool: &Arc<ItemPool>,
        thread_pool: &Arc<ThreadPool>,
        processed_items: Arc<SpinLock<Option<ProcessedItems>>>,
        merge_strategy: MergeStrategy,
        no_sort: bool,
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

        // Take items synchronously before spawning to avoid a race condition:
        // if we took items inside the spawned closure, a subsequent restart_matcher()
        // could call kill() + reset() before the old closure runs, causing the old
        // closure to re-take items that should belong to the new matcher.
        let start = item_pool.num_taken();
        let items = item_pool.take();
        let total = items.len();
        trace!("matcher start, total: {total}");

        // The coordinator runs on a dedicated OS thread so it does not occupy
        // a pool slot while waiting for workers.  All pool threads are
        // therefore available for the parallel matching work.
        let num_workers = thread_pool.num_threads();
        let pool_for_work = Arc::clone(thread_pool);

        std::thread::spawn(move || {
            // Process items in parallel using a shared work queue.  Each worker
            // thread atomically grabs the next available chunk, processes it,
            // and immediately merges its partial results.  This means threads
            // that finish early automatically pick up more work, providing
            // natural load balancing.
            //
            // The chunk size controls the granularity of work distribution and
            // the frequency of atomic counter updates / interrupt checks.
            const CHUNK_SIZE: usize = 1 << 20;

            // Convert items into an Arc slice so all workers can share them.
            let shared_items: Arc<[Arc<dyn SkimItem>]> = items.into();

            // Clones for the process_chunk closure.
            let matcher_engine: Arc<dyn crate::MatchEngine> = Arc::from(matcher_engine);
            let interrupt_for_work = Arc::clone(&interrupt);
            let processed_for_work = Arc::clone(&processed);
            let matched_for_work = Arc::clone(&matched);
            let rank_builder_for_work = Arc::clone(&rank_builder);

            thread_pool::parallel_work_queue(
                &pool_for_work,
                num_workers,
                &shared_items,
                CHUNK_SIZE,
                // identity – seed value for each worker's local accumulator
                Vec::<MatchedItem>::new,
                // process_chunk – called for each chunk; returns a Vec of matches
                move |chunk_start, chunk: &[Arc<dyn crate::SkimItem>]| {
                    // Check interrupt before processing this chunk.
                    if interrupt_for_work.load(Ordering::Relaxed) {
                        return Vec::new();
                    }

                    let mut local_matches = Vec::new();
                    let mut chunk_matched: usize = 0;

                    for (i, item) in chunk.iter().enumerate() {
                        if let Some(match_result) = matcher_engine.match_item(item.as_ref()) {
                            chunk_matched += 1;
                            let mut rank = match_result.rank;
                            let index = chunk_start + i + start;
                            rank.index = i32::try_from(index).unwrap_or(i32::MAX);
                            local_matches.push(MatchedItem::new(
                                Arc::clone(item),
                                rank,
                                Some(match_result.matched_range),
                                &rank_builder_for_work,
                            ));
                        }
                    }

                    // Flush counters for this chunk so the UI sees progress.
                    processed_for_work.fetch_add(chunk.len(), Ordering::Relaxed);
                    if chunk_matched > 0 {
                        matched_for_work.fetch_add(chunk_matched, Ordering::Relaxed);
                    }

                    local_matches
                },
                // reduce – accumulate chunk matches into the worker-local Vec.
                // No sorting here — that would be O(m²/chunk_size) per worker.
                |acc: &mut Vec<MatchedItem>, mut partial: Vec<MatchedItem>| {
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
                move |acc: &mut Vec<MatchedItem>| {
                    if !no_sort {
                        acc.sort_unstable();
                    }
                },
                // merge – concat pre-sorted worker results and sort().
                // Rust's stable sort detects the k sorted runs and merges
                // them in O(n log k), then writes into processed_items.
                |worker_results: Vec<Vec<MatchedItem>>| {
                    if interrupt.load(Ordering::SeqCst) {
                        return;
                    }

                    merge_worker_results(worker_results, no_sort, &processed_items, merge_strategy, &needs_render);
                },
            );
            stopped.store(true, Ordering::Relaxed);
        });

        MatcherControl {
            stopped: stopped_clone,
            interrupt: interrupt_clone,
            matched: matched_clone,
            processed: processed_clone,
        }
    }
}
