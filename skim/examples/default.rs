extern crate skim;
use skim::prelude::*;
use std::io::Cursor;

pub fn main() {
    let options = SkimOptionsBuilder::default()
        .height("50%".to_string())
        .multi(true)
        .build()
        .unwrap();

    let input = "aaaaa\nbbbb\nccc".to_string();

    // `SkimItemReader` is a helper to turn any `BufRead` into a stream of `SkimItem`
    // `SkimItem` was implemented for `AsRef<str>` by default
    let item_reader = SkimItemReader::default();
    let items = item_reader.of_bufread(Cursor::new(input));

    // `run_with` would read and show items from the stream
    let selected_items = Skim::run_with(&options, Some(items))
        .map(|out| out.selected_items)
        .unwrap_or_else(|| Vec::new());

    println!("\n\n\n\n\n\n");
    for item in selected_items.iter() {
        println!("{:?}", item.output().chars().collect::<Vec<char>>());
    }
}
