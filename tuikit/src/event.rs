//! events a `Term` could return

use crate::key::Key;

#[derive(Eq, PartialEq, Hash, Debug, Copy, Clone)]
pub enum Event<UserEvent: Send + 'static = ()> {
    Key(Key),
    Resize {
        width: u16,
        height: u16,
    },
    Restarted,
    /// user defined signal 1
    User(UserEvent),

    #[doc(hidden)]
    __Nonexhaustive,
}

impl<UserEvent: Send + 'static> From<crossterm::event::Event> for Event<UserEvent> {
    fn from(event: crossterm::event::Event) -> Self {
        match event {
            crossterm::event::Event::Key(key_event) => Event::Key(Key::from(key_event)),
            crossterm::event::Event::Mouse(mouse_event) => {
                Event::Key(Key::from(crossterm::event::Event::Mouse(mouse_event)))
            }
            crossterm::event::Event::Resize(width, height) => Event::Resize { width, height },
            crossterm::event::Event::Paste(_) => Event::Key(Key::BracketedPasteStart),
            _ => Event::Key(Key::Unknown),
        }
    }
}
