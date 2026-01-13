extern crate skim;
use skim::{prelude::*, reader::CommandCollector};

pub fn main() {
    env_logger::init();

    let glogm = Some(String::from("git log --oneline --color=always | head -n10"));

    let options = SkimOptionsBuilder::default()
        .height(String::from("50%"))
        .cmd(glogm)
        .preview(Some(String::from("echo {}")))
        .multi(true)
        .reverse(true)
        .cmd_collector(Rc::new(RefCell::new(SkimItemReader::new(
            SkimItemReaderOption::default().ansi(true),
        ))) as Rc<RefCell<dyn CommandCollector>>)
        .build()
        .unwrap();

    log::debug!("Options: ansi {}", options.ansi);

    let selected_items = Skim::run_with(options, None)
        .map(|out| out.selected_items)
        .unwrap_or_default();

    for item in selected_items.iter() {
        print!("selected: {}{}", item.output(), "\n");
    }
}
