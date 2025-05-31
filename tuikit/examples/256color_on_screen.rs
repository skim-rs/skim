use crossterm::style::{Color, ContentStyle, Stylize as _};
use std::io;
use tuikit::canvas::Canvas;
use tuikit::output::Output;
use tuikit::screen::Screen;

fn main() {
    let mut output = Output::new(Box::new(io::stdout())).unwrap();
    let (width, height) = output.terminal_size().unwrap();
    let mut screen = Screen::new(width, height);

    for fg in 0..=255 {
        let _ = screen.print_with_style(
            fg / 16,
            (fg % 16) * 5,
            format!("{:5}", fg).as_str(),
            ContentStyle::default().with(Color::AnsiValue(fg as u8)),
        );
    }

    let _ = screen.set_cursor(15, 80);
    let _ = screen.present();
    output.flush();

    let _ = screen.print_with_style(0, 78, "HELLO WORLD", ContentStyle::default());
    let _ = screen.present();
    output.flush();

    for bg in 0..=255 {
        let _ = screen.print_with_style(
            bg / 16,
            (bg % 16) * 5,
            format!("{:5}", bg).as_str(),
            ContentStyle {
                background_color: Some(Color::AnsiValue(bg as u8)),
                ..ContentStyle::default()
            },
        );
    }
    let _ = screen.present();
    output.reset_attributes();
    output.flush()
}
