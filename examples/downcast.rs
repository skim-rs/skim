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
    fn text(&self) -> Cow<'_, str> {
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
        .height("50%")
        .multi(true)
        .preview("")
        .build()
        .unwrap();

    let (tx, rx): (SkimItemSender, SkimItemReceiver) = unbounded();

    tx.send(vec![
        Arc::new(Item {
            text: "a".into(),
            index: 0,
        }) as Arc<dyn SkimItem>,
        Arc::new(Item {
            text: "b".into(),
            index: 1,
        }) as Arc<dyn SkimItem>,
        Arc::new(Item {
            text: "c".into(),
            index: 2,
        }) as Arc<dyn SkimItem>,
    ])
    .unwrap();

    drop(tx);

    let selected_items = Skim::run_with(options, Some(rx))
        .map(|out| out.selected_items)
        .unwrap_or_default()
        .iter()
        .map(|selected_item| (**selected_item).as_any().downcast_ref::<Item>().unwrap().to_owned())
        .collect::<Vec<Item>>();

    for item in selected_items {
        println!("{item:?}");
    }
}
