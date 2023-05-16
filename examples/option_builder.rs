extern crate skim;
use skim::prelude::*;
use std::io::Cursor;

pub fn main() {
    let options = SkimOptionsBuilder::default()
        .height(Some("50%"))
        .multi(true)
        .build()
        .unwrap();
    let item_reader = SkimItemReader::default();

    //==================================================
    // first run
    let input = "aaaaa\nbbbb\nccc";
    let (items, opt_ingest_handle) = item_reader.of_bufread(Box::new(Cursor::new(input)));
    let selected_items = Skim::run_with(&options, Some(items))
        .map(|out| out.selected_items)
        .unwrap_or_else(Vec::new);

    for item in selected_items.iter() {
        println!("{}", item.output());
    }

    opt_ingest_handle.map(|handle| handle.join());

    //==================================================
    // second run
    let input = "11111\n22222\n333333333";
    let (items, opt_ingest_handle) = item_reader.of_bufread(Box::new(Cursor::new(input)));
    let selected_items = Skim::run_with(&options, Some(items))
        .map(|out| out.selected_items)
        .unwrap_or_else(Vec::new);

    for item in selected_items.iter() {
        println!("{}", item.output());
    }

    opt_ingest_handle.map(|handle| handle.join());
}
