use color_eyre::eyre::{Ok, Result};
use criterion::{Criterion, criterion_group, criterion_main};

use skim::prelude::*;

async fn wait_until_done(mut opts: SkimOptions) -> Result<SkimOutput> {
    opts.cmd = Some(String::from("cat bench_data.txt"));
    let mut skim = Skim::init(opts, None)?;
    skim.start();
    skim.init_tui()?;
    skim.enter().await?;
    while !skim.tick().await? {
        if skim.reader_done() && skim.matcher_stopped() {
            skim.event_sender().send(Event::Action(Action::Accept(None))).await?;
        }
    }
    Ok(skim.output())
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("default", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.to_async(rt)
            .iter(async || wait_until_done(SkimOptions::default()).await);
    });
    c.bench_function("query", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.to_async(rt)
            .iter(async || wait_until_done(SkimOptionsBuilder::default().query("test").build().unwrap()).await);
    });
    c.bench_function("typing", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.to_async(rt).iter(async || {
            let mut skim = Skim::init(SkimOptionsBuilder::default().cmd("cat bench_data.txt").build()?, None)?;
            skim.start();
            skim.init_tui()?;
            skim.enter().await?;
            let s = skim.event_sender();
            let mut sent = false;
            let mut done_since = 0;
            let mut done = false;
            while !skim.tick().await? {
                if skim.reader_done() && skim.matcher_stopped() {
                    if done {
                        done_since += 1;
                    } else {
                        done_since = 1;
                    }
                    if sent && done_since > 5 {
                        s.send(Event::Action(Action::Accept(None))).await?;
                    } else if !sent {
                        s.send(Event::Action(Action::AddChar('t'))).await?;
                        s.send(Event::Action(Action::AddChar('e'))).await?;
                        s.send(Event::Action(Action::AddChar('s'))).await?;
                        s.send(Event::Action(Action::AddChar('t'))).await?;
                        sent = true;
                    }
                    done = true;
                } else {
                    done = false;
                }
            }
            Ok(skim.output())
        });
    });
    c.bench_function("frizbee", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.to_async(rt).iter(async || {
            wait_until_done(
                SkimOptionsBuilder::default()
                    .query("test")
                    .algorithm(FuzzyAlgorithm::Frizbee)
                    .build()
                    .unwrap(),
            )
            .await
        });
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = criterion_benchmark
);
criterion_main!(benches);
