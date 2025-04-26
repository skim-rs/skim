use std::io;
use tuikit::output::Output;

fn main() {
    let output = Output::new(Box::new(io::stdout())).unwrap();
    let (width, height) = output.terminal_size().unwrap();
    println!("width: {}, height: {}", width, height);
}
