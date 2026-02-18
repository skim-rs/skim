use skim::prelude::*;

#[tokio::main]
pub async fn main() -> color_eyre::eyre::Result<()> {
    let opts = SkimOptionsBuilder::default().cmd("cat bench_data.txt").build()?;

    println!("START");
    let mut skim = Skim::init(opts, None)?;
    skim.start();
    skim.init_tui()?;
    skim.enter().await?;
    while !skim.tick().await? {
        if skim.reader_done() && skim.matcher_stopped() {
            skim.event_sender()
                .send(Event::Action(Action::Accept(Some(String::from("Done")))))
                .await?;
        }
    }
    println!("DONE: {:?}", skim.output());
    color_eyre::eyre::Ok(())
}
