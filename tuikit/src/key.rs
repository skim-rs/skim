//! Key compatibility layer for crossterm migration
//!
//! This module provides compatibility for code that expects the old tuikit Key types.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use std::fmt;

/// Key event type - compatibility wrapper around crossterm's KeyEvent
#[derive(Eq, PartialEq, Hash, Debug, Copy, Clone)]
pub enum Key {
    // Character keys
    Char(char),

    // Special keys
    Backspace,
    Delete,
    Enter,
    Esc,
    Tab,
    BackTab,

    // Arrow keys
    Up,
    Down,
    Left,
    Right,

    // Page keys
    PageUp,
    PageDown,
    Home,
    End,
    Insert,

    // Function keys
    F(u8), // F1-F12

    // Control sequences
    Ctrl(char),
    Alt(char),
    CtrlAlt(char),

    // Mouse events (legacy support)
    WheelUp(u16, u16, usize),
    WheelDown(u16, u16, usize),
    SingleClick(MouseButton, u16, u16),
    DoubleClick(MouseButton, u16, u16),

    // Bracketed paste
    BracketedPasteStart,
    BracketedPasteEnd,

    // Unknown/unsupported
    Unknown,

    // Null key for compatibility
    Null,
}

impl From<KeyEvent> for Key {
    fn from(key_event: KeyEvent) -> Self {
        let KeyEvent { code, modifiers, .. } = key_event;

        match code {
            KeyCode::Char(c) => {
                if modifiers.contains(KeyModifiers::CONTROL) && modifiers.contains(KeyModifiers::ALT) {
                    Key::CtrlAlt(c)
                } else if modifiers.contains(KeyModifiers::CONTROL) {
                    Key::Ctrl(c)
                } else if modifiers.contains(KeyModifiers::ALT) {
                    Key::Alt(c)
                } else {
                    Key::Char(c)
                }
            }
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Delete => Key::Delete,
            KeyCode::Enter => Key::Enter,
            KeyCode::Esc => Key::Esc,
            KeyCode::Tab => Key::Tab,
            KeyCode::BackTab => Key::BackTab,
            KeyCode::Up => Key::Up,
            KeyCode::Down => Key::Down,
            KeyCode::Left => Key::Left,
            KeyCode::Right => Key::Right,
            KeyCode::PageUp => Key::PageUp,
            KeyCode::PageDown => Key::PageDown,
            KeyCode::Home => Key::Home,
            KeyCode::End => Key::End,
            KeyCode::Insert => Key::Insert,
            KeyCode::F(n) => Key::F(n),
            _ => Key::Unknown,
        }
    }
}

impl From<crossterm::event::Event> for Key {
    fn from(event: crossterm::event::Event) -> Self {
        match event {
            crossterm::event::Event::Key(key_event) => Key::from(key_event),
            crossterm::event::Event::Mouse(mouse_event) => {
                match mouse_event.kind {
                    MouseEventKind::ScrollUp => Key::WheelUp(mouse_event.row, mouse_event.column, 1),
                    MouseEventKind::ScrollDown => Key::WheelDown(mouse_event.row, mouse_event.column, 1),
                    MouseEventKind::Down(button) => Key::SingleClick(button, mouse_event.row, mouse_event.column),
                    // For now, treat double click same as single click
                    MouseEventKind::Up(button) => Key::SingleClick(button, mouse_event.row, mouse_event.column),
                    _ => Key::Unknown,
                }
            }
            crossterm::event::Event::Paste(_) => Key::BracketedPasteStart, // Simplified
            _ => Key::Unknown,
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Key::Char(c) => write!(f, "{}", c),
            Key::Ctrl(c) => write!(f, "Ctrl-{}", c),
            Key::Alt(c) => write!(f, "Alt-{}", c),
            Key::CtrlAlt(c) => write!(f, "Ctrl-Alt-{}", c),
            Key::F(n) => write!(f, "F{}", n),
            Key::Backspace => write!(f, "Backspace"),
            Key::Delete => write!(f, "Delete"),
            Key::Enter => write!(f, "Enter"),
            Key::Esc => write!(f, "Esc"),
            Key::Tab => write!(f, "Tab"),
            Key::BackTab => write!(f, "BackTab"),
            Key::Up => write!(f, "Up"),
            Key::Down => write!(f, "Down"),
            Key::Left => write!(f, "Left"),
            Key::Right => write!(f, "Right"),
            Key::PageUp => write!(f, "PageUp"),
            Key::PageDown => write!(f, "PageDown"),
            Key::Home => write!(f, "Home"),
            Key::End => write!(f, "End"),
            Key::Insert => write!(f, "Insert"),
            Key::WheelUp(row, col, count) => write!(f, "WheelUp({},{},{})", row, col, count),
            Key::WheelDown(row, col, count) => write!(f, "WheelDown({},{},{})", row, col, count),
            Key::SingleClick(button, row, col) => write!(f, "Click({:?},{},{})", button, row, col),
            Key::DoubleClick(button, row, col) => write!(f, "DoubleClick({:?},{},{})", button, row, col),
            Key::BracketedPasteStart => write!(f, "BracketedPasteStart"),
            Key::BracketedPasteEnd => write!(f, "BracketedPasteEnd"),
            Key::Unknown => write!(f, "Unknown"),
            Key::Null => write!(f, "Null"),
        }
    }
}

/// Parse a key name string into a Key
pub fn from_keyname(keyname: &str) -> Option<Key> {
    match keyname.to_lowercase().as_str() {
        "backspace" | "bs" => Some(Key::Backspace),
        "delete" | "del" => Some(Key::Delete),
        "enter" | "return" => Some(Key::Enter),
        "escape" | "esc" => Some(Key::Esc),
        "tab" => Some(Key::Tab),
        "backtab" | "shift-tab" => Some(Key::BackTab),
        "up" => Some(Key::Up),
        "down" => Some(Key::Down),
        "left" => Some(Key::Left),
        "right" => Some(Key::Right),
        "pageup" | "page-up" => Some(Key::PageUp),
        "pagedown" | "page-down" => Some(Key::PageDown),
        "home" => Some(Key::Home),
        "end" => Some(Key::End),
        "insert" | "ins" => Some(Key::Insert),
        s if s.starts_with("f") => {
            s[1..]
                .parse::<u8>()
                .ok()
                .and_then(|n| if n >= 1 && n <= 12 { Some(Key::F(n)) } else { None })
        }
        s if s.starts_with("ctrl-") => {
            let rest = &s[5..];
            if rest.len() == 1 {
                rest.chars().next().map(Key::Ctrl)
            } else {
                None
            }
        }
        s if s.starts_with("alt-") => {
            let rest = &s[4..];
            if rest.len() == 1 {
                rest.chars().next().map(Key::Alt)
            } else {
                None
            }
        }
        s if s.len() == 1 => s.chars().next().map(Key::Char),
        _ => None,
    }
}
