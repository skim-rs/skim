#![allow(missing_docs, clippy::pedantic)]

#[allow(dead_code)]
#[macro_use]
mod common;

use color_eyre::Result;
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use skim::tui::Event;
use std::time::{Duration, Instant};

#[test]
fn mouse_selection_refreshes_preview() -> Result<()> {
    let options = common::insta::parse_options(&["--layout", "reverse", "--preview", "echo preview:{}"]);
    let mut h = common::insta::enter_items(["first", "second", "third"], options)?;

    h.prepare_snap()?;
    assert!(h.buffer_view().contains("preview:first"));

    let list_area = h.skim.app().layout.list_area;
    h.send(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: list_area.x,
        row: list_area.y + 1,
        modifiers: KeyModifiers::NONE,
    }))?;
    h.tick()?;

    let start = Instant::now();
    loop {
        h.prepare_snap()?;

        let buffer = h.buffer_view();
        if buffer.contains("preview:second") {
            return Ok(());
        }

        if start.elapsed() > Duration::from_secs(1) {
            panic!("expected preview for second item\n{buffer}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
