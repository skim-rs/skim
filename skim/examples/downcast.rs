extern crate skim;
use skim::prelude::*;

/// This example illustrates downcasting custom structs that implement
/// `SkimItem` after calling `Skim::run_with`.

#[derive(Debug, Clone)]
struct Item {
    text: String,
    index: usize,
}

impl SkimItem for Item {
    fn text(&self) -> Cow<str> {
        Cow::Borrowed(&self.text)
    }

    fn preview(&self, _context: PreviewContext) -> ItemPreview {
        ItemPreview::Text(self.text.to_owned())
    }

    fn get_index(&self) -> usize {
        self.index
    }

    fn set_index(&mut self, index: usize) {
        self.index = index
    }
}

pub fn main() {
    let options = SkimOptionsBuilder::default()
        .height(String::from("50%"))
        .multi(true)
        .preview(Some(String::new()))
        .build()
        .unwrap();

    let (tx, rx): (SkimItemSender, SkimItemReceiver) = unbounded();

    tx.send(Arc::new(Item {
        text: "a".to_string(),
        index: 0,
    }))
    .unwrap();
    tx.send(Arc::new(Item {
        text: "b".to_string(),
        index: 1,
    }))
    .unwrap();
    tx.send(Arc::new(Item {
        text: "c".to_string(),
        index: 2,
    }))
    .unwrap();

    drop(tx);

    let selected_items = Skim::run_with(&options, Some(rx))
        .map(|out| out.selected_items)
        .unwrap_or_default()
        .iter()
        .map(|selected_item| (**selected_item).as_any().downcast_ref::<Item>().unwrap().to_owned())
        .collect::<Vec<Item>>();

    for item in selected_items {
        println!("{item:?}");
    }
}
