//! This module contains the matching coordinator
use rayon::ThreadPool;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use rayon::prelude::*;

use crate::MatchRange;
use crate::engine::normalized::NormalizedEngineFactory;
use crate::engine::split::SplitMatchEngineFactory;
use crate::item::{ItemPool, MatchedItem, RankBuilder};
use crate::prelude::{AndOrEngineFactory, ExactOrFuzzyEngineFactory, RegexEngineFactory};
use crate::{CaseMatching, MatchEngineFactory, SkimOptions};

static NUM_THREADS: LazyLock<usize> = LazyLock::new(|| {
    std::thread::available_parallelism()
        .ok()
        .map(|inner| inner.get())
        .unwrap_or_else(|| 0)
});

static OPT_MATCHER_THREAD_POOL: LazyLock<Option<ThreadPool>> =
    LazyLock::new(|| rayon::ThreadPoolBuilder::new().num_threads(*NUM_THREADS).build().ok());

static PLACEHOLDER_RANK: [i32; 5] = [0, 0, 0, 0, 0];
static PLACEHOLDER_RANGE: Option<MatchRange> = None;

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
}

impl Matcher {
    /// Creates a new Matcher builder with the given engine factory.
    pub fn builder(engine_factory: Rc<dyn MatchEngineFactory>) -> Self {
        Self {
            engine_factory,
            case_matching: CaseMatching::default(),
        }
    }

    /// Sets the case matching mode (smart, ignore, or respect).
    pub fn case(mut self, case_matching: CaseMatching) -> Self {
        self.case_matching = case_matching;
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
        if options.regex {
            let regex_factory = RegexEngineFactory::builder();
            if options.normalize {
                Rc::new(NormalizedEngineFactory::new(regex_factory))
            } else {
                Rc::new(regex_factory)
            }
        } else {
            let rank_builder = Arc::new(RankBuilder::new(options.tiebreak.clone()));
            log::debug!("Creating matcher for algo {:?}", options.algorithm);
            let fuzzy_engine_factory = ExactOrFuzzyEngineFactory::builder()
                .fuzzy_algorithm(options.algorithm)
                .exact_mode(options.exact)
                .rank_builder(rank_builder)
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
            if options.normalize {
                Rc::new(NormalizedEngineFactory::new(andor_factory))
            } else {
                Rc::new(andor_factory)
            }
        }
    }

    /// Creates a Matcher configured from the given SkimOptions.
    pub fn from_options(options: &SkimOptions) -> Self {
        let engine_factory = Self::create_engine_factory(options);
        Matcher::builder(engine_factory).case(options.case).build()
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
    pub fn run<C>(&self, query: &str, item_pool: Arc<ItemPool>, callback: C) -> MatcherControl
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

        let query_empty: bool = query.is_empty();

        let run_matcher = || {
            rayon::spawn(move || {
                // let _num_taken = item_pool.num_taken();
                let items = item_pool.take();

                // 1. use rayon for parallel
                // 2. return Err to skip iteration
                //    check https://doc.rust-lang.org/std/result/enum.Result.html#method.from_iter

                trace!("matcher start, total: {}", items.len());
                let matched_items: Vec<MatchedItem> = items
                    .par_chunks(8192)
                    .enumerate()
                    .take_any_while(|(_master_idx, chunk)| {
                        if interrupt.load(Ordering::Relaxed) {
                            stopped.store(true, Ordering::Relaxed);
                            return false;
                        }

                        processed.fetch_add(chunk.len(), Ordering::Relaxed);
                        true
                    })
                    .map(|(master_idx, chunk)| {
                        let vec: Vec<MatchedItem> = chunk
                            .iter()
                            .enumerate()
                            .map(move |(idx, item)| {
                                let item_idx = ((master_idx - 1) * 8192) + idx;

                                (item_idx, item)
                            })
                            .filter_map(|(_item_idx, item)| {
                                if interrupt.load(Ordering::Relaxed) {
                                    stopped.store(true, Ordering::Relaxed);
                                    return None;
                                }
                                if query_empty {
                                    return Some(MatchedItem {
                                        item: item.clone(),
                                        rank: PLACEHOLDER_RANK,
                                        matched_range: PLACEHOLDER_RANGE.clone(),
                                    });
                                }

                                matcher_engine.match_item(item.clone()).map(|match_result| {
                                    // item is Arc but we get &Arc from iterator, so one clone is needed
                                    MatchedItem {
                                        item: item.clone(),
                                        rank: match_result.rank,
                                        matched_range: Some(match_result.matched_range),
                                    }
                                })
                            })
                            .collect();

                        matched.fetch_add(vec.len(), Ordering::Relaxed);

                        vec
                    })
                    .flatten_iter()
                    .collect();

                if !interrupt.load(Ordering::SeqCst) {
                    trace!("matcher stop, total matched: {}", matched_items.len());
                    callback(matched_items);
                }
                stopped.store(true, Ordering::Relaxed);
            });
        };

        match OPT_MATCHER_THREAD_POOL.as_ref() {
            Some(pool) => {
                pool.install(|| run_matcher());
            }
            None => run_matcher(),
        }

        MatcherControl {
            stopped: stopped_clone,
            interrupt: interrupt_clone,
            matched: matched_clone,
            processed: processed_clone,
        }
    }
}
