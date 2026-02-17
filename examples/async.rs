use skim::{Skim, prelude::SkimOptionsBuilder};

#[tokio::main]
async fn main() {
    let options = SkimOptionsBuilder::default().build().unwrap();
    Skim::run_with(options, None).unwrap();
}
