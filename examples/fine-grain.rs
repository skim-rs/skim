extern crate skim;
use color_eyre::Result;
use skim::prelude::*;

#[tokio::main(flavor = "current_thread")]
pub async fn main() -> Result<()> {
    let options = SkimOptionsBuilder::default().height("50%").multi(true).build()?;

    let (tx_item, rx_item): (SkimItemSender, SkimItemReceiver) = unbounded();
    let mut skim = Skim::init(options, Some(rx_item))?;

    skim.start();
    skim.init_tui()?;

    let event_tx = skim.event_sender();

    skim.enter().await?;

    let output = skim
        .run_until(async move {
            for i in 1..=10 {
                let _ = event_tx.try_send(Event::ClearItems);
                let _ = tx_item.send(vec![Arc::new(format!("item {i}")) as Arc<dyn SkimItem>]);
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        })
        .await?;

    for item in output.selected_items.iter() {
        println!("{}", item.output());
    }

    Ok(())
}
