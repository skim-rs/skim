#![allow(missing_docs, clippy::pedantic)]

#[allow(dead_code)]
#[macro_use]
mod common;

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use skim::tui::BorderType;

fn mouse_down(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn list_inner_area(h: &common::insta::TestHarness) -> Rect {
    let app = h.skim.app();
    let list_area = app.layout.list_area;

    if !matches!(app.options.border, BorderType::None | BorderType::ForceOff) {
        return Rect {
            x: list_area.x + 1,
            y: list_area.y + 1,
            width: list_area.width.saturating_sub(2),
            height: list_area.height.saturating_sub(2),
        };
    }

    list_area
}

fn mouse_down_list_row(h: &common::insta::TestHarness, row: u16) -> MouseEvent {
    let inner = list_inner_area(h);
    mouse_down(inner.x, inner.y + row)
}

insta_test!(
    mouse_selection_refreshes_preview,
    ["first", "second", "third"],
    &["--layout", "reverse", "--preview", "echo preview:{}"],
    {
        @snap;
        @mouse(|h| mouse_down_list_row(h, 1));
        @snap;
    }
);
