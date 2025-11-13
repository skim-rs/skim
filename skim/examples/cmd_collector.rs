extern crate skim;
use reader::CommandCollector;
use skim::prelude::*;

struct BasicSkimItem {
    value: String,
}

impl SkimItem for BasicSkimItem {
    fn text(&self) -> Cow<'_, str> {
        return Cow::Borrowed(&self.value);
    }
}

struct BasicCmdCollector {
    pub items: Vec<String>,
}

impl CommandCollector for BasicCmdCollector {
    fn invoke(&mut self, _cmd: &str, _components_to_stop: Arc<AtomicUsize>) -> (SkimItemReceiver, Sender<i32>) {
        let (tx, rx) = unbounded();
        let (tx_interrupt, _rx_interrupt) = unbounded();
        while let Some(value) = self.items.pop() {
            let item = BasicSkimItem { value };
            tx.send(Arc::from(item) as Arc<dyn SkimItem>).unwrap();
        }

        (rx, tx_interrupt)
    }
}

fn main() {
    let cmd_collector = BasicCmdCollector {
        items: vec![String::from("foo"), String::from("bar"), String::from("baz")],
    };
    let options = SkimOptionsBuilder::default()
        .cmd_collector(Rc::from(RefCell::from(cmd_collector)))
        .build()
        .unwrap();

    let selected_items = Skim::run_with(options, None)
        .map(|out| out.selected_items)
        .unwrap_or_default();

    for item in selected_items.iter() {
        println!("{}", item.output());
    }
}
