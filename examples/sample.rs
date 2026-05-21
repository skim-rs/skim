//! Minimal example that runs skim and prints selected items.

extern crate skim;
use skim::prelude::*;

fn main() {
    let options = SkimOptions::default();

    let selected_items = Skim::run_with(options, None)
        .map(|out| out.selected_items)
        .unwrap_or_default();

    for item in &selected_items {
        println!("{}", item.output());
    }
}
