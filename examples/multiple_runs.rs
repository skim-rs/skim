use skim::{Skim, prelude::SkimOptionsBuilder};

// Hint: use `ps -T -p $(pgrep -f target/debug/examples/multiple_runs)` to watch threads while the
// different invocations run, and make sure none is leaking through
fn main() {
    for i in 0..3 {
        let opts = SkimOptionsBuilder::default()
            .header(Some(format!("run {i}")))
            .build()
            .unwrap();
        let res = Skim::run_with(opts, None).unwrap();
        println!("run {i}: {:?}", res.selected_items.iter().next().map(|x| x.output()));
    }
    std::thread::sleep(std::time::Duration::from_secs(5));
}
