use std::{
    io::stdout,
    ops::Deref,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, SendError, Sender},
    },
    thread::{self, JoinHandle, sleep},
    time::Duration,
};

use async_ratatui::BackgroundWidget;
use ratatui::{
    crossterm::{
        self,
        event::{self, KeyCode, KeyModifiers},
    },
    prelude::*,
    widgets::WidgetRef,
};

const TICK: Duration = Duration::from_millis(20);

struct BasicApp {
    should_quit: bool,
    sender: Option<Sender<Event>>,
    receiver: Option<Receiver<Event>>,
}

enum Event {
    Stop,
    Render(Arc<Buffer>),
    SetName(String),
}

struct BasicState {
    name: String,
}

impl StatefulWidget for &BasicApp {
    type State = Arc<Mutex<BasicState>>;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let state = state.lock().expect("Failed to acquire state during render");
        let greeting = format!("Hello, {}!", state.name);
        buf.set_string(area.x, area.y, greeting, Style::default());
    }
}

impl BasicApp {
    fn spawn(&mut self, state: Arc<Mutex<BasicState>>) -> JoinHandle<()> {
        let rx = self
            .receiver
            .take()
            .expect("Receiver not initialized before task spawn");
        thread::spawn(move || {
            for i in 0..=15 * 500 {
                let ev = rx.try_recv();
                if ev.is_ok() {
                    match ev.unwrap() {
                        Event::Stop => break,
                        Event::Render(_) => (),
                        Event::SetName(s) => {
                            let mut state = state.lock().expect("Failed to acquire state in bg thread");
                            state.name = s;
                        }
                    }
                }
                {
                    // let mut state = state.lock().expect("Failed to acquire state in bg thread");
                    // state.name = format!("Ran for {} ticks", i);
                }
                thread::sleep(TICK);
            }
            return;
        })
    }
    fn send(&self, ev: Event) -> Result<(), std::sync::mpsc::SendError<Event>> {
        if let Some(tx) = &self.sender {
            tx.send(ev)
        } else {
            panic!("Sender not initialized during send");
        }
    }
}
pub fn main() -> std::io::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut app = BasicApp {
        should_quit: false,
        sender: Some(tx),
        receiver: Some(rx),
    };
    let mut state = Arc::new(Mutex::new(BasicState {
        name: String::from("Hi"),
    }));
    let mut terminal = ratatui::init();
    let j = &app.spawn(state.clone());
    let mut i = 0;
    while !j.is_finished() {
        if event::poll(TICK)? {
            match event::read()? {
                event::Event::Key(k) => {
                    match (k.code, k.modifiers) {
                        (KeyCode::Char('c'), KeyModifiers::CONTROL) => app.send(Event::Stop),
                        (KeyCode::Char(c), _) => app.send(Event::SetName(format!("Key: {c}"))),
                        _ => Ok(()),
                    };
                }
                _ => (),
            };
        }
        terminal.draw(|frame| {
            frame.render_stateful_widget(&app, frame.area(), &mut state);
        });
        i += 1;
        // app.send(Event::SetName(format!("From main thread, at iteration {i}")));
    }
    ratatui::restore();
    Ok(())
}
