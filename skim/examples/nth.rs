extern crate skim;
use skim::prelude::*;
use std::io::Cursor;

/// `nth` option is supported by SkimItemReader.
/// In the example below, with `nth=2` set, only `123` could be matched.
pub fn main() {
    let input = "foo 123";

    let options = SkimOptionsBuilder::default()
        .query(Some(String::from("f")))
        .build()
        .unwrap();
    let item_reader = SkimItemReader::new(SkimItemReaderOption::default().nth(vec!["2"].into_iter()).build());

    let items = item_reader.of_bufread(Cursor::new(input));
    let selected_items = Skim::run_with(&options, Some(items))
        .map(|out| out.selected_items)
        .unwrap_or_default();

    for item in selected_items.iter() {
        println!("{}", item.output());
    }
}
