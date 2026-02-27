//! Microbenchmark that isolates the fuzzy matcher DP from all other overhead
//! (I/O, threading, sorting).

use std::fs;

use criterion::{Criterion, criterion_group, criterion_main};

use skim::CaseMatching;
use skim::fuzzy_matcher::FuzzyMatcher;
use skim::fuzzy_matcher::arinae::ArinaeMatcher;
use skim::fuzzy_matcher::frizbee::FrizbeeMatcher;
use skim::prelude::SkimMatcherV2;

fn load_lines() -> Vec<String> {
    let data = fs::read_to_string("benches/fixtures/1M.txt").expect("1M.txt missing");
    data.lines().map(|l| l.to_string()).collect()
}

fn bench_matcher(c: &mut Criterion) {
    let lines = load_lines();

    c.bench_function("micro_skim_v2", |b| {
        let m = SkimMatcherV2::default().smart_case();
        b.iter(|| {
            let mut count = 0u64;
            for line in &lines {
                if m.fuzzy_indices(line, "test").is_some() {
                    count += 1;
                }
            }
            count
        });
    });
    c.bench_function("micro_frizbee", |b| {
        let m = FrizbeeMatcher::default().case(CaseMatching::Smart).max_typos(Some(0));
        b.iter(|| {
            let mut count = 0u64;
            for line in &lines {
                if m.fuzzy_indices(line, "test").is_some() {
                    count += 1;
                }
            }
            count
        });
    });
    c.bench_function("micro_typos_frizbee", |b| {
        let m = FrizbeeMatcher::default().case(CaseMatching::Smart).max_typos(Some(1));
        b.iter(|| {
            let mut count = 0u64;
            for line in &lines {
                if m.fuzzy_indices(line, "test").is_some() {
                    count += 1;
                }
            }
            count
        });
    });
    c.bench_function("micro_arinae", |b| {
        let m = ArinaeMatcher::new(CaseMatching::Smart, false);
        b.iter(|| {
            let mut count = 0u64;
            for line in &lines {
                if m.fuzzy_indices(line, "test").is_some() {
                    count += 1;
                }
            }
            count
        });
    });
    c.bench_function("micro_arinae_range", |b| {
        let m = ArinaeMatcher::new(CaseMatching::Smart, false);
        b.iter(|| {
            let mut count = 0u64;
            for line in &lines {
                if m.fuzzy_match_range(line, "test").is_some() {
                    count += 1;
                }
            }
            count
        });
    });
    c.bench_function("micro_arinae_score", |b| {
        let m = ArinaeMatcher::new(CaseMatching::Smart, false);
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
    c.bench_function("micro_typos_arinae", |b| {
        let m = ArinaeMatcher::new(CaseMatching::Smart, true);
        b.iter(|| {
            let mut count = 0u64;
            for line in &lines {
                if m.fuzzy_indices(line, "test").is_some() {
                    count += 1;
                }
            }
            count
        });
    });
    c.bench_function("micro_typos_arinae_range", |b| {
        let m = ArinaeMatcher::new(CaseMatching::Smart, true);
        b.iter(|| {
            let mut count = 0u64;
            for line in &lines {
                if m.fuzzy_match_range(line, "test").is_some() {
                    count += 1;
                }
            }
            count
        });
    });
    c.bench_function("micro_typos_arinae_score", |b| {
        let m = ArinaeMatcher::new(CaseMatching::Smart, true);
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
}

criterion_group!(benches, bench_matcher);
criterion_main!(benches);
