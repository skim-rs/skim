//! A lightweight thread pool with a shared work queue.
//!
//! Worker threads pick up the next available job as soon as they finish their
//! current one, giving natural load balancing without work-stealing.

use std::cell::UnsafeCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

// ---------------------------------------------------------------------------
// Thread-count partitioning
// ---------------------------------------------------------------------------

/// Splits `n` logical threads between the reader pipeline and the matcher.
///
/// Returns `(reader, matcher)` where:
/// - `reader`  = ⌈n / 3⌉, minimum 1
/// - `matcher` = ⌊2n / 3⌋, minimum 1
///
/// On single-core machines (`n = 1`) both values are 1 so neither subsystem
/// starves; the OS time-slices between the two small pools as usual.
#[must_use]
pub fn partition_threads(n: usize) -> (usize, usize) {
    let reader = n.div_ceil(3); // ⌈n/3⌉
    let matcher = (2 * n) / 3; // ⌊2n/3⌋
    (reader.max(1), matcher.max(1))
}

// ---------------------------------------------------------------------------
// Job type
// ---------------------------------------------------------------------------

type Job = Box<dyn FnOnce() + Send + 'static>;

// ---------------------------------------------------------------------------
// Shared state between the pool handle and workers
// ---------------------------------------------------------------------------

struct SharedState {
    queue: Mutex<QueueInner>,
    /// Wakes workers when a new job is enqueued or shutdown is requested.
    job_available: Condvar,
}

struct QueueInner {
    jobs: VecDeque<Job>,
    shutdown: bool,
}

// ---------------------------------------------------------------------------
// ThreadPool
// ---------------------------------------------------------------------------

/// A simple thread pool backed by a FIFO work queue.
///
/// Each worker thread blocks on a shared condvar and picks up the next
/// available job as soon as it becomes idle.  This means that when a thread
/// finishes a job (including any reduce/merge work that was part of that job)
/// it immediately checks for the next queued job without any extra
/// coordination.
pub struct ThreadPool {
    shared: Arc<SharedState>,
    workers: Vec<thread::JoinHandle<()>>,
    num_threads: usize,
}

impl ThreadPool {
    /// Creates a new pool with `num_threads` worker threads.
    ///
    /// # Panics
    ///
    /// Panics if `num_threads` is 0.
    #[must_use]
    pub fn new(num_threads: usize) -> Self {
        assert!(num_threads > 0, "ThreadPool requires at least 1 thread");

        let shared = Arc::new(SharedState {
            queue: Mutex::new(QueueInner {
                jobs: VecDeque::new(),
                shutdown: false,
            }),
            job_available: Condvar::new(),
        });

        let mut workers = Vec::with_capacity(num_threads);

        for _ in 0..num_threads {
            let worker_shared = Arc::clone(&shared);
            workers.push(thread::spawn(move || worker_loop(&worker_shared)));
        }

        Self {
            shared,
            workers,
            num_threads,
        }
    }

    /// Returns the number of worker threads in the pool.
    #[inline]
    #[must_use]
    pub fn num_threads(&self) -> usize {
        self.num_threads
    }

    /// Submits a closure to be executed by the next available worker.
    ///
    /// The lock is dropped *before* notifying the condvar so that the woken
    /// worker can acquire it immediately instead of blocking on the notifier.
    ///
    /// # Panics
    ///
    /// Panics if the internal job-queue mutex is poisoned.
    pub fn spawn<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        {
            let mut queue = self.shared.queue.lock().unwrap();
            queue.jobs.push_back(Box::new(f));
        } // lock dropped before notify
        self.shared.job_available.notify_one();
    }

    /// Submits multiple closures in a single lock acquisition, then wakes
    /// exactly as many workers as there are new jobs (capped at the pool size).
    /// This avoids unnecessary wakes when fewer jobs than workers are
    /// submitted.
    ///
    /// # Panics
    ///
    /// Panics if the internal job-queue mutex is poisoned.
    pub fn spawn_batch<I>(&self, jobs: I)
    where
        I: IntoIterator<Item = Box<dyn FnOnce() + Send + 'static>>,
    {
        let count = {
            let mut queue = self.shared.queue.lock().unwrap();
            let before = queue.jobs.len();
            for job in jobs {
                queue.jobs.push_back(job);
            }
            queue.jobs.len() - before
        }; // lock dropped before notify
        if count >= self.num_threads {
            self.shared.job_available.notify_all();
        } else {
            for _ in 0..count {
                self.shared.job_available.notify_one();
            }
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        // Signal shutdown.
        {
            let mut queue = self.shared.queue.lock().unwrap();
            queue.shutdown = true;
        }
        self.shared.job_available.notify_all();

        // Join all workers (ignore panics from individual threads).
        for handle in self.workers.drain(..) {
            let _ = handle.join();
        }
    }
}

// ---------------------------------------------------------------------------
// Worker loop
// ---------------------------------------------------------------------------

fn worker_loop(shared: &SharedState) {
    loop {
        let next_job = {
            let mut queue = shared.queue.lock().unwrap();
            loop {
                if let Some(ready) = queue.jobs.pop_front() {
                    break Some(ready);
                }
                if queue.shutdown {
                    break None;
                }
                queue = shared.job_available.wait(queue).unwrap();
            }
        };

        match next_job {
            Some(runnable) => runnable(),
            None => return, // shutdown
        }
    }
}

// ---------------------------------------------------------------------------
// Parallel work-queue helpers used by the matcher
// ---------------------------------------------------------------------------
// Cache-line–aligned result slot
// ---------------------------------------------------------------------------

/// A single worker's result slot, padded to a full cache line so that
/// concurrent writes to adjacent slots by different cores don't cause
/// false sharing.
///
/// Alignment is platform-dependant, this is taken from <https://docs.rs/crossbeam-utils/0.8.21/src/crossbeam_utils/cache_padded.rs.html>
///
// Starting from Intel's Sandy Bridge, spatial prefetcher is now pulling pairs of 64-byte cache
// lines at a time, so we have to align to 128 bytes rather than 64.
#[cfg_attr(
    any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "arm64ec",
        target_arch = "powerpc64",
    ),
    repr(align(128))
)]
// arm, mips, mips64, sparc, and hexagon have 32-byte cache line size.
#[cfg_attr(
    any(
        target_arch = "arm",
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "sparc",
        target_arch = "hexagon",
    ),
    repr(align(32))
)]
// m68k has 16-byte cache line size.
#[cfg_attr(target_arch = "m68k", repr(align(16)))]
// s390x has 256-byte cache line size.
#[cfg_attr(target_arch = "s390x", repr(align(256)))]
// x86, wasm, riscv, and sparc64 have 64-byte cache line size.
// All others are assumed to have 64-byte cache line size.
#[cfg_attr(
    not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "arm64ec",
        target_arch = "powerpc64",
        target_arch = "arm",
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "sparc",
        target_arch = "hexagon",
        target_arch = "m68k",
        target_arch = "s390x",
    )),
    repr(align(64))
)]
struct Slot<R> {
    value: UnsafeCell<Option<R>>,
}

// SAFETY: each slot is written by exactly one worker (unique `worker_id`)
// and read by the coordinator only after the barrier guarantees all writes
// are visible.  No two threads ever access the same slot concurrently.
unsafe impl<R: Send> Send for Slot<R> {}
unsafe impl<R: Send> Sync for Slot<R> {}

impl<R> Slot<R> {
    fn new() -> Self {
        Self {
            value: UnsafeCell::new(None),
        }
    }
}

// ---------------------------------------------------------------------------

/// Processes `items` in parallel across `num_workers` threads from the given
/// pool, then hands the per-worker results to `merge`.
///
/// The work is split into chunks of `chunk_size`.  Each worker thread
/// repeatedly grabs the next available chunk (via an atomic counter), runs
/// `process_chunk` on it, and folds the partial result into a *local*
/// accumulator using `reduce`.  When all chunks are consumed, each worker
/// calls `prepare` on its local accumulator (e.g. to sort it) — this step
/// runs **in parallel** across all workers — and then writes the prepared
/// result into its slot.  The coordinator collects every worker's result and
/// passes them all to `merge` in a single call.
///
/// Because each worker picks up the *next* chunk as soon as it finishes the
/// previous one, faster threads naturally do more work without any explicit
/// work-stealing.
///
/// # Parameters
///
/// * `pool`           – thread pool whose workers will execute the chunks.
/// * `num_workers`    – how many workers to dispatch (capped to pool size internally by caller).
/// * `items`          – the data to process; shared read-only across workers via `Arc`.
/// * `chunk_size`     – number of items per chunk.
/// * `identity`       – the identity/seed value for per-worker local accumulators (called once per worker).
/// * `process_chunk`  – `(chunk_start_index, &[T]) -> R` – processes one chunk.
/// * `reduce`         – folds a per-chunk result into a worker-local accumulator (`&mut acc, partial`).
/// * `prepare`        – called on each worker's finished accumulator **on the worker thread** (runs in parallel).  Use this for expensive per-worker work like sorting.
/// * `merge`          – called once on the coordinator with all per-worker results.
#[allow(clippy::too_many_arguments)]
pub fn parallel_work_queue<S, T, R, P, M, I, W, G>(
    pool: &ThreadPool,
    num_workers: usize,
    items: &Arc<S>,
    chunk_size: usize,
    identity: I,
    process_chunk: P,
    reduce: M,
    prepare: W,
    merge: G,
) where
    S: AsRef<[T]> + Send + Sync + ?Sized + 'static,
    T: Send + Sync + 'static,
    R: Send + 'static,
    P: Fn(usize, &[T]) -> R + Send + Sync + 'static,
    M: Fn(&mut R, R) + Send + Sync + 'static,
    I: Fn() -> R + Send + Sync + 'static,
    W: Fn(&mut R) + Send + Sync + 'static,
    G: FnOnce(Vec<R>),
{
    let items_slice: &[T] = AsRef::<[T]>::as_ref(&**items);
    let total = items_slice.len();
    if total == 0 {
        merge(Vec::new());
        return;
    }

    let num_chunks = total.div_ceil(chunk_size);

    // Shared atomic counter – workers fetch-add to grab the next chunk index.
    let next_chunk = Arc::new(AtomicUsize::new(0));

    // Contiguous, cache-line-aligned per-worker result slots.  Each worker
    // writes only to its own slot (lock-free via UnsafeCell); the
    // coordinator reads after the AtomicCounter barrier.
    let slots: Arc<Vec<Slot<R>>> = Arc::new((0..num_workers).map(|_| Slot::new()).collect());

    // Barrier: we wait until all workers have finished.
    let remaining = Arc::new(AtomicCounter::new(num_workers));
    // Register the coordinator thread so workers can unpark it.
    remaining.set_waiter();

    let process_chunk = Arc::new(process_chunk);
    let reduce = Arc::new(reduce);
    let prepare = Arc::new(prepare);
    let identity = Arc::new(identity);

    // Build all jobs up-front and submit them in a single batch to minimise
    // lock acquisitions on the work queue.
    let jobs: Vec<Box<dyn FnOnce() + Send + 'static>> = (0..num_workers)
        .map(|worker_id| {
            let w_items = Arc::clone(items);
            let w_next_chunk = Arc::clone(&next_chunk);
            let w_slots: Arc<Vec<Slot<R>>> = Arc::clone(&slots);
            let w_remaining = Arc::clone(&remaining);
            let w_process_chunk = Arc::clone(&process_chunk);
            let w_reduce = Arc::clone(&reduce);
            let w_prepare = Arc::clone(&prepare);
            let w_identity = Arc::clone(&identity);

            let job: Box<dyn FnOnce() + Send + 'static> = Box::new(move || {
                // Scope all Arc-holding work so clones are dropped before we
                // signal completion.  This lets the coordinator safely unwrap
                // the outer Arcs.
                let local_acc = {
                    let mut local_acc = w_identity();

                    loop {
                        let chunk_idx = w_next_chunk.fetch_add(1, Ordering::Relaxed);
                        if chunk_idx >= num_chunks {
                            break;
                        }

                        let start = chunk_idx * chunk_size;
                        let end = total.min(start + chunk_size);
                        let slice: &[T] = AsRef::<[T]>::as_ref(&*w_items);
                        let partial = w_process_chunk(start, &slice[start..end]);
                        w_reduce(&mut local_acc, partial);
                    }

                    // Run prepare (e.g. sort) while still on the worker thread
                    // so that this work happens in parallel across workers.
                    w_prepare(&mut local_acc);

                    // w_items, w_next_chunk, w_process_chunk, w_reduce,
                    // w_prepare, w_identity are dropped when this block ends.
                    local_acc
                };

                // Write into our own slot – lock-free, no contention.
                // SAFETY: each worker_id is unique; no other thread writes to
                // this slot, and the coordinator reads only after the barrier.
                unsafe { *w_slots[worker_id].value.get() = Some(local_acc) };

                // Drop the slots Arc *before* signalling completion.
                // The coordinator calls Arc::into_inner(slots) after wait_for_zero
                // returns; that requires the strong count to be exactly 1.  Without
                // this explicit drop, w_slots would still be alive in the closure
                // frame when dec_and_notify wakes the coordinator, causing
                // Arc::into_inner to spuriously return None and silently lose results.
                drop(w_slots);

                // Signal completion.
                w_remaining.dec_and_notify();
            });
            job
        })
        .collect();

    pool.spawn_batch(jobs);

    // Block until all workers are done.
    remaining.wait_for_zero();

    // Collect per-worker results and hand them to `merge` in one call.
    // Workers dropped their `w_slots` Arc clone explicitly before signalling
    // completion, so we are the sole owner here.
    if let Some(slots) = Arc::into_inner(slots) {
        let results: Vec<R> = slots.into_iter().filter_map(|slot| slot.value.into_inner()).collect();

        merge(results);
    } else {
        log::error!("More than one ref to the slots remaining after workers exit. This SHOULD NOT happen.");
    }
}

// ---------------------------------------------------------------------------
// AtomicCounter with parking (avoids spinning in the coordinator)
// ---------------------------------------------------------------------------

struct AtomicCounter {
    count: AtomicUsize,
    /// The thread that called `wait_for_zero`.  Workers unpark it when the
    /// count reaches zero.  Set once by `wait_for_zero` before any worker can
    /// finish, so plain `Relaxed` loads inside `dec_and_notify` are fine
    /// (the `fetch_sub` with `AcqRel` provides the necessary ordering).
    waiter: UnsafeCell<Option<thread::Thread>>,
}

// SAFETY: `waiter` is written exactly once (by the coordinator in
// `set_waiter`, before any worker can observe it via `dec_and_notify`)
// and read by workers only after that write is visible (guaranteed by the
// `AcqRel` ordering on the atomic counter operations).
unsafe impl Send for AtomicCounter {}
unsafe impl Sync for AtomicCounter {}

impl AtomicCounter {
    fn new(n: usize) -> Self {
        Self {
            count: AtomicUsize::new(n),
            waiter: UnsafeCell::new(None),
        }
    }

    /// Register the current thread as the waiter.
    ///
    /// Must be called exactly once, before any worker calls `dec_and_notify`.
    fn set_waiter(&self) {
        // SAFETY: called once by the coordinator before workers start.
        unsafe { *self.waiter.get() = Some(thread::current()) };
    }

    /// Decrements the counter by one and unparks the waiter if it reaches zero.
    ///
    /// # Panics (debug only)
    ///
    /// Debug-asserts that the counter has not already reached zero, which
    /// would indicate a double-decrement bug.
    fn dec_and_notify(&self) {
        let prev = self.count.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(prev > 0, "AtomicCounter decremented below zero — double-decrement bug?");
        if prev == 1 {
            // We just decremented from 1 → 0.
            // SAFETY: waiter was set before workers were dispatched.
            if let Some(t) = unsafe { &*self.waiter.get() } {
                t.unpark();
            }
        }
    }

    /// Blocks until the counter reaches zero.
    fn wait_for_zero(&self) {
        while self.count.load(Ordering::Acquire) > 0 {
            thread::park();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_threads_split() {
        // Single-core: both pools get at least 1 thread.
        assert_eq!(partition_threads(1), (1, 1));
        // Two cores: 1 reader, 1 matcher.
        assert_eq!(partition_threads(2), (1, 1));
        // Three cores: 1 reader, 2 matcher.
        assert_eq!(partition_threads(3), (1, 2));
        // Six cores: 2 reader, 4 matcher.
        assert_eq!(partition_threads(6), (2, 4));
        // Eight cores: 3 reader, 5 matcher; sums to 8.
        assert_eq!(partition_threads(8), (3, 5));
        // Nine cores: 3 reader, 6 matcher; sums to 9.
        assert_eq!(partition_threads(9), (3, 6));
        // The two values always sum to n for n >= 3.
        for n in 3..=64 {
            let (r, m) = partition_threads(n);
            assert_eq!(r + m, n, "partition_threads({n}) = ({r}, {m}) does not sum to {n}");
            assert!(r >= 1);
            assert!(m >= 1);
        }
    }

    #[test]
    fn spawn_runs_closure() {
        let pool = ThreadPool::new(2);
        let flag = Arc::new(AtomicUsize::new(0));
        let flag2 = Arc::clone(&flag);
        pool.spawn(move || {
            flag2.store(42, Ordering::SeqCst);
        });
        // Give it a moment.
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!(flag.load(Ordering::SeqCst), 42);
    }

    #[test]
    fn spawn_batch_runs_all() {
        let pool = ThreadPool::new(4);
        let counter = Arc::new(AtomicUsize::new(0));
        let jobs: Vec<Box<dyn FnOnce() + Send + 'static>> = (0..10)
            .map(|_| {
                let c = Arc::clone(&counter);
                let job: Box<dyn FnOnce() + Send + 'static> = Box::new(move || {
                    c.fetch_add(1, Ordering::SeqCst);
                });
                job
            })
            .collect();
        pool.spawn_batch(jobs);
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn parallel_work_queue_sums() {
        let pool = ThreadPool::new(4);
        let items: Arc<[u64]> = (1..=1000u64).collect::<Vec<_>>().into();
        let mut result = 0u64;
        parallel_work_queue(
            &pool,
            4,
            &items,
            64,
            || 0u64,
            |_start, chunk| chunk.iter().sum::<u64>(),
            |acc, partial| *acc += partial,
            |_| {},
            |worker_results| {
                for partial in worker_results {
                    result += partial;
                }
            },
        );
        assert_eq!(result, 500_500);
    }

    #[test]
    fn parallel_work_queue_empty() {
        let pool = ThreadPool::new(2);
        let items: Arc<[u64]> = Arc::from(Vec::<u64>::new().into_boxed_slice());
        let mut result = Vec::<u64>::new();
        parallel_work_queue(
            &pool,
            2,
            &items,
            64,
            Vec::<u64>::new,
            |_start, chunk| chunk.to_vec(),
            |acc, mut partial| acc.append(&mut partial),
            |_| {},
            |worker_results| {
                for partial in worker_results {
                    result.extend(partial);
                }
            },
        );
        assert!(result.is_empty());
    }

    #[test]
    fn parallel_work_queue_single_thread() {
        let pool = ThreadPool::new(1);
        let items: Arc<[i32]> = (0..100i32).collect::<Vec<_>>().into();
        let mut result = 0i32;
        parallel_work_queue(
            &pool,
            1,
            &items,
            10,
            || 0i32,
            |_start, chunk| chunk.iter().sum::<i32>(),
            |acc, partial| *acc += partial,
            |_| {},
            |worker_results| {
                for partial in worker_results {
                    result += partial;
                }
            },
        );
        assert_eq!(result, (0..100).sum::<i32>());
    }

    #[test]
    fn pool_drop_joins_threads() {
        let flag = Arc::new(AtomicUsize::new(0));
        {
            let pool = ThreadPool::new(2);
            let f = Arc::clone(&flag);
            pool.spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(30));
                f.store(1, Ordering::SeqCst);
            });
        } // pool dropped here – should join
        assert_eq!(flag.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn parallel_work_queue_many_workers_few_chunks() {
        // More workers than chunks — extra workers should gracefully no-op.
        let pool = ThreadPool::new(8);
        let items: Arc<[u64]> = (1..=10u64).collect::<Vec<_>>().into();
        let mut result = 0u64;
        parallel_work_queue(
            &pool,
            8,
            &items,
            5,
            || 0u64,
            |_start, chunk| chunk.iter().sum::<u64>(),
            |acc, partial| *acc += partial,
            |_| {},
            |worker_results| {
                for partial in worker_results {
                    result += partial;
                }
            },
        );
        assert_eq!(result, 55);
    }

    #[test]
    fn parallel_work_queue_single_thread_pool_no_deadlock() {
        // With a 1-thread pool the coordinator must NOT be a pool job —
        // it runs on a dedicated OS thread and submits all worker jobs to
        // the pool.  This ensures the single pool thread is always free to
        // run those workers and no deadlock can occur.
        let (tx, rx) = std::sync::mpsc::channel();
        let pool = Arc::new(ThreadPool::new(1));
        let items: Arc<[u64]> = (1..=100u64).collect::<Vec<_>>().into();
        let pool_coord = Arc::clone(&pool);
        // Coordinator is a dedicated thread, not a pool job.
        std::thread::spawn(move || {
            parallel_work_queue(
                &pool_coord,
                1,
                &items,
                10,
                || 0u64,
                |_start, chunk| chunk.iter().sum::<u64>(),
                |acc, partial| *acc += partial,
                |_| {},
                |worker_results| {
                    let _ = tx.send(worker_results.into_iter().sum::<u64>());
                },
            );
        });
        let result = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("deadlock or timeout");
        assert_eq!(result, 5050);
    }
}
