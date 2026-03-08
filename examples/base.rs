use skim::prelude::*;

fn main() -> color_eyre::Result<()> {
    let opts = SkimOptionsBuilder::default().multi(true).reverse(true).build()?;
    let res = Skim::run_items(opts, ["hello", "world"])?;

    for item in res.selected_items {
        println!("Selected {} (id {})", item.output(), item.get_index());
    }

    Ok(())
}
