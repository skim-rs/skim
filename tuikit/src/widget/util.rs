use crossterm::event::{Event, MouseEvent};

use crate::widget::Rectangle;

pub fn adjust_event(event: &Event, inner_rect: Rectangle) -> Option<Event> {
    match event.as_mouse_event().map(|e| (e.kind, e.row, e.column)) {
        Some((_, row, col)) => {
            if inner_rect.contains(row, col) {
                let (row, col) = inner_rect.relative_to_origin(row, col);
                Some(Event::Mouse(MouseEvent {
                    row,
                    column: col,
                    ..event.as_mouse_event().unwrap()
                }))
            } else {
                None
            }
        }
        None => Some(event.clone()),
    }
}
