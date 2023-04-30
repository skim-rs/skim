use crate::global::mark_new_run;
///! Reader is used for reading items from datasource (e.g. stdin or command output)
///!
///! After reading in a line, reader will save an item into the pool(items)
use crate::options::SkimOptions;
use crate::spinlock::SpinLock;
use crate::{SkimItem, SkimItemReceiver};
use crossbeam_channel::{bounded, Select, Sender};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

const CHANNEL_SIZE: usize = 1024;
const ITEMS_INITIAL_CAPACITY: usize = 65536;

pub trait CommandCollector {
    /// execute the `cmd` and produce a
    /// - skim item producer
    /// - a channel sender, any message send would mean to terminate the `cmd` process (for now).
    ///
    /// Internally, the command collector may start several threads(components), the collector
    /// should add `1` on every thread creation and sub `1` on thread termination. reader would use
    /// this information to determine whether the collector had stopped or not.
    fn invoke(&mut self, cmd: &str, components_to_stop: Arc<AtomicUsize>) -> (SkimItemReceiver, Sender<i32>);
}

pub struct ReaderControl {
    tx_interrupt: Sender<i32>,
    tx_interrupt_cmd: Option<Sender<i32>>,
    components_to_stop: Arc<AtomicUsize>,
    items: Arc<SpinLock<Vec<Arc<dyn SkimItem>>>>,
}

impl ReaderControl {
    pub fn kill(self) {
        debug!(
            "kill reader, components before: {}",
            self.components_to_stop.load(Ordering::SeqCst)
        );

        let _ = self.tx_interrupt_cmd.map(|tx| tx.send(1));
        let _ = self.tx_interrupt.send(1);
        while self.components_to_stop.load(Ordering::SeqCst) != 0 {}
    }

    pub fn take(&self) -> Vec<Arc<dyn SkimItem>> {
        let mut items = self.items.lock();
        let mut ret = Vec::with_capacity(items.len());
        ret.append(&mut items);
        ret
    }

    pub fn is_done(&self) -> bool {
        let items = self.items.lock();
        self.components_to_stop.load(Ordering::SeqCst) == 0 && items.is_empty()
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
        let items = Arc::new(SpinLock::new(Vec::with_capacity(ITEMS_INITIAL_CAPACITY)));
        let items_clone = items.clone();

        let (rx_item, tx_interrupt_cmd) = self.rx_item.take().map(|rx| (rx, None)).unwrap_or_else(|| {
            let components_to_stop_clone = components_to_stop.clone();
            let (rx_item, tx_interrupt_cmd) = self.cmd_collector.borrow_mut().invoke(cmd, components_to_stop_clone);
            (rx_item, Some(tx_interrupt_cmd))
        });

        let components_to_stop_clone = components_to_stop.clone();
        let tx_interrupt = collect_item(components_to_stop_clone, rx_item, items_clone);

        ReaderControl {
            tx_interrupt,
            tx_interrupt_cmd,
            components_to_stop,
            items,
        }
    }
}

fn collect_item(
    components_to_stop: Arc<AtomicUsize>,
    rx_item: SkimItemReceiver,
    items: Arc<SpinLock<Vec<Arc<dyn SkimItem>>>>,
) -> Sender<i32> {
    let (tx_interrupt, rx_interrupt) = bounded(CHANNEL_SIZE);

    let started = Arc::new(AtomicBool::new(false));
    let started_clone = started.clone();
    thread::spawn(move || {
        debug!("reader: collect_item start");
        components_to_stop.fetch_add(1, Ordering::SeqCst);
        started_clone.store(true, Ordering::SeqCst); // notify parent that it is started

        let mut sel = Select::new();
        let item_channel = sel.recv(&rx_item);
        let interrupt_channel = sel.recv(&rx_interrupt);

        loop {
            match sel.ready() {
                i if i == item_channel => {
                    if let Ok(item) = rx_item.recv() {
                        let mut vec = items.lock();
                        vec.push(item)
                    } else {
                        break;
                    }
                }
                i if i == interrupt_channel => break,
                _ => unreachable!(),
            }
        }

        components_to_stop.fetch_sub(1, Ordering::SeqCst);
        debug!("reader: collect_item stop");
    });

    while !started.load(Ordering::SeqCst) {
        // busy waiting for the thread to start. (components_to_stop is added)
    }

    tx_interrupt
}
