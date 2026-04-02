use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Cursor, Stderr};
use std::time::Duration;

use clap::Parser as _;
use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};

use ratatui::backend::TestBackend;
use ratatui::prelude::CrosstermBackend;
use skim::prelude::*;

/// Small inline fixture — fast to load, good for latency benchmarks.
const SMALL_ITEMS: &[&str] = &[
    "src/main.rs",
    "src/lib.rs",
    "src/options.rs",
    "src/skim.rs",
    "benches/partial.rs",
    "tests/common/insta.rs",
    "Cargo.toml",
    "README.md",
];

/// Path to the medium fixture shipped with the repo (≈664 lines).
const FIXTURE_DEFAULT: &str = "benches/fixtures/default.txt";

/// Path to the large fixture (100 000 lines).  Skip in low-time CI runs if absent.
const FIXTURE_100K: &str = "benches/fixtures/100K.txt";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read a fixture file into memory and return each non-empty line as a `String`.
fn load_fixture(path: &str) -> Vec<String> {
    let f = File::open(path).unwrap_or_else(|e| panic!("cannot open fixture {path}: {e}"));
    BufReader::new(f)
        .lines()
        .map_while(Result::ok)
        .filter(|l| !l.is_empty())
        .collect()
}

/// Build a newline-separated `Vec<u8>` from a slice of items.
fn items_to_bytes(items: &[impl AsRef<str>]) -> Vec<u8> {
    let mut buf = Vec::new();
    for item in items {
        buf.extend_from_slice(item.as_ref().as_bytes());
        buf.push(b'\n');
    }
    buf
}

/// Create a `SkimItemReceiver` that will produce `items` via the full
/// `SkimItemReader` pipeline (the same path used when skim reads stdin).
fn make_receiver(items: &[impl AsRef<str>], options: &SkimOptions) -> SkimItemReceiver {
    let reader_opts = SkimItemReaderOption::from_options(options);
    let item_reader = SkimItemReader::new(reader_opts);
    item_reader.of_bufread(Cursor::new(items_to_bytes(items)))
}

/// Spin-wait until the reader is done and the matcher has stopped (mirrors the
/// logic in `tests/common/insta.rs`).  Panics after `timeout`.
fn wait_until_done(skim: &mut Skim<TestBackend>, timeout: Duration) {
    let start = std::time::Instant::now();
    while !skim.reader_done() {
        assert!(start.elapsed() < timeout, "timeout waiting for reader");
        skim.check_reader();
        std::thread::sleep(Duration::from_millis(1));
    }
    skim.check_reader();
    while !skim.app().matcher_control.stopped() {
        assert!(start.elapsed() < timeout, "timeout waiting for matcher");
        std::thread::sleep(Duration::from_millis(1));
    }
}

// ---------------------------------------------------------------------------
// Benchmark group
// ---------------------------------------------------------------------------

fn criterion_benchmark(c: &mut Criterion) {
    // -----------------------------------------------------------------------
    // Phase 0 — Options
    // -----------------------------------------------------------------------

    c.bench_function("parse_options", |b| {
        b.iter(|| SkimOptions::parse_from(Vec::<&str>::new()));
    });

    // `from_env` merges SKIM_DEFAULT_OPTIONS, SKIM_OPTIONS_FILE, and argv.
    // In a clean test environment it should behave identically to parse_from,
    // but the code path is different and may regress independently.
    c.bench_function("options_from_env", |b| {
        b.iter(SkimOptions::from_env);
    });

    // `build()` post-processes raw options: expands keymaps, normalises
    // tiebreak / layout, reads history files, etc.
    c.bench_function("options_build", |b| {
        b.iter_batched(
            || SkimOptions::parse_from(Vec::<&str>::new()),
            |opts| opts.build(),
            BatchSize::SmallInput,
        );
    });

    // -----------------------------------------------------------------------
    // Phase 1 — SkimItemReader pipeline construction
    // -----------------------------------------------------------------------

    // Building the reader option struct from a SkimOptions (called in sk_main
    // before Skim::init).
    c.bench_function("item_reader_option_from_options", |b| {
        let opts = SkimOptions::default().build();
        b.iter(|| SkimItemReaderOption::from_options(&opts));
    });

    // Constructing a new SkimItemReader (spawns a thread-pool).
    c.bench_function("item_reader_new", |b| {
        b.iter_batched(
            || {
                let opts = SkimOptions::default().build();
                SkimItemReaderOption::from_options(&opts)
            },
            SkimItemReader::new,
            BatchSize::SmallInput,
        );
    });

    // `of_bufread` starts the I/O dispatcher and pool threads but does NOT
    // wait for all items to be processed; it just returns the receiver.
    // Benchmark with the default fixture to get a realistic buffer.
    c.bench_function("of_bufread_setup", |b| {
        let opts = SkimOptions::default().build();
        let data = items_to_bytes(&load_fixture(FIXTURE_DEFAULT));
        b.iter_batched(
            || {
                let reader_opts = SkimItemReaderOption::from_options(&opts);
                (SkimItemReader::new(reader_opts), data.clone())
            },
            |(reader, bytes)| {
                // Calling of_bufread starts the background threads; we
                // deliberately drop the receiver immediately so they clean up.
                let _rx = reader.of_bufread(Cursor::new(bytes));
            },
            BatchSize::SmallInput,
        );
    });

    // -----------------------------------------------------------------------
    // Phase 2 — Skim::init  (unchanged from before, kept for comparison)
    // -----------------------------------------------------------------------

    c.bench_function("init", |b| {
        b.iter_batched(
            || SkimOptions::default().build(),
            |options: SkimOptions| Skim::<CrosstermBackend<BufWriter<Stderr>>>::init(options, None),
            BatchSize::SmallInput,
        );
    });

    c.bench_function("init_with_source", |b| {
        b.iter_batched(
            || {
                let (_tx, rx) = bounded(8);
                (SkimOptions::default().build(), rx)
            },
            |input: (SkimOptions, SkimItemReceiver)| {
                Skim::<CrosstermBackend<BufWriter<Stderr>>>::init(input.0, Some(input.1))
            },
            BatchSize::SmallInput,
        );
    });

    // -----------------------------------------------------------------------
    // Phase 2+3 — Skim::init + start
    // -----------------------------------------------------------------------

    c.bench_function("start", |b| {
        b.iter_batched(
            || Skim::<CrosstermBackend<BufWriter<Stderr>>>::init(SkimOptions::default().build(), None).unwrap(),
            |mut skim: Skim| skim.start(),
            BatchSize::SmallInput,
        );
    });

    // -----------------------------------------------------------------------
    // Phase 3+4 — ingest + match with realistic data
    // -----------------------------------------------------------------------
    // These benchmarks measure end-to-end reader+matcher throughput: time from
    // start() until all items are ingested and the matcher has finished its
    // first pass.  They are the closest in-process analog to the CLI startup
    // time measured by `cargo bench --bench cli`.

    // Small inline fixture (8 items) — baseline latency floor.
    c.bench_function("ingest_and_match_small", |b| {
        b.iter_batched(
            || {
                let opts = SkimOptions::default().build();
                let rx = make_receiver(SMALL_ITEMS, &opts);
                Skim::<TestBackend>::init(opts, Some(rx)).unwrap()
            },
            |mut skim: Skim<TestBackend>| {
                skim.start();
                wait_until_done(&mut skim, Duration::from_secs(5));
            },
            BatchSize::SmallInput,
        );
    });

    // Medium fixture (≈664 lines from FIXTURE_DEFAULT).
    {
        let items = load_fixture(FIXTURE_DEFAULT);
        let n = items.len() as u64;
        let mut group = c.benchmark_group("ingest_and_match_default_fixture");
        group.throughput(Throughput::Elements(n));
        group.bench_function("ingest_and_match", |b| {
            b.iter_batched(
                || {
                    let opts = SkimOptions::default().build();
                    let rx = make_receiver(&items, &opts);
                    Skim::<TestBackend>::init(opts, Some(rx)).unwrap()
                },
                |mut skim: Skim<TestBackend>| {
                    skim.start();
                    wait_until_done(&mut skim, Duration::from_secs(10));
                },
                BatchSize::SmallInput,
            );
        });
        group.finish();
    }

    // Large fixture (100 000 lines).  Only run when the file exists so the
    // bench suite does not fail in environments without the fixture.
    if std::path::Path::new(FIXTURE_100K).exists() {
        let items = load_fixture(FIXTURE_100K);
        let n = items.len() as u64;
        let mut group = c.benchmark_group("ingest_and_match_100k_fixture");
        group.throughput(Throughput::Elements(n));
        // Fewer samples: each iteration loads 100 k items.
        group.sample_size(10);
        group.bench_function("ingest_and_match", |b| {
            b.iter_batched(
                || {
                    let opts = SkimOptions::default().build();
                    let rx = make_receiver(&items, &opts);
                    Skim::<TestBackend>::init(opts, Some(rx)).unwrap()
                },
                |mut skim: Skim<TestBackend>| {
                    skim.start();
                    wait_until_done(&mut skim, Duration::from_secs(30));
                },
                BatchSize::SmallInput,
            );
        });
        group.finish();
    }

    // -----------------------------------------------------------------------
    // Phase 2–5 — full_setup  (unchanged from before, kept for comparison)
    // -----------------------------------------------------------------------

    c.bench_function("full_setup", |b| {
        b.iter(|| {
            let mut options = SkimOptions::default().build();
            if let Some(ref filter_query) = options.filter
                && options.query.is_none()
            {
                options.query = Some(filter_query.clone());
            }
            let mut skim = Skim::init(options, None).unwrap();

            skim.start();

            if skim.should_enter() {
                skim.init_tui().unwrap();
            }
        });
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(100);
    targets = criterion_benchmark
);
criterion_main!(benches);
