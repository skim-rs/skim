use criterion::{Criterion, criterion_group, criterion_main};

use skim::prelude::*;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("default", |b| {
        b.iter(|| {
            let opts = SkimOptionsBuilder::default()
                .cmd("cat bench_data.txt")
                .filter("test")
                .build()?;
            Skim::run_with(opts, None)
        });
    });
    c.bench_function("regex", |b| {
        b.iter(|| {
            let opts = SkimOptionsBuilder::default()
                .cmd("cat bench_data.txt")
                .filter("test")
                .regex(true)
                .build()?;
            Skim::run_with(opts, None)
        });
    });
    c.bench_function("frizbee", |b| {
        b.iter(|| {
            let opts = SkimOptionsBuilder::default()
                .cmd("cat bench_data.txt")
                .filter("test")
                .algorithm(FuzzyAlgorithm::Frizbee)
                .build()?;
            Skim::run_with(opts, None)
        });
    });
    c.bench_function("clangd", |b| {
        b.iter(|| {
            let opts = SkimOptionsBuilder::default()
                .cmd("cat bench_data.txt")
                .filter("test")
                .algorithm(FuzzyAlgorithm::Clangd)
                .build()?;
            Skim::run_with(opts, None)
        });
    });
    c.bench_function("ansi", |b| {
        b.iter(|| {
            let opts = SkimOptionsBuilder::default()
                .cmd("cat bench_data.txt")
                .filter("test")
                .ansi(true)
                .build()?;
            Skim::run_with(opts, None)
        });
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = criterion_benchmark
);
criterion_main!(benches);
