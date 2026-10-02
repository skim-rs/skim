//! Demonstrates `Skim::new_items` and a buffered streaming preview callback.

use std::io::{BufWriter, Write};
use std::time::Duration;

use eyre::Result;
use skim::prelude::*;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let options = SkimOptionsBuilder::default()
        .height("50%")
        .preview_fn(PreviewCallback::streaming(|current, _selected, writer| {
            let mut writer = BufWriter::new(writer);
            if let Some(item) = current {
                for step in 1..=10 {
                    if writeln!(writer, "\x1b[32m{}\x1b[0m: step {step}/10", item.text()).is_err()
                        || writer.flush().is_err()
                    {
                        // Changing the selection closes the old preview's writer.
                        return;
                    }
                    // The callback runs on a worker, so this does not block the UI.
                    std::thread::sleep(Duration::from_millis(250));
                }
            }
        }))
        .build()?;

    // Starts the reader and initializes the TUI, but does not enter it yet.
    let mut skim = Skim::new_items(options, ["first", "second", "third"])?;
    skim.enter().await?;
    skim.run().await?;

    for item in skim.output().selected_items {
        println!("{}", item.output());
    }
    Ok(())
}
