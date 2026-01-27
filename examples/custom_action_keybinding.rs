extern crate skim;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use skim::prelude::*;
use skim::tui::event::{Action, ActionCallback, Event};
use std::io::Cursor;

/// This example demonstrates how to bind custom action callbacks to keyboard shortcuts.
///
/// It shows how to:
/// 1. Create custom action callbacks
/// 2. Bind them to specific key combinations
/// 3. Use them interactively in skim

fn main() {
    // Create a custom callback that adds a prefix to the query
    let add_prefix_callback = ActionCallback::new(|app: &mut skim::tui::App<'static>| {
        // Get current query and add prefix
        let current_query = app.input.value.clone();

        // Clear the line first, then add new content
        let mut events = vec![Event::Action(Action::UnixLineDiscard)];

        let prefix = "TODO: ";
        for ch in prefix.chars() {
            events.push(Event::Action(Action::AddChar(ch)));
        }

        // Add back the original query
        for ch in current_query.chars() {
            events.push(Event::Action(Action::AddChar(ch)));
        }

        Ok(events)
    });

    // Create a callback that selects all and exits
    let select_all_callback = ActionCallback::new(|app: &mut skim::tui::App<'static>| {
        let count = app.item_pool.len();

        Ok(vec![
            Event::Action(Action::SelectAll),
            Event::Action(Action::Accept(Some(format!("Selected {count} items")))),
        ])
    });

    // Build basic options
    let mut options = SkimOptionsBuilder::default()
        .multi(true)
        .prompt("Select> ".to_string())
        .header(Some(String::from(
            "<C-p>: add prefix to prompt\t<C-a>: select all and exit with count",
        )))
        .build()
        .unwrap();

    // Now manually add custom keybindings to the keymap
    // We can access the keymap directly since it's public

    // Bind Ctrl-P to add prefix
    options.keymap.insert(
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
        vec![Action::Custom(add_prefix_callback)],
    );

    // Bind Ctrl-A to select all with message
    options.keymap.insert(
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        vec![Action::Custom(select_all_callback)],
    );

    // Create sample items
    let items = vec![
        "Write documentation",
        "Fix bug #123",
        "Implement feature X",
        "Review pull request",
        "Update dependencies",
        "Refactor module Y",
        "Add unit tests",
        "Optimize performance",
    ];

    let item_reader = SkimItemReader::default();
    let input = items.join("\n");
    let item_source = item_reader.of_bufread(Cursor::new(input));

    // Run skim with our custom keybindings
    if let Ok(output) = Skim::run_with(options, Some(item_source)) {
        println!("output: {output:?}");
    } else {
        println!("\nAborted!");
    }
}
