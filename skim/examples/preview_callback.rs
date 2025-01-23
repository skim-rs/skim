use std::io::Cursor;

use skim::prelude::*;

pub fn main() {
    let _ = env_logger::init();
    let options = SkimOptionsBuilder::default()
        .multi(true)
        .preview_fn(Some(PreviewCallback::from(|items: Vec<Arc<dyn SkimItem>>| {
            items
                .iter()
                .map(|s| s.text().to_ascii_uppercase().into())
                .collect::<Vec<_>>()
        })))
        .build()
        .unwrap();
    let item_reader = SkimItemReader::default();

    let input = "aaaaa\nbbbb\nccc";
    let items = item_reader.of_bufread(Cursor::new(input));
    let selected_items = Skim::run_with(&options, Some(items))
        .map(|out| out.selected_items)
        .unwrap_or_else(|| Vec::new());

    for item in selected_items.iter() {
        println!("{}", item.output());
    }
}
