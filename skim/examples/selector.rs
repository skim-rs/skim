extern crate skim;
use skim::prelude::*;

struct BasicSelector {
    pub pat: String,
}

impl Selector for BasicSelector {
    fn should_select(&self, _index: usize, item: &dyn SkimItem) -> bool {
        return item.text().contains(&self.pat);
    }
}

pub fn main() {
    let selector = BasicSelector {
        pat: String::from("examples"),
    };
    let options = SkimOptionsBuilder::default()
        .multi(true)
        .selector(Some(Rc::from(selector)))
        .query(Some(String::from("skim/")))
        .build()
        .unwrap();

    let selected_items = Skim::run_with(options, None)
        .map(|out| out.selected_items)
        .unwrap_or_default();

    for item in selected_items.iter() {
        println!("{}", item.output());
    }
}
