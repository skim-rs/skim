//! Helper utilities for converting input sources into skim item streams.

use std::collections::BTreeMap;
use std::error::Error;
use std::io::{BufRead, BufReader};
use std::process::{Child, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::thread_pool::ThreadPool;

/// Size of the read buffer used by the parallel I/O reader thread.
const PARALLEL_READ_BUF_SIZE: usize = 256 * 1024;

use regex::Regex;

use crate::field::FieldRange;
use crate::helper::item::DefaultSkimItem;
use crate::reader::CommandCollector;
use crate::{SkimItem, SkimItemReceiver, SkimItemSender, SkimOptions};

const DELIMITER_STR: &str = r"[\t\n ]+";
const READ_BUFFER_SIZE: usize = 1024;
const ITEMS_BUFFER_SIZE: usize = 1024;
const SEND_TIMEOUT_MS: u64 = 100; // Send items if we haven't sent anything in 100ms

pub enum CollectorInput {
    Pipe(Box<dyn BufRead + Send>),
    Command(String),
}

/// Options for configuring how items are read and parsed
#[derive(Debug)]
pub struct SkimItemReaderOption {
    buf_size: usize,
    use_ansi_color: bool,
    transform_fields: Vec<FieldRange>,
    matching_fields: Vec<FieldRange>,
    delimiter: Regex,
    line_ending: u8,
    show_error: bool,
}

impl Default for SkimItemReaderOption {
    fn default() -> Self {
        Self {
            buf_size: READ_BUFFER_SIZE,
            line_ending: b'\n',
            use_ansi_color: false,
            transform_fields: Vec::new(),
            matching_fields: Vec::new(),
            delimiter: Regex::new(DELIMITER_STR).unwrap(),
            show_error: false,
        }
    }
}

impl SkimItemReaderOption {
    /// Creates reader options from skim options
    #[must_use]
    pub fn from_options(options: &SkimOptions) -> Self {
        Self {
            buf_size: READ_BUFFER_SIZE,
            line_ending: if options.read0 { b'\0' } else { b'\n' },
            use_ansi_color: options.ansi,
            transform_fields: options
                .with_nth
                .iter()
                .filter_map(|f| if f.is_empty() { None } else { FieldRange::from_str(f) })
                .collect(),
            matching_fields: options
                .nth
                .iter()
                .filter_map(|f| if f.is_empty() { None } else { FieldRange::from_str(f) })
                .collect(),
            delimiter: options.delimiter.clone(),
            show_error: options.show_cmd_error,
        }
    }

    /// Sets the buffer size for reading
    #[must_use]
    pub fn buf_size(mut self, buf_size: usize) -> Self {
        self.buf_size = buf_size;
        self
    }

    /// Sets the line ending character (default: '\n')
    #[must_use]
    pub fn line_ending(mut self, line_ending: u8) -> Self {
        self.line_ending = line_ending;
        self
    }

    /// Enables or disables ANSI color code parsing
    #[must_use]
    pub fn ansi(mut self, enable: bool) -> Self {
        self.use_ansi_color = enable;
        self
    }

    /// Sets the field delimiter regex
    #[must_use]
    pub fn delimiter(mut self, delimiter: Regex) -> Self {
        self.delimiter = delimiter;
        self
    }

    /// Sets the fields to display (transform) from the input
    #[must_use]
    pub fn with_nth<'a, T>(mut self, with_nth: T) -> Self
    where
        T: Iterator<Item = &'a str>,
    {
        self.transform_fields = with_nth.filter_map(FieldRange::from_str).collect();
        self
    }

    /// Sets the transform fields directly
    #[must_use]
    pub fn transform_fields(mut self, transform_fields: Vec<FieldRange>) -> Self {
        self.transform_fields = transform_fields;
        self
    }

    /// Sets the fields to use for matching
    #[must_use]
    pub fn nth<'a, T>(mut self, nth: T) -> Self
    where
        T: Iterator<Item = &'a str>,
    {
        self.matching_fields = nth.filter_map(FieldRange::from_str).collect();
        self
    }

    /// Sets the matching fields directly
    #[must_use]
    pub fn matching_fields(mut self, matching_fields: Vec<FieldRange>) -> Self {
        self.matching_fields = matching_fields;
        self
    }

    /// Enables reading null-terminated lines instead of newline-terminated
    #[must_use]
    pub fn read0(mut self, enable: bool) -> Self {
        if enable {
            self.line_ending = b'\0';
        } else {
            self.line_ending = b'\n';
        }
        self
    }

    /// Sets whether to show command errors
    #[must_use]
    pub fn show_error(mut self, show_error: bool) -> Self {
        self.show_error = show_error;
        self
    }

    /// Builds the options (currently a no-op, returns self)
    #[must_use]
    pub fn build(self) -> Self {
        self
    }

    /// Returns true if no field transformations or ANSI parsing is needed
    #[must_use]
    pub fn is_simple(&self) -> bool {
        !self.use_ansi_color && self.matching_fields.is_empty() && self.transform_fields.is_empty()
    }
}

/// Reader for converting various input sources into streams of skim items
pub struct SkimItemReader {
    option: Arc<SkimItemReaderOption>,
    /// Thread pool used for chunk-processing jobs.  Reader and matcher share
    /// this pool so they compete for the same thread budget rather than each
    /// spawning their own OS threads.  Defaults to a private pool sized to the
    /// number of logical CPUs; callers can replace it with a shared pool via
    /// [`with_thread_pool`](Self::with_thread_pool) or
    /// [`set_thread_pool`](Self::set_thread_pool).
    thread_pool: Arc<ThreadPool>,
}

fn default_thread_pool() -> Arc<ThreadPool> {
    let n = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let (reader_threads, _) = crate::thread_pool::partition_threads(n);
    Arc::new(ThreadPool::new(reader_threads))
}

impl Default for SkimItemReader {
    fn default() -> Self {
        Self {
            option: Arc::new(Default::default()),
            thread_pool: default_thread_pool(),
        }
    }
}

impl SkimItemReader {
    /// Creates a new item reader with the given options
    #[must_use]
    pub fn new(option: SkimItemReaderOption) -> Self {
        Self {
            option: Arc::new(option),
            thread_pool: default_thread_pool(),
        }
    }

    /// Sets the reader options
    #[must_use]
    pub fn option(mut self, option: SkimItemReaderOption) -> Self {
        self.option = Arc::new(option);
        self
    }

    /// Replaces the thread pool used for chunk-processing.  Pass the matcher's
    /// pool here so that reader and matcher share the same thread budget.
    #[must_use]
    pub fn with_thread_pool(mut self, pool: Arc<ThreadPool>) -> Self {
        self.thread_pool = pool;
        self
    }

    /// Like [`with_thread_pool`] but takes `&mut self` — useful when the pool
    /// is only available after construction (e.g. injected from the app).
    pub fn set_thread_pool(&mut self, pool: Arc<ThreadPool>) {
        self.thread_pool = pool;
    }
}

impl SkimItemReader {
    /// Converts a `BufRead` source into a stream of skim items
    pub fn of_bufread(&self, source: impl BufRead + Send + 'static) -> SkimItemReceiver {
        if self.option.is_simple() {
            self.raw_bufread(source)
        } else {
            self.read_and_collect_from_command(Arc::new(AtomicUsize::new(0)), CollectorInput::Pipe(Box::new(source)))
                .0
        }
    }

    /// Helper function that contains the common logic for reading lines from a `BufRead` source
    /// and converting them into `SkimItems`.
    fn read_lines_into_items(
        mut source: impl BufRead + Send + 'static,
        tx_item: &SkimItemSender,
        option: &Arc<SkimItemReaderOption>,
        transform_fields: &[FieldRange],
        matching_fields: &[FieldRange],
    ) {
        let mut buffer = Vec::with_capacity(option.buf_size);
        let mut items_to_send = Vec::with_capacity(ITEMS_BUFFER_SIZE);
        let mut last_send_time = Instant::now();
        let send_timeout = Duration::from_millis(SEND_TIMEOUT_MS);

        loop {
            buffer.clear();

            // start reading
            match source.read_until(option.line_ending, &mut buffer) {
                Ok(0) => break,
                Ok(_) => {
                    // Strip line endings
                    if buffer.ends_with(b"\r\n") {
                        buffer.pop();
                        buffer.pop();
                    } else if buffer.ends_with(&[option.line_ending]) {
                        buffer.pop();
                    }

                    let Ok(line) = std::str::from_utf8(&buffer) else {
                        continue;
                    };

                    trace!("got item {line}");

                    let raw_item = DefaultSkimItem::new(
                        line,
                        option.use_ansi_color,
                        transform_fields,
                        matching_fields,
                        &option.delimiter,
                    );
                    items_to_send.push(Arc::new(raw_item) as Arc<dyn SkimItem>);
                }
                Err(err) => {
                    trace!("Got {err:?} when reading, skipping");
                } // String not UTF8 or other error, skip.
            }

            // Send batched items if buffer is full OR timeout has elapsed
            let should_send = items_to_send.len() == ITEMS_BUFFER_SIZE
                || (!items_to_send.is_empty() && last_send_time.elapsed() >= send_timeout);

            if should_send {
                let batch = std::mem::replace(&mut items_to_send, Vec::with_capacity(ITEMS_BUFFER_SIZE));
                match tx_item.send(batch) {
                    Ok(()) => {
                        last_send_time = Instant::now();
                    }
                    Err(e) => {
                        warn!("Failed to send items: {e:?}");
                        break;
                    }
                }
            }
        }

        // Send remaining items
        if !items_to_send.is_empty() {
            let _ = tx_item.send(items_to_send);
        }
    }

    /// Parallel reader for the simple (no ANSI, no field transforms) case.
    ///
    /// Pipeline:
    ///
    /// 1. **I/O thread** (dedicated) — reads large byte chunks (~256 KB) from
    ///    `source`, splitting on line boundaries, and sends them tagged with
    ///    monotonic sequence numbers into a bounded channel.
    /// 2. **Dispatcher thread** (dedicated, lightweight) — drains that channel
    ///    and submits one pool job per chunk.  The bounded channel provides
    ///    natural back-pressure on the I/O thread when the pool is busy.
    /// 3. **Pool jobs** — parse lines, validate UTF-8, and create
    ///    `DefaultSkimItem` + `Arc` per line.  Because these jobs share the
    ///    same pool as the matcher, reader and matcher compete for the same
    ///    thread budget rather than over-subscribing available CPU cores.
    /// 4. **Reorder thread** (dedicated) — collects `(seq, items)` from pool
    ///    jobs and emits them in sequence order so downstream index assignment
    ///    and `--tac` behaviour are correct.
    fn raw_bufread(&self, source: impl BufRead + Send + 'static) -> SkimItemReceiver {
        let (tx_item, rx_item): (SkimItemSender, SkimItemReceiver) = kanal::bounded(1024 * 1024);
        let option = self.option.clone();
        let pool = Arc::clone(&self.thread_pool);

        let num_threads = pool.num_threads();
        let (tx_chunks, rx_chunks) = kanal::bounded::<(usize, Vec<u8>)>(num_threads * 4);
        let (tx_results, rx_results) = kanal::bounded::<(usize, Vec<Arc<dyn SkimItem>>)>(num_threads * 4);

        let line_ending = option.line_ending;

        // Stage 1: I/O thread.
        Self::spawn_io_reader(source, tx_chunks, line_ending);

        // Stage 2: dispatcher thread — bridges the bounded channel to the pool.
        thread::spawn(move || {
            while let Ok((seq, chunk)) = rx_chunks.recv() {
                let tx = tx_results.clone();
                let opt = option.clone();
                pool.spawn(move || {
                    let result = Self::process_chunk(seq, &chunk, &opt);
                    let _ = tx.send(result);
                });
            }
            // rx_chunks closed → all chunks dispatched; tx_results dropped here
            // so the reorder thread exits once the last pool job finishes.
        });

        // Stage 4: reorder thread.
        Self::spawn_reorder_thread(rx_results, tx_item);

        rx_item
    }

    /// Stage 1 of the parallel reader: reads large byte chunks from `source`,
    /// splitting on line boundaries, and sends them to workers.
    fn spawn_io_reader(
        source: impl BufRead + Send + 'static,
        tx_chunks: kanal::Sender<(usize, Vec<u8>)>,
        line_ending: u8,
    ) {
        thread::spawn(move || {
            debug!("parallel reader: I/O thread start");

            let mut source = source;
            let mut leftover: Vec<u8> = Vec::new();
            let mut seq = 0usize;
            let mut read_buf = vec![0u8; PARALLEL_READ_BUF_SIZE];

            loop {
                let n = match std::io::Read::read(&mut source, &mut read_buf) {
                    Ok(0) => {
                        // EOF — flush any remaining leftover as the final chunk.
                        if !leftover.is_empty() {
                            let _ = tx_chunks.send((seq, std::mem::take(&mut leftover)));
                        }
                        break;
                    }
                    Ok(n) => n,
                    Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => {
                        // Flush any accumulated data before exiting on error.
                        if !leftover.is_empty() {
                            let _ = tx_chunks.send((seq, std::mem::take(&mut leftover)));
                        }
                        break;
                    }
                };

                // Combine leftover from previous iteration with fresh data.
                let data = if leftover.is_empty() {
                    read_buf[..n].to_vec()
                } else {
                    let mut combined = std::mem::take(&mut leftover);
                    combined.extend_from_slice(&read_buf[..n]);
                    combined
                };

                // Split at the last newline: everything up to it forms a
                // complete-line chunk; the remainder carries over.
                if let Some(last_nl) = memchr::memrchr(line_ending, &data) {
                    leftover = data[last_nl + 1..].to_vec();
                    let mut chunk = data;
                    chunk.truncate(last_nl + 1);
                    if tx_chunks.send((seq, chunk)).is_err() {
                        break;
                    }
                    seq += 1;
                } else {
                    // No newline at all — accumulate for the next read.
                    leftover = data;
                }
            }

            debug!("parallel reader: I/O thread stop (sent {seq} chunks)");
        });
    }

    /// Parses a raw byte chunk into a tagged batch of items.
    ///
    /// Shared by both the pool-based and dedicated-thread code paths.
    fn process_chunk(seq: usize, chunk: &[u8], opt: &SkimItemReaderOption) -> (usize, Vec<Arc<dyn SkimItem>>) {
        let mut items = Vec::new();
        let line_ending = opt.line_ending;

        // Chunks produced by the I/O thread end with the line-ending delimiter
        // (except possibly the final leftover at EOF).  `split()` would produce
        // a spurious trailing empty segment in that case, so we trim the
        // trailing delimiter first.  After trimming, every segment — including
        // empty ones — maps 1:1 to an input line.
        let chunk_trimmed: &[u8] = if chunk.last() == Some(&line_ending) {
            &chunk[..chunk.len() - 1]
        } else {
            chunk
        };

        for line_bytes in chunk_trimmed.split(|&b: &u8| b == line_ending) {
            // Strip optional \r for \r\n endings.
            let line_bytes: &[u8] = line_bytes.strip_suffix(b"\r").unwrap_or(line_bytes);
            let Ok(line) = std::str::from_utf8(line_bytes) else {
                continue;
            };
            // Use DefaultSkimItem::new to preserve ANSI-escape stripping
            // behaviour even when --ansi is not set.
            items.push(Arc::new(DefaultSkimItem::new(
                line,
                opt.use_ansi_color,
                &opt.transform_fields,
                &opt.matching_fields,
                &opt.delimiter,
            )) as Arc<dyn SkimItem>);
        }

        (seq, items)
    }

    /// Stage 3: receives item batches from workers and emits them through the
    /// downstream channel in the original sequence order.
    fn spawn_reorder_thread(rx_results: kanal::Receiver<(usize, Vec<Arc<dyn SkimItem>>)>, tx_item: SkimItemSender) {
        thread::spawn(move || {
            debug!("parallel reader: reorder thread start");
            let mut expected = 0usize;
            let mut pending: BTreeMap<usize, Vec<Arc<dyn SkimItem>>> = BTreeMap::new();

            while let Ok((seq, items)) = rx_results.recv() {
                pending.insert(seq, items);
                // Flush consecutive batches starting from the expected seq.
                while let Some(batch) = pending.remove(&expected) {
                    if tx_item.send(batch).is_err() {
                        return;
                    }
                    expected += 1;
                }
            }
            // Drain anything left (shouldn't normally happen).
            while let Some((&seq, _)) = pending.first_key_value() {
                if pending.remove(&seq).is_some_and(|batch| tx_item.send(batch).is_err()) {
                    return;
                }
            }
            debug!("parallel reader: reorder thread stop");
        });
    }

    /// `components_to_stop` == 0 => all the threads have been stopped
    /// return (`channel_for_receive_item`, `channel_to_stop_command`)
    fn read_and_collect_from_command(
        &self,
        components_to_stop: Arc<AtomicUsize>,
        input: CollectorInput,
    ) -> (SkimItemReceiver, crate::prelude::Sender<i32>) {
        let send_error = self.option.show_error;
        let (command, source) = match input {
            CollectorInput::Pipe(pipe) => (None, pipe),
            CollectorInput::Command(cmd) => get_command_output(&cmd, send_error).expect("command not found"),
        };

        let (tx_interrupt, rx_interrupt) = crate::prelude::bounded(8);
        let (tx_item, rx_item): (SkimItemSender, SkimItemReceiver) = crate::prelude::bounded(1024 * 1024);

        let started = Arc::new(AtomicBool::new(false));
        let started_clone = started.clone();
        let components_to_stop_clone = components_to_stop.clone();
        let tx_item_clone = tx_item.clone();
        // listening to close signal and kill command if needed
        thread::spawn(move || {
            debug!("collector: command killer start");
            components_to_stop_clone.fetch_add(1, Ordering::SeqCst);
            started_clone.store(true, Ordering::SeqCst); // notify parent that it is started

            let _ = rx_interrupt.recv();
            if let Some(mut child) = command {
                // clean up resources
                let _ = child.kill();
                let _ = child.wait();

                if send_error {
                    let has_error = child
                        .try_wait()
                        .map(|os| os.is_none_or(|s| !s.success()))
                        .unwrap_or(false);
                    if has_error {
                        trace!("collector: sending error");
                        let output = child.wait_with_output().expect("could not retrieve error message");
                        let error_text = String::from_utf8_lossy(&output.stderr).to_string();
                        let error_items: Vec<Arc<dyn SkimItem>> = error_text
                            .lines()
                            .map(|line| {
                                Arc::new(DefaultSkimItem::new(
                                    line,
                                    false,
                                    &[],
                                    &[],
                                    &Regex::new(DELIMITER_STR).unwrap(),
                                )) as Arc<dyn SkimItem>
                            })
                            .collect();
                        let _ = tx_item_clone.send(error_items);
                    }
                }
            }

            components_to_stop_clone.fetch_sub(1, Ordering::SeqCst);
            debug!("collector: command killer stop");
        });

        while !started.load(Ordering::SeqCst) {
            // busy waiting for the thread to start. (components_to_stop is added)
        }

        let tx_interrupt_clone = tx_interrupt.clone();
        let option = self.option.clone();
        let transform_fields = option.transform_fields.clone();
        let matching_fields = option.matching_fields.clone();

        // Increment before submitting so components_to_stop is already non-zero
        // when this function returns; no busy-wait needed.
        components_to_stop.fetch_add(1, Ordering::SeqCst);
        self.thread_pool.spawn(move || {
            debug!("collector: command collector start");

            Self::read_lines_into_items(source, &tx_item, &option, &transform_fields, &matching_fields);

            let _ = tx_interrupt_clone.send(1); // ensure the killer thread will exit
            components_to_stop.fetch_sub(1, Ordering::SeqCst);
            debug!("collector: command collector stop");
        });

        (rx_item, tx_interrupt)
    }
}

impl CommandCollector for SkimItemReader {
    fn invoke(
        &mut self,
        cmd: &str,
        components_to_stop: Arc<AtomicUsize>,
    ) -> (SkimItemReceiver, crate::prelude::Sender<i32>) {
        self.read_and_collect_from_command(components_to_stop, CollectorInput::Command(cmd.to_string()))
    }

    fn set_thread_pool(&mut self, pool: Arc<ThreadPool>) {
        self.thread_pool = pool;
    }
}

type CommandOutput = (Option<Child>, Box<dyn BufRead + Send>);

fn get_command_output(cmd: &str, send_error: bool) -> Result<CommandOutput, Box<dyn Error>> {
    let (reader, writer) = std::io::pipe()?;
    let mut command = crate::shell_cmd(cmd);
    command.stdout(writer.try_clone()?);
    if send_error {
        trace!("redirecting stderr to the output");
        command.stderr(writer);
    } else {
        command.stderr(Stdio::null());
    }

    Ok((command.spawn().ok(), Box::new(BufReader::new(reader))))
}
