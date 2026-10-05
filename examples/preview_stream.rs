//! Demonstrates `Skim::new_items` and a buffered streaming preview callback.

use std::fmt::Display;
use std::io::{BufWriter, Write};
use std::time::Duration;

use eyre::{OptionExt, Result};
use skim::prelude::*;
use skim::tui::options::PreviewLayout;

#[derive(Default)]
struct Latencies {
    computed: bool,
    tcp: Duration,
    kex: Duration,
    handshake_end: Duration,
}

impl Display for Latencies {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("- tcp:	{}\n", self.duration_fmt(self.tcp)))?;
        f.write_fmt(format_args!("- kex:	{}\n", self.duration_fmt(self.kex)))?;
        f.write_fmt(format_args!(
            "- handshake_end:	{}\n",
            self.duration_fmt(self.handshake_end)
        ))?;
        Ok(())
    }
}

impl Latencies {
    fn duration_fmt(&self, d: Duration) -> String {
        if self.computed {
            format!("{d:?}")
        } else {
            String::from("calculating...")
        }
    }

    async fn probe(&mut self, _host: &str) -> Result<()> {
        tokio::time::sleep(Duration::from_millis(1200)).await;
        self.computed = true;
        self.tcp = Duration::from_millis(12);
        self.kex = Duration::from_millis(102);
        self.handshake_end = Duration::from_millis(204);
        Ok(())
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // bridge handle: the callback runs on a plain OS thread, so it cannot .await
    let handle = tokio::runtime::Handle::current();
    let layout = PreviewLayout {
        pty: true,
        ..Default::default()
    };

    let options = SkimOptionsBuilder::default()
        .height("50%")
        .preview_window(layout)
        .preview_fn(PreviewCallback::streaming(move |current, _selected, writer| {
            let item = current.ok_or_eyre("No item under the cursor")?;
            let host = item.text().into_owned();
            let mut writer = BufWriter::new(writer);
            let mut latencies = Latencies::default();

            // Set New Line Mode so `\n` also resets the column
            write!(writer, "\x1b[20h")?;
            writeln!(writer, "{host}")?;
            writeln!(writer, "Latencies")?;

            write!(writer, "{latencies}")?;
            writer.flush()?;

            // was: send_ssh_probe(host).await? — block just this worker thread
            handle.block_on(latencies.probe(&host))?;
            // Move up three rows, reset the column, and clear each old latency line.
            write!(writer, "\x1b[3;1H\x1b[J")?;
            write!(writer, "{latencies}")?;
            writer.flush()?;
            Ok(())
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
