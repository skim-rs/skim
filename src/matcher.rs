use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Weak};

use rayon::prelude::*;
use rayon::ThreadPool;
use tuikit::key::Key;

use crate::event::Event;
use crate::item::{ItemPool, MatchedItem};
use crate::spinlock::SpinLock;
use crate::{CaseMatching, MatchEngine, MatchEngineFactory, SkimItem};
use crate::{MatchRange, Rank};
use std::rc::Rc;

const UNMATCHED_RANK: Rank = [0i32, 0i32, 0i32, 0i32];
const UNMATCHED_RANGE: Option<MatchRange> = None;

//==============================================================================
pub struct MatcherControl {
    stopped: Arc<AtomicBool>,
    processed: Arc<AtomicUsize>,
    matched: Arc<AtomicUsize>,
    items: Arc<SpinLock<Vec<MatchedItem>>>,
}

impl Drop for MatcherControl {
    fn drop(&mut self) {
        self.kill();
        // wait until fully stopped to drop unlike take()
        drop(self.into_items());
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

    pub fn take(&mut self) -> Vec<MatchedItem> {
        let mut items = self.items.lock();
        std::mem::take(&mut *items)
    }

    pub fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn into_items(&mut self) -> Vec<MatchedItem> {
        while !self.stopped.load(Ordering::Relaxed) {}
        self.take()
    }
}

//==============================================================================
pub struct Matcher {
    engine_factory: Rc<dyn MatchEngineFactory>,
    case_matching: CaseMatching,
}

#[allow(dead_code)]
impl Matcher {
    pub fn builder(engine_factory: Rc<dyn MatchEngineFactory>) -> Self {
        Self {
            engine_factory,
            case_matching: CaseMatching::default(),
        }
    }

    pub fn get_case(&self) -> CaseMatching {
        self.case_matching.clone()
    }

    pub fn set_case(mut self, case_matching: CaseMatching) -> Self {
        self.case_matching = case_matching;
        self
    }

    pub fn build(self) -> Self {
        self
    }

    pub fn run(
        &self,
        query: &str,
        disabled: bool,
        item_pool_weak: Weak<ItemPool>,
        tx_heartbeat: Sender<(Key, Event)>,
        matched_items: Vec<MatchedItem>,
        matcher_pool: &Arc<ThreadPool>,
    ) -> MatcherControl {
        let matcher_engine = self.engine_factory.create_engine_with_case(query, self.case_matching);
        debug!("engine: {}", matcher_engine);
        let stopped = Arc::new(AtomicBool::new(false));
        let stopped_clone = stopped.clone();
        let processed = Arc::new(AtomicUsize::new(0));
        let processed_clone = processed.clone();
        let matched = Arc::new(AtomicUsize::new(0));
        let matched_clone = matched.clone();
        let matched_items = Arc::new(SpinLock::new(matched_items));
        let matched_items_weak = Arc::downgrade(&matched_items);

        // shortcut for when there is no query or query is disabled
        let matcher_disabled: bool = disabled || query.is_empty();

        matcher_pool.install(|| {
            matcher_pool.spawn(move || {
                if let Some(item_pool_strong) = Weak::upgrade(&item_pool_weak) {
                    let num_taken = item_pool_strong.num_taken();
                    let items = item_pool_strong.take();
                    let stopped_ref = stopped.as_ref();
                    let processed_ref = processed.as_ref();
                    let matched_ref = matched.as_ref();

                    trace!("matcher start, total: {}", items.len());

                    if let Some(matched_items_strong) = Weak::upgrade(&matched_items_weak) {
                        let par_iter = items
                            .par_iter()
                            .enumerate()
                            .chunks(4096)
                            .take_any_while(|vec| {
                                if stopped_ref.load(Ordering::Relaxed) {
                                    return false;
                                }

                                processed_ref.fetch_add(vec.len(), Ordering::Relaxed);
                                true
                            })
                            .flatten()
                            .filter_map(|(index, item)| {
                                // dummy values should not change, as changing them
                                // may cause the disabled/query empty case disappear!
                                // especially item index.  Needs an index to appear!
                                if matcher_disabled {
                                    return Some(MatchedItem {
                                        item: Arc::downgrade(item),
                                        rank: UNMATCHED_RANK,
                                        matched_range: UNMATCHED_RANGE,
                                        item_idx: (num_taken + index) as u32,
                                    });
                                }

                                Self::process_item(index, num_taken, matched_ref, matcher_engine.as_ref(), item)
                            });

                        if !stopped_ref.load(Ordering::Relaxed) {
                            let mut pool = matched_items_strong.lock();
                            pool.clear();
                            pool.par_extend(par_iter);
                            trace!("matcher stop, total matched: {}", pool.len());
                        }
                    }
                }

                let _ = tx_heartbeat.send((Key::Null, Event::EvHeartBeat));
                stopped.store(true, Ordering::Relaxed);
            });
        });

        MatcherControl {
            stopped: stopped_clone,
            matched: matched_clone,
            processed: processed_clone,
            items: matched_items,
        }
    }

    fn process_item(
        index: usize,
        num_taken: usize,
        matched: &AtomicUsize,
        matcher_engine: &dyn MatchEngine,
        item: &Arc<dyn SkimItem>,
    ) -> Option<MatchedItem> {
        matcher_engine.match_item(item.as_ref()).map(|match_result| {
            matched.fetch_add(1, Ordering::Relaxed);

            MatchedItem {
                item: Arc::downgrade(item),
                rank: match_result.rank,
                matched_range: Some(match_result.matched_range),
                item_idx: (num_taken + index) as u32,
            }
        })
    }
}
