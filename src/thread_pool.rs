//! A lightweight thread pool with a shared work queue.
//!
//! Worker threads pick up the next available job as soon as they finish their
//! current one, giving natural load balancing without work-stealing.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

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
    /// # Panics
    ///
    /// Panics if the internal job-queue mutex is poisoned.
    pub fn spawn<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let mut queue = self.shared.queue.lock().unwrap();
        queue.jobs.push_back(Box::new(f));
        // Wake one waiting worker.
        self.shared.job_available.notify_one();
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

/// Processes `items` in parallel across `num_workers` threads from the given
/// pool, then returns the combined result.
///
/// The work is split into chunks of `chunk_size`.  Each worker thread
/// repeatedly grabs the next available chunk (via an atomic counter), runs
/// `process_chunk` on it, and immediately merges the partial result into a
/// shared accumulator using `reduce`.  Once all chunks are processed the
/// accumulator is returned.
///
/// Because each worker picks up the *next* chunk as soon as it finishes the
/// previous one (including the reduce step), faster threads naturally do more
/// work without any explicit work-stealing.
///
/// # Parameters
///
/// * `pool`           – thread pool whose workers will execute the chunks.
/// * `num_workers`    – how many workers to dispatch (capped to pool size internally by caller).
/// * `items`          – the data to process; shared read-only across workers via `Arc`.
/// * `chunk_size`     – number of items per chunk.
/// * `identity`       – the identity/seed value for the accumulator (called once per worker and once for init).
/// * `process_chunk`  – `(chunk_start_index, &[T]) -> R` – processes one chunk.
/// * `reduce`         – merges a per-chunk result into the accumulator (`&mut acc, partial`).
///
/// # Panics
///
/// Panics if the internal accumulator mutex is poisoned.
pub fn parallel_work_queue<T, R, P, M, I>(
    pool: &ThreadPool,
    num_workers: usize,
    items: &Arc<[T]>,
    chunk_size: usize,
    identity: I,
    process_chunk: P,
    reduce: M,
) -> R
where
    T: Send + Sync + 'static,
    R: Send + 'static,
    P: Fn(usize, &[T]) -> R + Send + Sync + 'static,
    M: Fn(&mut R, R) + Send + Sync + 'static,
    I: Fn() -> R + Send + Sync + 'static,
{
    let total = items.len();
    if total == 0 {
        return identity();
    }

    let num_chunks = total.div_ceil(chunk_size);

    // Shared atomic counter – workers fetch-add to grab the next chunk index.
    let next_chunk = Arc::new(AtomicUsize::new(0));

    // Shared accumulator protected by a mutex.
    let accumulator: Arc<Mutex<R>> = Arc::new(Mutex::new(identity()));

    // Barrier: we wait until all workers have finished.
    let remaining = Arc::new(AtomicCounter::new(num_workers));

    let process_chunk = Arc::new(process_chunk);
    let reduce = Arc::new(reduce);
    let identity = Arc::new(identity);

    for _ in 0..num_workers {
        let w_items = Arc::clone(items);
        let w_next_chunk = Arc::clone(&next_chunk);
        let w_accumulator = Arc::clone(&accumulator);
        let w_remaining = Arc::clone(&remaining);
        let w_process_chunk = Arc::clone(&process_chunk);
        let w_reduce = Arc::clone(&reduce);
        let w_identity = Arc::clone(&identity);

        pool.spawn(move || {
            // Each worker keeps a local accumulator to batch up results and
            // reduce lock contention on the shared accumulator.
            let mut local_acc = w_identity();

            loop {
                let chunk_idx = w_next_chunk.fetch_add(1, Ordering::Relaxed);
                if chunk_idx >= num_chunks {
                    break;
                }

                let start = chunk_idx * chunk_size;
                let end = total.min(start + chunk_size);
                let partial = w_process_chunk(start, &w_items[start..end]);
                w_reduce(&mut local_acc, partial);
            }

            // Merge local accumulator into the shared one.
            {
                let mut shared_acc = w_accumulator.lock().unwrap();
                w_reduce(&mut *shared_acc, local_acc);
            }

            // Drop all Arc clones *before* signaling completion so the
            // coordinator can safely `Arc::try_unwrap` the accumulator.
            drop(w_accumulator);
            drop(w_items);
            drop(w_next_chunk);
            drop(w_process_chunk);
            drop(w_reduce);
            drop(w_identity);

            // Signal completion.
            w_remaining.dec_and_notify();
        });
    }

    // Block until all workers are done.
    remaining.wait_for_zero();

    // Extract the final accumulator value.
    Arc::try_unwrap(accumulator)
        .ok()
        .expect("all workers finished; Arc should be unique")
        .into_inner()
        .unwrap()
}

// ---------------------------------------------------------------------------
// AtomicCounter with parking (avoids spinning in the coordinator)
// ---------------------------------------------------------------------------

struct AtomicCounter {
    state: Mutex<usize>,
    done: Condvar,
}

impl AtomicCounter {
    fn new(n: usize) -> Self {
        Self {
            state: Mutex::new(n),
            done: Condvar::new(),
        }
    }

    /// Decrements the counter by one and notifies waiters if it reaches zero.
    fn dec_and_notify(&self) {
        let mut count = self.state.lock().unwrap();
        *count = count.saturating_sub(1);
        if *count == 0 {
            self.done.notify_all();
        }
    }

    /// Blocks until the counter reaches zero.
    fn wait_for_zero(&self) {
        let mut count = self.state.lock().unwrap();
        while *count > 0 {
            count = self.done.wait(count).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parallel_work_queue_sums() {
        let pool = ThreadPool::new(4);
        let items: Arc<[u64]> = (1..=1000u64).collect::<Vec<_>>().into();
        let result = parallel_work_queue(
            &pool,
            4,
            &items,
            64,
            || 0u64,
            |_start, chunk| chunk.iter().sum::<u64>(),
            |acc, partial| *acc += partial,
        );
        assert_eq!(result, 500_500);
    }

    #[test]
    fn parallel_work_queue_empty() {
        let pool = ThreadPool::new(2);
        let items: Arc<[u64]> = Arc::from(Vec::<u64>::new().into_boxed_slice());
        let result = parallel_work_queue(
            &pool,
            2,
            &items,
            64,
            Vec::<u64>::new,
            |_start, chunk| chunk.to_vec(),
            |acc, mut partial| acc.append(&mut partial),
        );
        assert!(result.is_empty());
    }

    #[test]
    fn parallel_work_queue_single_thread() {
        let pool = ThreadPool::new(1);
        let items: Arc<[i32]> = (0..100i32).collect::<Vec<_>>().into();
        let result = parallel_work_queue(
            &pool,
            1,
            &items,
            10,
            || 0i32,
            |_start, chunk| chunk.iter().sum::<i32>(),
            |acc, partial| *acc += partial,
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
}
