use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use std::thread::{self, JoinHandle};

use defer_drop::DeferDrop;
use rayon::prelude::*;
use rayon::ThreadPool;

use tuikit::key::Key;

use crate::event::Event;
use crate::item::{ItemPool, MatchedItem, MatchedItemMetadata};
use crate::malloc_trim;
use crate::spinlock::SpinLock;
use crate::{CaseMatching, MatchEngine, MatchEngineFactory, SkimItem};
use std::rc::Rc;

//==============================================================================
pub struct MatcherControl {
    stopped: Arc<AtomicBool>,
    processed: Arc<AtomicUsize>,
    matched: Arc<AtomicUsize>,
    items: Arc<SpinLock<Vec<MatchedItem>>>,
    opt_thread_handle: Option<JoinHandle<()>>,
}

impl Drop for MatcherControl {
    fn drop(&mut self) {
        self.kill();
        // lock before drop
        drop(self.take());
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
        if let Some(handle) = self.opt_thread_handle.take() {
            let _ = handle.join();
            malloc_trim()
        }
    }

    fn take(&mut self) -> Vec<MatchedItem> {
        let mut items = self.items.lock();
        std::mem::take(&mut *items)
    }

    pub fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn into_items(&mut self) -> Vec<MatchedItem> {
        while !self.stopped.load(Ordering::Relaxed) {}
        let mut locked = self.items.lock();

        std::mem::take(&mut *locked)
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

    pub fn run(
        &self,
        query: &str,
        disabled: bool,
        thread_pool_weak: Weak<ThreadPool>,
        item_pool_weak: Weak<DeferDrop<ItemPool>>,
        tx_heartbeat: Sender<(Key, Event)>,
    ) -> MatcherControl {
        let matcher_engine = self.engine_factory.create_engine_with_case(query, self.case_matching);
        debug!("engine: {}", matcher_engine);
        let stopped = Arc::new(AtomicBool::new(false));
        let stopped_clone = stopped.clone();
        let processed = Arc::new(AtomicUsize::new(0));
        let processed_clone = processed.clone();
        let matched = Arc::new(AtomicUsize::new(0));
        let matched_clone = matched.clone();
        let matched_items = Arc::new(SpinLock::new(Vec::new()));
        let matched_items_weak = Arc::downgrade(&matched_items);

        // shortcut for when there is no query or query is disabled
        let matcher_disabled: bool = disabled || query.is_empty();

        let matcher_handle = thread::spawn(move || {
            if let Some(thread_pool_strong) = thread_pool_weak.upgrade() {
                thread_pool_strong.install(|| {
                    if let Some(item_pool_strong) = Weak::upgrade(&item_pool_weak) {
                        let num_taken = item_pool_strong.num_taken();
                        let items = item_pool_strong.take();

                        trace!("matcher start, total: {}", items.len());

                        let new_items = items
                            .par_iter()
                            .enumerate()
                            .chunks(4096)
                            .take_any_while(|_| !stopped.load(Ordering::Relaxed))
                            .map(|vec| {
                                vec.into_iter()
                                    .filter_map(|(index, item)| {
                                        processed.fetch_add(1, Ordering::Relaxed);

                                        if matcher_disabled {
                                            return Some(MatchedItem {
                                                item: Arc::downgrade(item),
                                                metadata: None,
                                            });
                                        }

                                        process_item(index, num_taken, matched.clone(), matcher_engine.as_ref(), item)
                                    })
                                    .collect::<Vec<MatchedItem>>()
                            })
                            .flatten()
                            .collect();

                        if !stopped.load(Ordering::Relaxed) {
                            if let Some(strong) = Weak::upgrade(&matched_items_weak) {
                                let mut pool = strong.lock();
                                *pool = new_items;
                                trace!("matcher stop, total matched: {}", pool.len());
                            }
                        }
                    }
                });
            }

            let _ = tx_heartbeat.send((Key::Null, Event::EvHeartBeat));
            stopped.store(true, Ordering::Relaxed);
        });

        MatcherControl {
            stopped: stopped_clone,
            matched: matched_clone,
            processed: processed_clone,
            items: matched_items,
            opt_thread_handle: Some(matcher_handle),
        }
    }
}

fn process_item(
    index: usize,
    num_taken: usize,
    matched: Arc<AtomicUsize>,
    matcher_engine: &dyn MatchEngine,
    item: &Arc<dyn SkimItem>,
) -> Option<MatchedItem> {
    matcher_engine.match_item(item.as_ref()).map(|match_result| {
        matched.fetch_add(1, Ordering::Relaxed);
        MatchedItem {
            item: Arc::downgrade(item),
            metadata: {
                Some(Box::new({
                    MatchedItemMetadata {
                        rank: match_result.rank,
                        matched_range: Some(match_result.matched_range),
                        item_idx: (num_taken + index) as u32,
                    }
                }))
            },
        }
    })
}
