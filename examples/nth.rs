extern crate skim;
use skim::prelude::*;
use std::io::Cursor;

#[cfg(feature = "malloc_trim")]
#[cfg(target_os = "linux")]
#[cfg(target_env = "gnu")]
use crate::malloc_trim;

/// `nth` option is supported by SkimItemReader.
/// In the example below, with `nth=2` set, only `123` could be matched.

pub fn main() {
    let input = "foo 123";

    let options = SkimOptionsBuilder::default().query(Some("f")).build().unwrap();
    let item_reader = SkimItemReader::new(SkimItemReaderOption::default().nth("2").build());

    let (items, opt_ingest_handle) = item_reader.of_bufread(Box::new(Cursor::new(input)));
    let selected_items = Skim::run_with(&options, Some(items))
        .map(|out| out.selected_items)
        .unwrap_or_else(Vec::new);

    for item in selected_items.iter() {
        println!("{}", item.output());
    }

    if let Some(handle) = opt_ingest_handle {
        let _ = handle.join();
        #[cfg(feature = "malloc_trim")]
        #[cfg(target_os = "linux")]
        #[cfg(target_env = "gnu")]
        malloc_trim();
    }
}
