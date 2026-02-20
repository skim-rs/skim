use std::io::{BufWriter, Stderr};

use clap::Parser as _;
use criterion::{Criterion, criterion_group, criterion_main};

use ratatui::prelude::CrosstermBackend;
use skim::prelude::*;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("parse_options", |b| {
        b.iter(|| SkimOptions::parse_from(Vec::<&str>::new()));
    });
    c.bench_function("init", |b| {
        b.iter_batched(
            || SkimOptions::default().build(),
            |options: SkimOptions| Skim::<CrosstermBackend<BufWriter<Stderr>>>::init(options, None),
            criterion::BatchSize::SmallInput,
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
            criterion::BatchSize::SmallInput,
        );
    });
    c.bench_function("start", |b| {
        b.iter_batched(
            || Skim::<CrosstermBackend<BufWriter<Stderr>>>::init(SkimOptions::default().build(), None).unwrap(),
            |mut skim: Skim| skim.start(),
            criterion::BatchSize::SmallInput,
        );
    });

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
