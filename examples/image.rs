//! Demonstrates image previews using a literal image path.
//!
//! Run with:
//! `cargo run --example image`

use skim::options::ImageProtocol;
use skim::prelude::*;

fn main() -> eyre::Result<()> {
    env_logger::init();

    let options = SkimOptionsBuilder::default()
        .preview("{}")
        .preview_window("right:60%")
        .image(ImageProtocol::Halfblocks)
        .build()?;

    let output = Skim::run_items(options, ["examples/Lenna.png"])?;

    for item in &output.selected_items {
        println!("{}", item.output());
    }

    Ok(())
}
