use std::sync::Arc;

use skim::prelude::*;

fn main() {
    let (sender, receiver): (SkimItemSender, SkimItemReceiver) = unbounded();
    let mut batch = Vec::new();
    for num in 1..=8 {
        batch.push(Arc::new(format!("Option {num}")) as Arc<dyn SkimItem>);
    }
    sender.send(batch).unwrap();
    drop(sender); // bug replicates even without this

    let _ = Skim::run_with(
        SkimOptions {
            multi: true,
            ..Default::default()
        },
        Some(receiver),
    );
}
