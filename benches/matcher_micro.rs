//! Microbenchmark that isolates the fuzzy matcher DP from all other overhead
//! (I/O, threading, sorting).

use std::fs;

use criterion::{Criterion, criterion_group, criterion_main};

use skim::CaseMatching;
use skim::fuzzy_matcher::FuzzyMatcher;
use skim::fuzzy_matcher::skim_v3::SkimV3Matcher;

fn load_lines() -> Vec<String> {
    let data = fs::read_to_string("benches/fixtures/1M.txt").expect("1M.txt missing");
    data.lines().map(|l| l.to_string()).collect()
}

fn bench_matcher(c: &mut Criterion) {
    let lines = load_lines();

    c.bench_function("micro_skim_v3", |b| {
        let m = SkimV3Matcher::new(CaseMatching::Smart, false);
        b.iter(|| {
            let mut count = 0u64;
            for line in &lines {
                if m.fuzzy_match(line, "test").is_some() {
                    count += 1;
                }
            }
            count
        });
    });

    c.bench_function("micro_skim_v3_typos", |b| {
        let m = SkimV3Matcher::new(CaseMatching::Smart, true);
        b.iter(|| {
            let mut count = 0u64;
            for line in &lines {
                if m.fuzzy_match(line, "test").is_some() {
                    count += 1;
                }
            }
            count
        });
    });

    // SIMD batch benchmarks — process items in batches of 8
    let bytes_lines: Vec<&[u8]> = lines.iter().map(|s| s.as_bytes()).collect();

    c.bench_function("micro_skim_v3_batch", |b| {
        let m = SkimV3Matcher::new(CaseMatching::Smart, false);
        let pattern = b"test";
        b.iter(|| {
            let results = m.batch_fuzzy_match_bytes(&bytes_lines, pattern, false);
            results.iter().filter(|r| r.is_some()).count() as u64
        });
    });

    c.bench_function("micro_skim_v3_batch_typos", |b| {
        let m = SkimV3Matcher::new(CaseMatching::Smart, true);
        let pattern = b"test";
        b.iter(|| {
            let results = m.batch_fuzzy_match_bytes(&bytes_lines, pattern, false);
            results.iter().filter(|r| r.is_some()).count() as u64
        });
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(20);
    targets = bench_matcher
);
criterion_main!(benches);
