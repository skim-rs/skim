//! Reader is used for reading items from datasource (e.g. stdin or command output)
//!
//! After reading in a line, reader will save an item into the pool(items)
use crate::item::ItemPool;
use crate::options::SkimOptions;
use crate::prelude::Sender;
use crate::spinlock::SpinLock;
use crate::{SkimItem, SkimItemReceiver};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Trait for collecting items from command output
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
    ) -> (SkimItemReceiver, crate::prelude::Sender<i32>);
}

/// Handle for controlling a running reader
pub struct ReaderControl {
    tx_interrupt: Sender<i32>,
    tx_interrupt_cmd: Option<Sender<i32>>,
    components_to_stop: Arc<AtomicUsize>,
    items: Arc<SpinLock<Vec<Arc<dyn SkimItem>>>>,
}

impl ReaderControl {
    /// Kills the reader and waits for all components to stop
    pub fn kill(&mut self) {
        debug!(
            "kill reader, components before: {}",
            self.components_to_stop.load(Ordering::SeqCst)
        );

        let _ = self.tx_interrupt_cmd.clone().map(|tx| tx.send(1));
        let _ = self.tx_interrupt.send(1);
        while self.components_to_stop.load(Ordering::SeqCst) != 0 {}
    }

    /// Takes all items collected so far
    pub fn take(&self) -> Vec<Arc<dyn SkimItem>> {
        let mut items = self.items.lock();
        let mut ret = Vec::with_capacity(items.len());
        ret.append(&mut items);
        ret
    }

    /// Returns true if the reader has finished and no items remain
    pub fn is_done(&self) -> bool {
        let items = self.items.lock();
        self.components_to_stop.load(Ordering::SeqCst) == 0 && items.is_empty()
    }
}

impl Drop for ReaderControl {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Reader for streaming items from commands or other sources
pub struct Reader {
    cmd_collector: Rc<RefCell<dyn CommandCollector>>,
    rx_item: Option<SkimItemReceiver>,
}

impl Reader {
    /// Creates a new reader from skim options
    pub fn from_options(options: &SkimOptions) -> Self {
        Self {
            cmd_collector: options.cmd_collector.clone(),
            rx_item: None,
        }
    }

    /// Sets the item source (if None, will use command collector)
    pub fn source(mut self, rx_item: Option<SkimItemReceiver>) -> Self {
        self.rx_item = rx_item;
        self
    }

    /// Starts the reader and returns a control handle
    pub fn run(&mut self, app_tx: Sender<Vec<Arc<dyn SkimItem>>>, cmd: &str) -> ReaderControl {
        let components_to_stop: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
        let items = Arc::new(SpinLock::new(Vec::new()));

        let (rx_item, tx_interrupt_cmd) = self.rx_item.take().map(|rx| (rx, None)).unwrap_or_else(|| {
            let components_to_stop_clone = components_to_stop.clone();
            let (rx_item, tx_interrupt_cmd) = self.cmd_collector.borrow_mut().invoke(cmd, components_to_stop_clone);
            (rx_item, Some(tx_interrupt_cmd))
        });

        let components_to_stop_clone = components_to_stop.clone();
        let tx_interrupt = collect_item(components_to_stop_clone, rx_item, move |items| _ = app_tx.send(items));

        ReaderControl {
            tx_interrupt,
            tx_interrupt_cmd,
            components_to_stop,
            items,
        }
    }

    /// Starts collecting items and sending them to the pool directly
    /// Returns a control handle
    pub fn collect(&mut self, item_pool: Arc<ItemPool>, cmd: &str) -> ReaderControl {
        let components_to_stop: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
        let items = Arc::new(SpinLock::new(Vec::new()));

        let (rx_item, tx_interrupt_cmd) = self.rx_item.take().map(|rx| (rx, None)).unwrap_or_else(|| {
            let components_to_stop_clone = components_to_stop.clone();
            let (rx_item, tx_interrupt_cmd) = self.cmd_collector.borrow_mut().invoke(cmd, components_to_stop_clone);
            (rx_item, Some(tx_interrupt_cmd))
        });

        let components_to_stop_clone = components_to_stop.clone();
        let tx_interrupt = collect_item(components_to_stop_clone, rx_item, move |items| {
            item_pool.append(items);
        });

        ReaderControl {
            tx_interrupt,
            tx_interrupt_cmd,
            components_to_stop,
            items,
        }
    }
}

fn collect_item<F>(components_to_stop: Arc<AtomicUsize>, rx_item: SkimItemReceiver, callback: F) -> Sender<i32>
where
    F: Fn(Vec<Arc<dyn SkimItem>>) + Send + 'static,
{
    let (tx_interrupt, rx_interrupt) = crate::prelude::bounded(8);

    let started = Arc::new(AtomicBool::new(false));
    let started_clone = started.clone();
    std::thread::spawn(move || {
        debug!("collect_item start");
        components_to_stop.fetch_add(1, Ordering::SeqCst);
        started_clone.store(true, Ordering::SeqCst); // notify parent that it is started

        loop {
            if let Ok(Some(msg)) = rx_interrupt.try_recv() {
                debug!("interrupt: {msg}");
                break;
            }
            match rx_item.try_recv() {
                Ok(Some(items)) => {
                    callback(items);
                }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
                Err(e) => {
                    debug!("rx_item: err {e}");
                    break;
                }
            }
        }

        components_to_stop.fetch_sub(1, Ordering::SeqCst);
        debug!("collect_item stop");
    });

    while !started.load(Ordering::SeqCst) {
        // busy waiting for the thread to start. (components_to_stop is added)
    }

    tx_interrupt
}
