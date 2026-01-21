extern crate skim;
use crossterm::event::{KeyCode, KeyModifiers};
use skim::prelude::*;

// No action is actually performed on your filesystem!
// This example only produce friendly print statements!

fn fake_delete_item(item: &str) {
    println!("Deleting item `{item}`...");
}

fn fake_create_item(item: &str) {
    println!("Creating a new item `{item}`...");
}

fn main() {
    // Note: `accept` is a keyword used define custom actions.
    // For full list of accepted keywords see `parse_event` in `src/event.rs`.
    // `delete` and `create` are arbitrary keywords used for this example.
    let options = SkimOptionsBuilder::default()
        .multi(true)
        .bind(vec!["bs:abort".into(), "enter:accept".into()])
        .build()
        .unwrap();

    if let Ok(out) = Skim::run_with(options, None) {
        match (out.final_key.code, out.final_key.modifiers) {
            // Delete each selected item
            (KeyCode::Backspace, KeyModifiers::NONE) => {
                out.selected_items.iter().for_each(|i| fake_delete_item(&i.text()))
            }
            // Create a new item based on the query
            (KeyCode::Enter, KeyModifiers::NONE) => fake_create_item(out.query.as_ref()),
            _ => (),
        }
    };
}
