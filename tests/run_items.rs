//! Public API tests for `Skim::run_items` without a terminal.
#![allow(missing_docs, clippy::pedantic)]

use std::time::Duration;

use skim::prelude::*;

#[test]
fn run_items_filters_supplied_items() {
    let mut options = SkimOptions::default();
    options.filter = Some("apple".into());
    let output = Skim::run_items(options.build(), ["apple", "banana", "pineapple"]).unwrap();
    let matches: Vec<_> = output
        .selected_items
        .iter()
        .map(|item| item.output().into_owned())
        .collect();
    assert!(!output.is_abort);
    assert_eq!(matches, ["apple", "pineapple"]);
}

#[test]
fn run_items_finishes_at_batch_boundaries() {
    // A missing sender close would leave filter mode waiting for the reader.
    let (tx, rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        for count in [0, 1, 1024, 1025, 2048] {
            let mut options = SkimOptions::default();
            options.filter = Some(String::new());
            options.no_sort = true;
            let items: Vec<_> = (0..count).map(|i| format!("item{i:04}")).collect();
            let output = Skim::run_items(options.build(), items.clone()).unwrap();
            let matches: Vec<_> = output
                .selected_items
                .iter()
                .map(|item| item.output().into_owned())
                .collect();
            assert_eq!(matches, items, "item count: {count}");
        }
        tx.send(()).unwrap();
    });
    rx.recv_timeout(Duration::from_secs(20))
        .expect("run_items did not finish");
    worker.join().unwrap();
}
