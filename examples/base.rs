//! Demonstrates basic item selection with inline status information.

use skim::prelude::*;
use skim::tui::statusline::InfoDisplay;

fn main() -> color_eyre::Result<()> {
    let opts = SkimOptionsBuilder::default()
        .multi(true)
        .reverse(true)
        .info(InfoDisplay::Inline)
        .build()?;
    let res = Skim::run_items(opts, ["hello", "world"])?;

    for item in res.selected_items {
        println!("Selected {} (id {})", item.output(), item.rank.index);
    }

    Ok(())
}
