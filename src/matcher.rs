use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;

use rayon::prelude::*;

use crate::item::{ItemPool, MatchedItem, RankBuilder};
use crate::prelude::{AndOrEngineFactory, ExactOrFuzzyEngineFactory, RegexEngineFactory};
use crate::spinlock::SpinLock;
use crate::{CaseMatching, MatchEngineFactory, SkimOptions};
use defer_drop::DeferDrop;
use std::rc::Rc;

//==============================================================================
pub struct MatcherControl {
    stopped: Arc<AtomicBool>,
    processed: Arc<AtomicUsize>,
    matched: Arc<AtomicUsize>,
    items: Arc<SpinLock<Vec<MatchedItem>>>,
}

impl Default for MatcherControl {
    fn default() -> Self {
        Self {
            // Default to stopped=true so initial state indicates "no matcher running"
            stopped: Arc::new(AtomicBool::new(true)),
            processed: Arc::new(AtomicUsize::new(0)),
            matched: Arc::new(AtomicUsize::new(0)),
            items: Arc::new(SpinLock::new(Vec::new())),
        }
    }
}

impl MatcherControl {
    pub fn get_num_processed(&self) -> usize {
        self.processed.load(Ordering::Relaxed)
    }

    pub fn get_num_matched(&self) -> usize {
        self.matched.load(Ordering::Relaxed)
    }

    pub fn kill(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
    }

    pub fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
    }

    pub fn items(&self) -> Arc<SpinLock<Vec<MatchedItem>>> {
        while !self.stopped() {}
        self.items.clone()
    }
}

impl Drop for MatcherControl {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
    }
}

//==============================================================================
pub struct Matcher {
    engine_factory: Rc<dyn MatchEngineFactory>,
    case_matching: CaseMatching,
}

impl Matcher {
    pub fn builder(engine_factory: Rc<dyn MatchEngineFactory>) -> Self {
        Self {
            engine_factory,
            case_matching: CaseMatching::default(),
        }
    }

    pub fn case(mut self, case_matching: CaseMatching) -> Self {
        self.case_matching = case_matching;
        self
    }

    pub fn build(self) -> Self {
        self
    }

    pub fn from_options(options: &SkimOptions) -> Self {
        let engine_factory: Rc<dyn MatchEngineFactory> = if options.regex {
            Rc::new(RegexEngineFactory::builder())
        } else {
            let rank_builder = Arc::new(RankBuilder::new(options.tiebreak.clone()));
            log::debug!("Creating matcher for algo {:?}", options.algorithm);
            let fuzzy_engine_factory = ExactOrFuzzyEngineFactory::builder()
                .fuzzy_algorithm(options.algorithm)
                .exact_mode(options.exact)
                .rank_builder(rank_builder)
                .build();
            Rc::new(AndOrEngineFactory::new(fuzzy_engine_factory))
        };

        Matcher::builder(engine_factory).case(options.case).build()
    }

    pub fn run<C>(&self, query: &str, item_pool: Arc<DeferDrop<ItemPool>>, callback: C) -> MatcherControl
    where
        C: Fn(Arc<SpinLock<Vec<MatchedItem>>>) + Send + 'static,
    {
        let matcher_engine = self.engine_factory.create_engine_with_case(query, self.case_matching);
        debug!("engine: {matcher_engine}");
        let stopped = Arc::new(AtomicBool::new(false));
        let stopped_clone = stopped.clone();
        let processed = Arc::new(AtomicUsize::new(0));
        let processed_clone = processed.clone();
        let matched = Arc::new(AtomicUsize::new(0));
        let matched_clone = matched.clone();
        let matched_items = Arc::new(SpinLock::new(Vec::new()));
        let matched_items_clone = matched_items.clone();

        thread::spawn(move || {
            let _num_taken = item_pool.num_taken();
            let items = item_pool.take();

            // 1. use rayon for parallel
            // 2. return Err to skip iteration
            //    check https://doc.rust-lang.org/std/result/enum.Result.html#method.from_iter

            trace!("matcher start, total: {}", items.len());
            let result: Result<Vec<_>, _> = items
                .into_par_iter()
                .enumerate()
                .filter_map(|(_, item)| {
                    processed.fetch_add(1, Ordering::Relaxed);
                    if stopped.load(Ordering::Relaxed) {
                        Some(Err("matcher killed"))
                    } else if let Some(match_result) = matcher_engine.match_item(item.clone()) {
                        matched.fetch_add(1, Ordering::Relaxed);
                        // item is Arc but we get &Arc from iterator, so one clone is needed
                        Some(Ok(MatchedItem {
                            item: item.clone(),
                            rank: match_result.rank,
                            matched_range: Some(match_result.matched_range),
                        }))
                    } else {
                        None
                    }
                })
                .collect();

            if let Ok(items) = result {
                let mut pool = matched_items.lock();
                *pool = items;
                trace!("matcher stop, total matched: {}", pool.len());
            }

            callback(matched_items.clone());
            stopped.store(true, Ordering::Relaxed);
        });

        MatcherControl {
            stopped: stopped_clone,
            matched: matched_clone,
            processed: processed_clone,
            items: matched_items_clone,
        }
    }
}
