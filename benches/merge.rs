#![allow(missing_docs)]

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use rand::rngs::StdRng;
use rand::{RngExt as _, SeedableRng as _};
use skim::Rank;
use skim::item::{MatchEntry, RankBuilder};

fn merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("merge");
    group.sample_size(10);
    let mut rng = StdRng::seed_from_u64(0);
    let builder = RankBuilder::default();
    let mut run = |indices: std::ops::Range<i32>| {
        let mut items: Vec<_> = indices
            .map(|index| {
                let rank = Rank {
                    index,
                    score: rng.random(),
                    ..Rank::default()
                };
                MatchEntry::new(&rank, &builder)
            })
            .collect();
        items.sort_unstable();
        items
    };
    for (id, n, k) in [("1M+4K", 1_000_000, 4_000), ("3M+64K", 3_000_000, 64_000)] {
        let (existing, incoming) = (run(0..n), run(n..n + k));
        group.bench_function(id, |b| {
            b.iter_batched(
                || (existing.as_slice(), incoming.as_slice()),
                |(a, b)| MatchEntry::merge_sorted(a, b),
                BatchSize::LargeInput,
            );
        });
    }
}

criterion_group!(benches, merge);
criterion_main!(benches);
