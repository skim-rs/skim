use gungraun::{library_benchmark, library_benchmark_group, main};
use std::fs;
use std::hint::black_box;

use skim::CaseMatching;
use skim::fuzzy_matcher::FuzzyMatcher;
use skim::fuzzy_matcher::arinae::ArinaeMatcher;
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use skim::fuzzy_matcher::frizbee::FrizbeeMatcher;
use skim::prelude::SkimMatcherV2;

fn load_lines() -> Vec<String> {
    let data = fs::read_to_string("benches/fixtures/1M.txt").expect("1M.txt missing");
    data.lines().map(|l| l.to_string()).collect()
}

#[inline(always)]
fn bench_matcher(m: impl FuzzyMatcher, lines: Vec<String>) -> u64 {
    let mut count = 0u64;
    for line in &lines {
        if m.fuzzy_indices(line, "test").is_some() {
            count += 1;
        }
    }
    count
}

#[library_benchmark]
fn skim_v2() -> u64 {
    bench_matcher(SkimMatcherV2::default().smart_case(), black_box(load_lines()))
}
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
#[library_benchmark]
fn frizbee() -> u64 {
    bench_matcher(
        FrizbeeMatcher::default().case(CaseMatching::Smart).max_typos(Some(0)),
        black_box(load_lines()),
    )
}
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
#[library_benchmark]
fn frizbee_typos() -> u64 {
    bench_matcher(
        FrizbeeMatcher::default().case(CaseMatching::Smart).max_typos(Some(1)),
        black_box(load_lines()),
    )
}
#[library_benchmark]
fn arinae() -> u64 {
    bench_matcher(
        ArinaeMatcher::new(CaseMatching::Smart, false, false),
        black_box(load_lines()),
    )
}
#[library_benchmark]
fn arinae_typos() -> u64 {
    bench_matcher(
        ArinaeMatcher::new(CaseMatching::Smart, true, false),
        black_box(load_lines()),
    )
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
library_benchmark_group!(
    name = benches,
    benchmarks = [skim_v2, frizbee, frizbee_typos, arinae, arinae_typos]
);

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
library_benchmark_group!(
    name = benches,
    benchmarks = [skim_v2, arinae, arinae_typos]
);

main!(library_benchmark_groups = benches);
