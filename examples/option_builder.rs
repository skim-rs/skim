//! Demonstrates configuring skim with `SkimOptionsBuilder`.

extern crate skim;
use skim::prelude::*;
use std::io::Cursor;

fn main() {
    let item_reader = SkimItemReader::default();

    //==================================================
    // first run
    let options = SkimOptionsBuilder::default().height("50%").multi(true).build().unwrap();
    let input = "aaaaa\nbbbb\nccc";
    let items = item_reader.of_bufread(Cursor::new(input));
    let selected_items = Skim::run_with(options, Some(items))
        .map(|out| out.selected_items)
        .unwrap_or_default();

    for item in &selected_items {
        println!("{}", item.output());
    }

    //==================================================
    // second run
    let options = SkimOptionsBuilder::default().height("50%").multi(true).build().unwrap();
    let input = "11111\n22222\n333333333";
    let items = item_reader.of_bufread(Cursor::new(input));
    let selected_items = Skim::run_with(options, Some(items))
        .map(|out| out.selected_items)
        .unwrap_or_default();

    for item in &selected_items {
        println!("{}", item.output());
    }
}
