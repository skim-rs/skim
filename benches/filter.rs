use std::fs;

use criterion::{Criterion, criterion_group, criterion_main};

use skim::Typos;
use skim::helper::item::DefaultSkimItem;
use skim::prelude::*;

const CHUNK_SIZE: usize = 1024;
fn load_lines(file: &str) -> Vec<String> {
    let data = fs::read_to_string(format!("benches/fixtures/{file}")).expect("{file} missing");
    data.lines().map(|l| l.to_string()).collect()
}

fn prepare(file: &str, opt_builder: &mut SkimOptionsBuilder) -> (SkimOptions, SkimItemReceiver) {
    let lines = load_lines(file);
    let opts = opt_builder.build().unwrap();
    let (tx, rx) = unbounded();
    let mut chunk_size = 0;
    let mut chunk = Vec::new();
    for line in lines {
        if chunk_size >= CHUNK_SIZE {
            tx.send(chunk).unwrap();
            chunk_size = 0;
            chunk = Vec::new();
        }
        chunk.push(Arc::new(DefaultSkimItem::from(line)) as Arc<dyn SkimItem>);
    }
    tx.send(chunk).unwrap();
    (opts, rx)
}

fn criterion_benchmark_10m(c: &mut Criterion) {
    c.bench_function("filter_10M_regex", |b| {
        b.iter_batched(
            || prepare("10M.txt", SkimOptionsBuilder::default().filter("test").regex(true)),
            |(opts, rx)| Skim::run_with(opts, Some(rx)),
            criterion::BatchSize::SmallInput,
        );
    });
    c.bench_function("filter_10M_frizbee", |b| {
        b.iter_batched(
            || {
                prepare(
                    "10M.txt",
                    SkimOptionsBuilder::default()
                        .filter("test")
                        .algorithm(FuzzyAlgorithm::Frizbee)
                        .typos(Typos::Disabled),
                )
            },
            |(opts, rx)| Skim::run_with(opts, Some(rx)),
            criterion::BatchSize::SmallInput,
        );
    });
    c.bench_function("filter_10M_frizbee_typos", |b| {
        b.iter_batched(
            || {
                prepare(
                    "10M.txt",
                    SkimOptionsBuilder::default()
                        .filter("test")
                        .algorithm(FuzzyAlgorithm::Frizbee)
                        .typos(Typos::Smart),
                )
            },
            |(opts, rx)| Skim::run_with(opts, Some(rx)),
            criterion::BatchSize::SmallInput,
        );
    });
    c.bench_function("filter_10M_clangd", |b| {
        b.iter_batched(
            || {
                prepare(
                    "10M.txt",
                    SkimOptionsBuilder::default()
                        .filter("test")
                        .algorithm(FuzzyAlgorithm::Clangd),
                )
            },
            |(opts, rx)| Skim::run_with(opts, Some(rx)),
            criterion::BatchSize::SmallInput,
        );
    });
    c.bench_function("filter_10M_fzy", |b| {
        b.iter_batched(
            || {
                prepare(
                    "10M.txt",
                    SkimOptionsBuilder::default()
                        .filter("test")
                        .algorithm(FuzzyAlgorithm::Fzy)
                        .typos(Typos::Disabled),
                )
            },
            |(opts, rx)| Skim::run_with(opts, Some(rx)),
            criterion::BatchSize::SmallInput,
        );
    });
    c.bench_function("filter_10M_fzy_typos", |b| {
        b.iter_batched(
            || {
                prepare(
                    "10M.txt",
                    SkimOptionsBuilder::default()
                        .filter("test")
                        .algorithm(FuzzyAlgorithm::Fzy)
                        .typos(Typos::Smart),
                )
            },
            |(opts, rx)| Skim::run_with(opts, Some(rx)),
            criterion::BatchSize::SmallInput,
        );
    });
    c.bench_function("filter_10M_arinae", |b| {
        b.iter_batched(
            || {
                prepare(
                    "10M.txt",
                    SkimOptionsBuilder::default()
                        .filter("test")
                        .algorithm(FuzzyAlgorithm::Arinae)
                        .typos(Typos::Disabled),
                )
            },
            |(opts, rx)| Skim::run_with(opts, Some(rx)),
            criterion::BatchSize::SmallInput,
        );
    });
    c.bench_function("filter_10M_arinae_typos", |b| {
        b.iter_batched(
            || {
                prepare(
                    "10M.txt",
                    SkimOptionsBuilder::default()
                        .filter("test")
                        .algorithm(FuzzyAlgorithm::Arinae)
                        .typos(Typos::Smart),
                )
            },
            |(opts, rx)| Skim::run_with(opts, Some(rx)),
            criterion::BatchSize::SmallInput,
        );
    });
}

fn criterion_benchmark_1m(c: &mut Criterion) {
    c.bench_function("filter_1M_regex", |b| {
        b.iter_batched(
            || prepare("1M.txt", SkimOptionsBuilder::default().filter("test").regex(true)),
            |(opts, rx)| Skim::run_with(opts, Some(rx)),
            criterion::BatchSize::SmallInput,
        );
    });
    c.bench_function("filter_1M_frizbee", |b| {
        b.iter_batched(
            || {
                prepare(
                    "1M.txt",
                    SkimOptionsBuilder::default()
                        .filter("test")
                        .algorithm(FuzzyAlgorithm::Frizbee)
                        .typos(Typos::Disabled),
                )
            },
            |(opts, rx)| Skim::run_with(opts, Some(rx)),
            criterion::BatchSize::SmallInput,
        );
    });
    c.bench_function("filter_1M_frizbee_typos", |b| {
        b.iter_batched(
            || {
                prepare(
                    "1M.txt",
                    SkimOptionsBuilder::default()
                        .filter("test")
                        .algorithm(FuzzyAlgorithm::Frizbee)
                        .typos(Typos::Smart),
                )
            },
            |(opts, rx)| Skim::run_with(opts, Some(rx)),
            criterion::BatchSize::SmallInput,
        );
    });
    c.bench_function("filter_1M_clangd", |b| {
        b.iter_batched(
            || {
                prepare(
                    "1M.txt",
                    SkimOptionsBuilder::default()
                        .filter("test")
                        .algorithm(FuzzyAlgorithm::Clangd),
                )
            },
            |(opts, rx)| Skim::run_with(opts, Some(rx)),
            criterion::BatchSize::SmallInput,
        );
    });
    c.bench_function("filter_1M_fzy", |b| {
        b.iter_batched(
            || {
                prepare(
                    "1M.txt",
                    SkimOptionsBuilder::default()
                        .filter("test")
                        .algorithm(FuzzyAlgorithm::Fzy)
                        .typos(Typos::Disabled),
                )
            },
            |(opts, rx)| Skim::run_with(opts, Some(rx)),
            criterion::BatchSize::SmallInput,
        );
    });
    c.bench_function("filter_1M_fzy_typos", |b| {
        b.iter_batched(
            || {
                prepare(
                    "1M.txt",
                    SkimOptionsBuilder::default()
                        .filter("test")
                        .algorithm(FuzzyAlgorithm::Fzy)
                        .typos(Typos::Smart),
                )
            },
            |(opts, rx)| Skim::run_with(opts, Some(rx)),
            criterion::BatchSize::SmallInput,
        );
    });
    c.bench_function("filter_1M_arinae", |b| {
        b.iter_batched(
            || {
                prepare(
                    "1M.txt",
                    SkimOptionsBuilder::default()
                        .filter("test")
                        .algorithm(FuzzyAlgorithm::Arinae)
                        .typos(Typos::Disabled),
                )
            },
            |(opts, rx)| Skim::run_with(opts, Some(rx)),
            criterion::BatchSize::SmallInput,
        );
    });
    c.bench_function("filter_1M_arinae_typos", |b| {
        b.iter_batched(
            || {
                prepare(
                    "1M.txt",
                    SkimOptionsBuilder::default()
                        .filter("test")
                        .algorithm(FuzzyAlgorithm::Arinae)
                        .typos(Typos::Smart),
                )
            },
            |(opts, rx)| Skim::run_with(opts, Some(rx)),
            criterion::BatchSize::SmallInput,
        );
    });

    c.bench_function("filter_1M_andor", |b| {
        b.iter_batched(
            || prepare("1M.txt", SkimOptionsBuilder::default().filter("boot foo | mnt foo")),
            |(opts, rx)| Skim::run_with(opts, Some(rx)),
            criterion::BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    name = benches_10m;
    config = Criterion::default().sample_size(10);
    targets = criterion_benchmark_10m
);
criterion_group!(
    name = benches_1m;
    config = Criterion::default().sample_size(100);
    targets = criterion_benchmark_1m
);
criterion_main!(benches_1m, benches_10m);
