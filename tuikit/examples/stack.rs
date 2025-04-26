use tuikit::prelude::*;

struct Model(String);

impl Draw for Model {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        let (width, height) = canvas.size()?;
        let message_width = self.0.len();
        let left = (width - message_width) / 2;
        let top = height / 2;
        let _ = canvas.print(top, left, &self.0);
        Ok(())
    }
}

impl Widget<String> for Model {
    fn on_event(&self, event: Event, _rect: Rectangle) -> Vec<String> {
        if let Event::Key(Key::MousePress(_, _, _)) = event {
            vec![format!("{} clicked", self.0)]
        } else {
            vec![]
        }
    }
}

fn main() {
    let term = Term::with_options(TermOptions::default().mouse_enabled(true)).unwrap();
    let (width, height) = term.term_size().unwrap();

    while let Ok(ev) = term.poll_event() {
        match ev {
            Event::Key(Key::Char('q')) | Event::Key(Key::Ctrl('c')) => break,
            _ => (),
        }
        let _ = term.print(1, 1, "press 'q' to exit, try click on windows");

        let stack = Stack::<String>::new()
            .top(
                Win::new(Model("win floating on top".to_string()))
                    .border(true)
                    .margin(Size::Percent(30)),
            )
            .bottom(
                HSplit::default()
                    .split(Win::new(Model("left".to_string())).border(true))
                    .split(Win::new(Model("right".to_string())).border(true)),
            );

        let message = stack.on_event(
            ev,
            Rectangle {
                width,
                height,
                top: 0,
                left: 0,
            },
        );
        let click_message = if message.is_empty() { "" } else { &message[0] };
        let _ = term.print(2, 1, click_message);
        let _ = term.draw(&stack);
        let _ = term.present();
    }
    let _ = term.set_cursor(0, 0);
    let _ = term.show_cursor(true);
}
