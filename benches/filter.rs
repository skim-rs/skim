use criterion::{Criterion, criterion_group, criterion_main};

use skim::Typos;
use skim::prelude::*;

fn criterion_benchmark_10m(c: &mut Criterion) {
    c.bench_function("filter_10M_default", |b| {
        b.iter(|| {
            let opts = SkimOptionsBuilder::default()
                .cmd("cat benches/fixtures/10M.txt")
                .filter("test")
                .build()?;
            Skim::run_with(opts, None)
        });
    });
    c.bench_function("filter_10M_regex", |b| {
        b.iter(|| {
            let opts = SkimOptionsBuilder::default()
                .cmd("cat benches/fixtures/10M.txt")
                .filter("test")
                .regex(true)
                .build()?;
            Skim::run_with(opts, None)
        });
    });
    c.bench_function("filter_10M_frizbee", |b| {
        b.iter(|| {
            let opts = SkimOptionsBuilder::default()
                .cmd("cat benches/fixtures/10M.txt")
                .filter("test")
                .algorithm(FuzzyAlgorithm::Frizbee)
                .build()?;
            Skim::run_with(opts, None)
        });
    });
    c.bench_function("filter_10M_frizbee_typos", |b| {
        b.iter(|| {
            let opts = SkimOptionsBuilder::default()
                .cmd("cat benches/fixtures/10M.txt")
                .filter("test")
                .typos(Typos::Smart)
                .algorithm(FuzzyAlgorithm::Frizbee)
                .build()?;
            Skim::run_with(opts, None)
        });
    });
    c.bench_function("filter_10M_clangd", |b| {
        b.iter(|| {
            let opts = SkimOptionsBuilder::default()
                .cmd("cat benches/fixtures/10M.txt")
                .filter("test")
                .algorithm(FuzzyAlgorithm::Clangd)
                .build()?;
            Skim::run_with(opts, None)
        });
    });
    c.bench_function("filter_10M_fzy", |b| {
        b.iter(|| {
            let opts = SkimOptionsBuilder::default()
                .cmd("cat benches/fixtures/10M.txt")
                .filter("test")
                .algorithm(FuzzyAlgorithm::Fzy)
                .build()?;
            Skim::run_with(opts, None)
        });
    });
    c.bench_function("filter_10M_fzy_typos", |b| {
        b.iter(|| {
            let opts = SkimOptionsBuilder::default()
                .cmd("cat benches/fixtures/10M.txt")
                .filter("test")
                .typos(Typos::Smart)
                .algorithm(FuzzyAlgorithm::Fzy)
                .build()?;
            Skim::run_with(opts, None)
        });
    });
}

fn criterion_benchmark_1m(c: &mut Criterion) {
    c.bench_function("filter_1M_default", |b| {
        b.iter(|| {
            let opts = SkimOptionsBuilder::default()
                .cmd("cat benches/fixtures/1M.txt")
                .filter("test")
                .build()?;
            Skim::run_with(opts, None)
        });
    });
    c.bench_function("filter_1M_regex", |b| {
        b.iter(|| {
            let opts = SkimOptionsBuilder::default()
                .cmd("cat benches/fixtures/1M.txt")
                .filter("test")
                .regex(true)
                .build()?;
            Skim::run_with(opts, None)
        });
    });
    c.bench_function("filter_1M_frizbee", |b| {
        b.iter(|| {
            let opts = SkimOptionsBuilder::default()
                .cmd("cat benches/fixtures/1M.txt")
                .filter("test")
                .algorithm(FuzzyAlgorithm::Frizbee)
                .build()?;
            Skim::run_with(opts, None)
        });
    });
    c.bench_function("filter_1M_frizbee_typos", |b| {
        b.iter(|| {
            let opts = SkimOptionsBuilder::default()
                .cmd("cat benches/fixtures/1M.txt")
                .filter("test")
                .typos(Typos::Smart)
                .algorithm(FuzzyAlgorithm::Frizbee)
                .build()?;
            Skim::run_with(opts, None)
        });
    });
    c.bench_function("filter_1M_clangd", |b| {
        b.iter(|| {
            let opts = SkimOptionsBuilder::default()
                .cmd("cat benches/fixtures/1M.txt")
                .filter("test")
                .algorithm(FuzzyAlgorithm::Clangd)
                .build()?;
            Skim::run_with(opts, None)
        });
    });
    c.bench_function("filter_1M_fzy", |b| {
        b.iter(|| {
            let opts = SkimOptionsBuilder::default()
                .cmd("cat benches/fixtures/1M.txt")
                .filter("test")
                .algorithm(FuzzyAlgorithm::Fzy)
                .build()?;
            Skim::run_with(opts, None)
        });
    });
    c.bench_function("filter_1M_fzy_typos", |b| {
        b.iter(|| {
            let opts = SkimOptionsBuilder::default()
                .cmd("cat benches/fixtures/1M.txt")
                .filter("test")
                .typos(Typos::Smart)
                .algorithm(FuzzyAlgorithm::Fzy)
                .build()?;
            Skim::run_with(opts, None)
        });
    });
}

criterion_group!(
    name = benches_10m;
    config = Criterion::default().sample_size(10).measurement_time(std::time::Duration::from_secs(100));
    targets = criterion_benchmark_10m
);
criterion_group!(
    name = benches_1m;
    config = Criterion::default().sample_size(100).measurement_time(std::time::Duration::from_secs(100));
    targets = criterion_benchmark_1m
);
criterion_main!(benches_1m, benches_10m);
