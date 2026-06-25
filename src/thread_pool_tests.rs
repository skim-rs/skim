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
    let (tx, rx) = std::sync::mpsc::channel();
    pool.spawn(move || {
        flag2.store(42, Ordering::SeqCst);
        let _ = tx.send(());
    });
    rx.recv_timeout(std::time::Duration::from_secs(5))
        .expect("closure did not complete");
    assert_eq!(flag.load(Ordering::SeqCst), 42);
}

#[test]
fn spawn_batch_runs_all() {
    let pool = ThreadPool::new(4);
    let counter = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = std::sync::mpsc::channel();
    let jobs: Vec<Box<dyn FnOnce() + Send + 'static>> = (0..10)
        .map(|_| {
            let c = Arc::clone(&counter);
            let tx = tx.clone();
            let job: Box<dyn FnOnce() + Send + 'static> = Box::new(move || {
                c.fetch_add(1, Ordering::SeqCst);
                let _ = tx.send(());
            });
            job
        })
        .collect();
    pool.spawn_batch(jobs);
    for _ in 0..10 {
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .expect("job did not complete");
    }
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
