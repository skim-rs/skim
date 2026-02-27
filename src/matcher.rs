//! This module contains the matching coordinator
use rayon::ThreadPool;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use rayon::prelude::*;

use crate::engine::normalized::NormalizedEngineFactory;
use crate::engine::split::SplitMatchEngineFactory;
use crate::item::{ItemPool, MatchedItem, RankBuilder};
use crate::prelude::{AndOrEngineFactory, ExactOrFuzzyEngineFactory, RegexEngineFactory};
use crate::{CaseMatching, MatchEngineFactory, SkimOptions};

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
    pub fn get_num_processed(&self) -> usize {
        self.processed.load(Ordering::Relaxed)
    }

    /// Returns the number of items that have matched so far.
    pub fn get_num_matched(&self) -> usize {
        self.matched.load(Ordering::Relaxed)
    }

    /// Signals the matcher to stop processing.
    pub fn kill(&mut self) {
        self.interrupt.store(true, Ordering::Relaxed);
    }

    /// Returns true if the matcher has stopped (either completed or killed).
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
    pub fn case(mut self, case_matching: CaseMatching) -> Self {
        self.case_matching = case_matching;
        self
    }

    /// Sets the rank builder (carries tiebreak criteria).
    pub fn rank_builder(mut self, rank_builder: Arc<RankBuilder>) -> Self {
        self.rank_builder = rank_builder;
        self
    }

    /// Finalizes the builder and returns the configured Matcher.
    pub fn build(self) -> Self {
        self
    }

    /// Creates a MatchEngineFactory from the given options.
    ///
    /// This is useful when you need the factory directly (e.g., for filter mode)
    /// without creating a full Matcher instance.
    pub fn create_engine_factory(options: &SkimOptions) -> Rc<dyn MatchEngineFactory> {
        Self::create_engine_factory_with_builder(options).0
    }

    /// Creates a MatchEngineFactory and the associated RankBuilder from the given options.
    ///
    /// Returns both so callers can attach the builder to `MatchedItem`s for lazy sort-key
    /// computation.
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
                .rank_builder(rank_builder.clone())
                .build();

            // If split_match is enabled, wrap the fuzzy factory with SplitMatchEngineFactory
            // Then wrap with AndOrEngineFactory so that queries like "foo:bar baz:qux" work
            let andor_factory = if let Some(delimiter) = options.split_match {
                let split_factory = SplitMatchEngineFactory::new(fuzzy_engine_factory, delimiter);
                AndOrEngineFactory::new(split_factory)
            } else {
                AndOrEngineFactory::new(fuzzy_engine_factory)
            };

            // Wrap with NormalizedEngineFactory if normalization is requested
            let factory: Rc<dyn MatchEngineFactory> = if options.normalize {
                Rc::new(NormalizedEngineFactory::new(andor_factory))
            } else {
                Rc::new(andor_factory)
            };
            (factory, rank_builder)
        }
    }

    /// Creates a Matcher configured from the given SkimOptions.
    pub fn from_options(options: &SkimOptions) -> Self {
        let (engine_factory, rank_builder) = Self::create_engine_factory_with_builder(options);
        Matcher::builder(engine_factory)
            .case(options.case)
            .rank_builder(rank_builder)
            .build()
    }

    /// Returns the case matching setting for this matcher.
    pub fn case_matching(&self) -> CaseMatching {
        self.case_matching
    }

    /// Returns a reference to the engine factory.
    pub fn engine_factory(&self) -> &Rc<dyn MatchEngineFactory> {
        &self.engine_factory
    }

    /// Runs the matcher on items from the pool in a background thread.
    ///
    /// The callback is invoked when matching is complete with the matched items.
    /// Returns a MatcherControl that can be used to monitor progress or stop the matcher.
    pub fn run<C>(&self, query: &str, item_pool: Arc<ItemPool>, thread_pool: &ThreadPool, callback: C) -> MatcherControl
    where
        C: Fn(Vec<MatchedItem>) + Send + 'static,
    {
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
        let items = item_pool.take();
        let total = items.len();
        trace!("matcher start, total: {}", total);

        thread_pool.spawn(move || {
            // Process items in parallel using chunk-based accounting to minimize
            // atomic contention. Each rayon work unit processes a chunk of items,
            // updating the shared `processed` and `matched` counters only once per
            // chunk instead of once per item. The interrupt flag is also checked
            // only once per chunk to amortize the atomic load.
            //
            // `with_min_len` ensures rayon doesn't split work into chunks smaller
            // than CHUNK_SIZE, keeping the overhead of the parallel iterator low
            // relative to the actual matching work.
            const CHUNK_SIZE: usize = 512;

            let matched_items: Vec<MatchedItem> = items
                .into_par_iter()
                .with_min_len(CHUNK_SIZE)
                .fold(
                    || (Vec::new(), 0usize, 0usize), // (local_matches, local_processed, local_matched)
                    |(mut local_matches, mut local_processed, mut local_matched), item| {
                        // Check interrupt once at the start of each chunk boundary.
                        // The fold processes items sequentially within each rayon work unit,
                        // so checking every CHUNK_SIZE items amortizes the atomic load.
                        if local_processed % CHUNK_SIZE == 0 && interrupt.load(Ordering::Relaxed) {
                            return (local_matches, local_processed, local_matched);
                        }

                        local_processed += 1;

                        if let Some(match_result) = matcher_engine.match_item(item.as_ref()) {
                            local_matched += 1;
                            local_matches.push(MatchedItem {
                                item,
                                rank: match_result.rank,
                                rank_builder: rank_builder.clone(),
                                matched_range: Some(match_result.matched_range),
                            });
                        }

                        // Flush counters periodically so the UI sees progress updates.
                        if local_processed % CHUNK_SIZE == 0 {
                            processed.fetch_add(CHUNK_SIZE, Ordering::Relaxed);
                            if local_matched > 0 {
                                matched.fetch_add(local_matched, Ordering::Relaxed);
                                local_matched = 0;
                            }
                        }

                        (local_matches, local_processed, local_matched)
                    },
                )
                .map(|(local_matches, local_processed, local_matched)| {
                    // Flush any remaining counts that didn't hit a chunk boundary.
                    let remainder = local_processed % CHUNK_SIZE;
                    if remainder > 0 {
                        processed.fetch_add(remainder, Ordering::Relaxed);
                    }
                    if local_matched > 0 {
                        matched.fetch_add(local_matched, Ordering::Relaxed);
                    }
                    local_matches
                })
                .reduce(Vec::new, |mut a, mut b| {
                    // Merge per-thread result vectors. Always extend the larger one
                    // to avoid unnecessary reallocations.
                    if a.len() >= b.len() {
                        a.extend(b);
                        a
                    } else {
                        b.extend(a);
                        b
                    }
                });

            if !interrupt.load(Ordering::SeqCst) {
                trace!("matcher stop, total matched: {}", matched_items.len());
                callback(matched_items);
            }
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
