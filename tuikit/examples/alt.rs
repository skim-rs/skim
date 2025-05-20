use crossterm::{
    event::{
        read, DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste, EnableFocusChange,
        EnableMouseCapture, Event, KeyCode, KeyModifiers,
    },
    execute,
    style::{PrintStyledContent, Stylize as _},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

fn main() -> std::io::Result<()> {
    let mut output = std::io::stdout();
    enable_raw_mode()?;
    execute!(
        output,
        EnableBracketedPaste,
        EnableFocusChange,
        EnableMouseCapture,
        EnterAlternateScreen
    )?;
    execute!(output, PrintStyledContent("hello world".red()))?;
    loop {
        match read()? {
            Event::FocusGained => println!("FocusGained"),
            Event::FocusLost => println!("FocusLost"),
            Event::Mouse(event) => println!("mouse: {:?}", event),
            Event::Paste(data) => println!("paste: {:?}", data),
            Event::Resize(width, height) => println!("New size {}x{}", width, height),
            Event::Key(event) => match (event.code, event.modifiers) {
                (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                    execute!(
                        output,
                        DisableBracketedPaste,
                        DisableFocusChange,
                        DisableMouseCapture,
                        LeaveAlternateScreen,
                        PrintStyledContent("Exited".green())
                    )?;
                    disable_raw_mode()?;
                    return Ok(());
                }

                _ => {
                    println!("key: {:?}", event);
                }
            },
        }
    }
}
