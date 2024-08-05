use crate::global::mark_new_run;
///! Reader is used for reading items from datasource (e.g. stdin or command output)
///!
///! After reading in a line, reader will save an item into the pool(items)
use crate::options::SkimOptions;
use crate::{SkimItem, SkimItemReceiver};
use crossbeam_channel::{unbounded, Select, Sender};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock, Weak};
use std::thread::{self, sleep, JoinHandle};
use std::time::Duration;

#[cfg(feature = "malloc_trim")]
#[cfg(target_os = "linux")]
#[cfg(target_env = "gnu")]
use crate::malloc_trim;

const ITEMS_INITIAL_CAPACITY: usize = 65536;
const SLEEP_FAST: Duration = Duration::from_millis(1);
const SLEEP_SLOW: Duration = Duration::from_millis(4);

pub trait CommandCollector {
    /// execute the `cmd` and produce a
    /// - skim item producer
    /// - a channel sender, any message send would mean to terminate the `cmd` process (for now).
    ///
    /// Internally, the command collector may start several threads(components), the collector
    /// should add `1` on every thread creation and sub `1` on thread termination. reader would use
    /// this information to determine whether the collector had stopped or not.
    fn invoke(
        &mut self,
        cmd: &str,
        components_to_stop: Arc<AtomicUsize>,
    ) -> (SkimItemReceiver, Sender<i32>, Option<JoinHandle<()>>);
}

pub struct ReaderControl {
    tx_interrupt: Sender<i32>,
    tx_interrupt_cmd: Option<Sender<i32>>,
    components_to_stop: Arc<AtomicUsize>,
    items: Arc<RwLock<Vec<Arc<dyn SkimItem>>>>,
    thread_reader: Option<JoinHandle<()>>,
    thread_ingest: Option<JoinHandle<()>>,
}

impl Drop for ReaderControl {
    fn drop(&mut self) {
        self.kill();
        drop(self.take());
    }
}

impl ReaderControl {
    pub fn kill(&mut self) {
        debug!(
            "kill reader, components before: {}",
            self.components_to_stop.load(Ordering::SeqCst)
        );

        let _ = self.tx_interrupt_cmd.as_ref().map(|tx| tx.send(1));
        let _ = self.tx_interrupt.send(1);

        if let Some(handle) = self.thread_reader.take() {
            let _ = handle.join();
        }

        if let Some(handle) = self.thread_ingest.take() {
            let _ = handle.join();
        }

        #[cfg(feature = "malloc_trim")]
        #[cfg(target_os = "linux")]
        #[cfg(target_env = "gnu")]
        malloc_trim();

        while self.components_to_stop.load(Ordering::SeqCst) != 0 {}
    }

    pub fn take(&mut self) -> Vec<Arc<dyn SkimItem>> {
        if let Ok(mut locked) = self.items.write() {
            let locked_len = locked.len();
            return std::mem::replace(&mut locked, Vec::with_capacity(locked_len));
        }

        Vec::new()
    }

    pub fn all_stopped(&self) -> bool {
        self.components_to_stop.load(Ordering::SeqCst) == 0
    }

    pub fn is_empty(&self) -> bool {
        if let Ok(locked) = self.items.read() {
            return locked.is_empty();
        }

        false
    }

    pub fn is_done(&self) -> bool {
        self.all_stopped() && self.is_empty()
    }
}

pub struct Reader {
    cmd_collector: Rc<RefCell<dyn CommandCollector>>,
    rx_item: Option<SkimItemReceiver>,
}

impl Reader {
    pub fn with_options(options: &SkimOptions) -> Self {
        Self {
            cmd_collector: options.cmd_collector.clone(),
            rx_item: None,
        }
    }

    pub fn source(mut self, rx_item: Option<SkimItemReceiver>) -> Self {
        self.rx_item = rx_item;
        self
    }

    pub fn run(&mut self, cmd: &str) -> ReaderControl {
        mark_new_run(cmd);

        let components_to_stop: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
        let items_strong = Arc::new(RwLock::new(Vec::with_capacity(ITEMS_INITIAL_CAPACITY)));
        let items_weak = Arc::downgrade(&items_strong);

        let (rx_item, tx_interrupt_cmd, opt_ingest_handle) =
            self.rx_item.take().map(|rx| (rx, None, None)).unwrap_or_else(|| {
                let components_to_stop_clone = components_to_stop.clone();
                let (rx_item, tx_interrupt_cmd, opt_ingest_handle) =
                    self.cmd_collector.borrow_mut().invoke(cmd, components_to_stop_clone);
                (rx_item, Some(tx_interrupt_cmd), opt_ingest_handle)
            });

        let components_to_stop_clone = components_to_stop.clone();
        let (tx_interrupt, thread_reader) = collect_item(components_to_stop_clone, rx_item, items_weak);

        ReaderControl {
            tx_interrupt,
            tx_interrupt_cmd,
            components_to_stop,
            items: items_strong,
            thread_reader: Some(thread_reader),
            thread_ingest: opt_ingest_handle,
        }
    }
}

fn collect_item(
    components_to_stop: Arc<AtomicUsize>,
    rx_item: SkimItemReceiver,
    items_weak: Weak<RwLock<Vec<Arc<dyn SkimItem>>>>,
) -> (Sender<i32>, JoinHandle<()>) {
    let (tx_interrupt, rx_interrupt) = unbounded();

    let started = Arc::new(AtomicBool::new(false));
    let started_clone = started.clone();
    let thread_reader = thread::spawn(move || {
        debug!("reader: collect_item start");
        components_to_stop.fetch_add(1, Ordering::SeqCst);
        started_clone.store(true, Ordering::SeqCst); // notify parent that it is started

        let mut sel = Select::new();
        let item_channel = sel.recv(&rx_item);
        let interrupt_channel = sel.recv(&rx_interrupt);
        let mut empty_count = 0usize;

        if let Some(items_strong) = Weak::upgrade(&items_weak) {
            loop {
                if empty_count >= 10 {
                    break;
                }

                match sel.ready() {
                    i if i == item_channel && !rx_item.is_empty() => {
                        let Ok(mut locked) = items_strong.write() else { continue };

                        locked.extend(rx_item.try_iter());

                        // slow path
                        if empty_count > 1 {
                            // faster for slow path but not for fast path
                            drop(locked);
                            sleep(SLEEP_SLOW);
                            continue;
                        }

                        // fast path
                        drop(locked);
                        sleep(SLEEP_FAST);
                    }
                    i if i == item_channel => {
                        empty_count += 1;
                        continue;
                    }
                    i if i == interrupt_channel => break,
                    _ => unreachable!(),
                }
            }
        }

        components_to_stop.fetch_sub(1, Ordering::SeqCst);
        debug!("reader: collect_item stop");
    });

    while !started.load(Ordering::SeqCst) {
        // busy waiting for the thread to start. (components_to_stop is added)
    }

    (tx_interrupt, thread_reader)
}
