use std::io::Cursor;

use skim::prelude::*;

pub fn main() {
    env_logger::init();
    let options = SkimOptionsBuilder::default()
        .multi(true)
        .preview_fn(PreviewCallback::from(|items: Vec<Arc<dyn SkimItem>>| {
            items.iter().map(|s| s.text().to_ascii_uppercase()).collect::<Vec<_>>()
        }))
        .build()
        .unwrap();
    let item_reader = SkimItemReader::default();

    let input = "aaaaa\nbbbb\nccc";
    let items = item_reader.of_bufread(Cursor::new(input));
    let selected_items = Skim::run_with(options, Some(items))
        .map(|out| out.selected_items)
        .unwrap_or_default();

    for item in selected_items.iter() {
        println!("{}", item.output());
    }
}
