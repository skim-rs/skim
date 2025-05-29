//! Legacy Bridge for Tuikit Compatibility
//!
//! This module provides compatibility layers for transitioning from tuikit to ratatui.
//! It includes mock implementations of tuikit types and conversion utilities.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton};
use ratatui::style::{Color, Modifier, Style};
use std::fmt;

/// Mock tuikit Key type for compatibility during transition
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LegacyKey {
    // Character keys
    Char(char),
    
    // Special keys
    Backspace,
    Enter,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Tab,
    BackTab,
    Delete,
    Insert,
    F(u8),
    Esc,
    
    // Mouse events (simplified)
    MousePress(MouseButton, u16, u16),
    MouseRelease(u16, u16),
    MouseHold(u16, u16),
    WheelUp(u16, u16, usize),
    WheelDown(u16, u16, usize),
    
    // Special values
    Null,
    Unknown,
    
    // Combined with modifiers (for convenience)
    Ctrl(char),
    Alt(char),
    CtrlAlt(char),
    
    // Key combinations
    ESC,
    AltBackspace,
    ShiftLeft,
    ShiftRight,
    ShiftUp,
    ShiftDown,
    CtrlLeft,
    CtrlRight,
    
    // Bracketed paste events
    BracketedPasteStart,
    BracketedPasteEnd,
}

impl LegacyKey {
    /// Convert crossterm KeyEvent to LegacyKey
    pub fn from_crossterm(key_event: KeyEvent) -> Self {
        match (key_event.code, key_event.modifiers) {
            (KeyCode::Char(ch), KeyModifiers::NONE) => LegacyKey::Char(ch),
            (KeyCode::Char(ch), KeyModifiers::CONTROL) => LegacyKey::Ctrl(ch),
            (KeyCode::Char(ch), KeyModifiers::ALT) => LegacyKey::Alt(ch),
            (KeyCode::Char(ch), modifiers) if modifiers.contains(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                LegacyKey::CtrlAlt(ch)
            }
            (KeyCode::Backspace, KeyModifiers::NONE) => LegacyKey::Backspace,
            (KeyCode::Backspace, KeyModifiers::ALT) => LegacyKey::AltBackspace,
            (KeyCode::Enter, _) => LegacyKey::Enter,
            (KeyCode::Left, KeyModifiers::NONE) => LegacyKey::Left,
            (KeyCode::Left, KeyModifiers::SHIFT) => LegacyKey::ShiftLeft,
            (KeyCode::Left, KeyModifiers::CONTROL) => LegacyKey::CtrlLeft,
            (KeyCode::Right, KeyModifiers::NONE) => LegacyKey::Right,
            (KeyCode::Right, KeyModifiers::SHIFT) => LegacyKey::ShiftRight,
            (KeyCode::Right, KeyModifiers::CONTROL) => LegacyKey::CtrlRight,
            (KeyCode::Up, KeyModifiers::NONE) => LegacyKey::Up,
            (KeyCode::Up, KeyModifiers::SHIFT) => LegacyKey::ShiftUp,
            (KeyCode::Down, KeyModifiers::NONE) => LegacyKey::Down,
            (KeyCode::Down, KeyModifiers::SHIFT) => LegacyKey::ShiftDown,
            (KeyCode::Home, _) => LegacyKey::Home,
            (KeyCode::End, _) => LegacyKey::End,
            (KeyCode::PageUp, _) => LegacyKey::PageUp,
            (KeyCode::PageDown, _) => LegacyKey::PageDown,
            (KeyCode::Tab, _) => LegacyKey::Tab,
            (KeyCode::BackTab, _) => LegacyKey::BackTab,
            (KeyCode::Delete, _) => LegacyKey::Delete,
            (KeyCode::Insert, _) => LegacyKey::Insert,
            (KeyCode::F(num), _) => LegacyKey::F(num),
            (KeyCode::Esc, _) => LegacyKey::ESC,
            // Note: Bracketed paste events would need special handling in a real implementation
            _ => LegacyKey::Unknown,
        }
    }
    
    /// Convert LegacyKey back to crossterm KeyEvent (best effort)
    pub fn to_crossterm(&self) -> Option<KeyEvent> {
        match self {
            LegacyKey::Char(ch) => Some(KeyEvent {
                code: KeyCode::Char(*ch),
                modifiers: KeyModifiers::NONE,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            LegacyKey::Ctrl(ch) => Some(KeyEvent {
                code: KeyCode::Char(*ch),
                modifiers: KeyModifiers::CONTROL,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            LegacyKey::Alt(ch) => Some(KeyEvent {
                code: KeyCode::Char(*ch),
                modifiers: KeyModifiers::ALT,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            LegacyKey::CtrlAlt(ch) => Some(KeyEvent {
                code: KeyCode::Char(*ch),
                modifiers: KeyModifiers::CONTROL | KeyModifiers::ALT,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            LegacyKey::Backspace => Some(KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            LegacyKey::Enter => Some(KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            LegacyKey::Left => Some(KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            LegacyKey::Right => Some(KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            LegacyKey::Up => Some(KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            LegacyKey::Down => Some(KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            LegacyKey::Home => Some(KeyEvent {
                code: KeyCode::Home,
                modifiers: KeyModifiers::NONE,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            LegacyKey::End => Some(KeyEvent {
                code: KeyCode::End,
                modifiers: KeyModifiers::NONE,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            LegacyKey::PageUp => Some(KeyEvent {
                code: KeyCode::PageUp,
                modifiers: KeyModifiers::NONE,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            LegacyKey::PageDown => Some(KeyEvent {
                code: KeyCode::PageDown,
                modifiers: KeyModifiers::NONE,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            LegacyKey::Tab => Some(KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            LegacyKey::BackTab => Some(KeyEvent {
                code: KeyCode::BackTab,
                modifiers: KeyModifiers::NONE,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            LegacyKey::Delete => Some(KeyEvent {
                code: KeyCode::Delete,
                modifiers: KeyModifiers::NONE,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            LegacyKey::Insert => Some(KeyEvent {
                code: KeyCode::Insert,
                modifiers: KeyModifiers::NONE,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            LegacyKey::F(num) => Some(KeyEvent {
                code: KeyCode::F(*num),
                modifiers: KeyModifiers::NONE,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            LegacyKey::Esc => Some(KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::NONE,
            }),
            // Mouse events and special keys don't translate directly
            _ => None,
        }
    }
    
    /// Check if this is a character key
    pub fn is_char(&self) -> bool {
        matches!(self, LegacyKey::Char(_) | LegacyKey::Ctrl(_) | LegacyKey::Alt(_) | LegacyKey::CtrlAlt(_))
    }
    
    /// Get the character if this is a character key
    pub fn get_char(&self) -> Option<char> {
        match self {
            LegacyKey::Char(ch) => Some(*ch),
            LegacyKey::Ctrl(ch) => Some(*ch),
            LegacyKey::Alt(ch) => Some(*ch),
            LegacyKey::CtrlAlt(ch) => Some(*ch),
            _ => None,
        }
    }
    
    /// Check if this key has control modifier
    pub fn has_ctrl(&self) -> bool {
        matches!(self, LegacyKey::Ctrl(_) | LegacyKey::CtrlAlt(_))
    }
    
    /// Check if this key has alt modifier
    pub fn has_alt(&self) -> bool {
        matches!(self, LegacyKey::Alt(_) | LegacyKey::CtrlAlt(_))
    }
}

impl fmt::Display for LegacyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LegacyKey::Char(ch) => write!(f, "{}", ch),
            LegacyKey::Ctrl(ch) => write!(f, "Ctrl+{}", ch),
            LegacyKey::Alt(ch) => write!(f, "Alt+{}", ch),
            LegacyKey::CtrlAlt(ch) => write!(f, "Ctrl+Alt+{}", ch),
            LegacyKey::Backspace => write!(f, "Backspace"),
            LegacyKey::Enter => write!(f, "Enter"),
            LegacyKey::Left => write!(f, "Left"),
            LegacyKey::Right => write!(f, "Right"),
            LegacyKey::Up => write!(f, "Up"),
            LegacyKey::Down => write!(f, "Down"),
            LegacyKey::Home => write!(f, "Home"),
            LegacyKey::End => write!(f, "End"),
            LegacyKey::PageUp => write!(f, "PageUp"),
            LegacyKey::PageDown => write!(f, "PageDown"),
            LegacyKey::Tab => write!(f, "Tab"),
            LegacyKey::BackTab => write!(f, "BackTab"),
            LegacyKey::Delete => write!(f, "Delete"),
            LegacyKey::Insert => write!(f, "Insert"),
            LegacyKey::F(num) => write!(f, "F{}", num),
            LegacyKey::Esc => write!(f, "Esc"),
            LegacyKey::MousePress(btn, x, y) => write!(f, "Mouse({:?} press at {},{})", btn, x, y),
            LegacyKey::MouseRelease(x, y) => write!(f, "Mouse(release at {},{})", x, y),
            LegacyKey::MouseHold(x, y) => write!(f, "Mouse(hold at {},{})", x, y),
            LegacyKey::WheelUp(x, y, count) => write!(f, "Wheel(up {} at {},{})", count, x, y),
            LegacyKey::WheelDown(x, y, count) => write!(f, "Wheel(down {} at {},{})", count, x, y),
            LegacyKey::Null => write!(f, "Null"),
            LegacyKey::Unknown => write!(f, "Unknown"),
            LegacyKey::ESC => write!(f, "ESC"),
            LegacyKey::AltBackspace => write!(f, "AltBackspace"),
            LegacyKey::ShiftLeft => write!(f, "ShiftLeft"),
            LegacyKey::ShiftRight => write!(f, "ShiftRight"),
            LegacyKey::ShiftUp => write!(f, "ShiftUp"),
            LegacyKey::ShiftDown => write!(f, "ShiftDown"),
            LegacyKey::BracketedPasteStart => write!(f, "BracketedPasteStart"),
            LegacyKey::BracketedPasteEnd => write!(f, "BracketedPasteEnd"),
            LegacyKey::CtrlLeft => write!(f, "CtrlLeft"),
            LegacyKey::CtrlRight => write!(f, "CtrlRight"),
        }
    }
}

/// Parse a key name string to LegacyKey (for compatibility with old key binding format)
pub fn from_keyname(keyname: &str) -> Option<LegacyKey> {
    match keyname.to_lowercase().as_str() {
        "space" => Some(LegacyKey::Char(' ')),
        "tab" => Some(LegacyKey::Tab),
        "btab" | "shift-tab" => Some(LegacyKey::BackTab),
        "enter" => Some(LegacyKey::Enter),
        "backspace" | "bs" => Some(LegacyKey::Backspace),
        "delete" | "del" => Some(LegacyKey::Delete),
        "insert" | "ins" => Some(LegacyKey::Insert),
        "esc" => Some(LegacyKey::Esc),
        "up" => Some(LegacyKey::Up),
        "down" => Some(LegacyKey::Down),
        "left" => Some(LegacyKey::Left),
        "right" => Some(LegacyKey::Right),
        "home" => Some(LegacyKey::Home),
        "end" => Some(LegacyKey::End),
        "pageup" => Some(LegacyKey::PageUp),
        "pagedown" => Some(LegacyKey::PageDown),
        
        // Function keys
        key if key.starts_with('f') && key.len() > 1 => {
            if let Ok(num) = key[1..].parse::<u8>() {
                if num >= 1 && num <= 12 {
                    Some(LegacyKey::F(num))
                } else {
                    None
                }
            } else {
                None
            }
        }
        
        // Control combinations
        key if key.starts_with("ctrl-") && key.len() == 6 => {
            let ch = key.chars().nth(5).unwrap();
            Some(LegacyKey::Ctrl(ch))
        }
        
        // Alt combinations
        key if key.starts_with("alt-") && key.len() == 5 => {
            let ch = key.chars().nth(4).unwrap();
            Some(LegacyKey::Alt(ch))
        }
        
        // Single character
        key if key.len() == 1 => {
            Some(LegacyKey::Char(key.chars().next().unwrap()))
        }
        
        _ => None,
    }
}

/// Mock tuikit Event type for compatibility
#[derive(Debug, Clone)]
pub enum LegacyTermEvent {
    Key(LegacyKey),
    Mouse { x: u16, y: u16, button: MouseButton },
    Resize { width: u16, height: u16 },
    Unknown,
}

impl LegacyTermEvent {
    /// Convert crossterm event to legacy term event
    pub fn from_crossterm(event: crossterm::event::Event) -> Self {
        match event {
            crossterm::event::Event::Key(key_event) => {
                LegacyTermEvent::Key(LegacyKey::from_crossterm(key_event))
            }
            crossterm::event::Event::Mouse(mouse_event) => {
                LegacyTermEvent::Mouse {
                    x: mouse_event.column,
                    y: mouse_event.row,
                    button: MouseButton::Left, // Simplified
                }
            }
            crossterm::event::Event::Resize(width, height) => {
                LegacyTermEvent::Resize { width, height }
            }
            _ => LegacyTermEvent::Unknown,
        }
    }
}

/// Mock tuikit Attr type for compatibility during transition
/// This provides a bridge between tuikit's Attr system and ratatui's Style system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LegacyAttr {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub modifiers: Modifier,
    // For compatibility with tuikit's field access pattern
    pub effect: Modifier,
}

impl Default for LegacyAttr {
    fn default() -> Self {
        Self {
            fg: None,
            bg: None,
            modifiers: Modifier::empty(),
            effect: Modifier::empty(),
        }
    }
}

impl LegacyAttr {
    /// Create empty attr
    pub fn empty() -> Self {
        Self::default()
    }
    
    /// Create attr with foreground color
    pub fn with_fg(fg: Color) -> Self {
        Self {
            fg: Some(fg),
            bg: None,
            modifiers: Modifier::empty(),
            effect: Modifier::empty(),
        }
    }
    
    /// Create attr with background color
    pub fn with_bg(bg: Color) -> Self {
        Self {
            fg: None,
            bg: Some(bg),
            modifiers: Modifier::empty(),
            effect: Modifier::empty(),
        }
    }
    
    /// Create attr with effect/modifier
    pub fn with_effect(modifier: Modifier) -> Self {
        Self {
            fg: None,
            bg: None,
            modifiers: modifier,
            effect: modifier,
        }
    }
    
    /// Convert to ratatui Style
    pub fn to_style(&self) -> Style {
        let mut style = Style::default();
        if let Some(fg) = self.fg {
            style = style.fg(fg);
        }
        if let Some(bg) = self.bg {
            style = style.bg(bg);
        }
        // Use both modifiers and effect field
        style = style.add_modifier(self.modifiers | self.effect);
        style
    }
    
    /// Create from ratatui Style
    pub fn from_style(style: Style) -> Self {
        Self {
            fg: style.fg,
            bg: style.bg,
            modifiers: style.add_modifier,
            effect: style.add_modifier,
        }
    }
    
    /// Set foreground color (chainable, for tuikit compatibility)
    pub fn fg(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }
    
    /// Set background color (chainable, for tuikit compatibility)
    pub fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }
    
    /// Add modifier/effect (chainable, for tuikit compatibility)
    pub fn add_modifier(mut self, modifier: Modifier) -> Self {
        self.modifiers |= modifier;
        self.effect |= modifier;
        self
    }
    
    /// Set effect (chainable, for tuikit compatibility)
    pub fn effect(mut self, modifier: Modifier) -> Self {
        self.modifiers |= modifier;
        self.effect |= modifier;
        self
    }
    
    /// Merge with another attr (other takes precedence)
    pub fn merge(&self, other: &LegacyAttr) -> LegacyAttr {
        LegacyAttr {
            fg: other.fg.or(self.fg),
            bg: other.bg.or(self.bg),
            modifiers: self.modifiers | other.modifiers,
            effect: self.effect | other.effect,
        }
    }
    
    /// Extend with another attr (same as merge)
    pub fn extend(self, other: LegacyAttr) -> LegacyAttr {
        self.merge(&other)
    }
}

/// Mock tuikit Effect type (maps to ratatui Modifier)
pub type LegacyEffect = Modifier;

/// Legacy color constants for compatibility
#[allow(dead_code)]
pub mod legacy_colors {
    use ratatui::style::Color;
    
    pub const BLACK: Color = Color::Black;
    pub const RED: Color = Color::Red;
    pub const GREEN: Color = Color::Green;
    pub const YELLOW: Color = Color::Yellow;
    pub const BLUE: Color = Color::Blue;
    pub const MAGENTA: Color = Color::Magenta;
    pub const CYAN: Color = Color::Cyan;
    pub const WHITE: Color = Color::White;
    pub const DARK_GRAY: Color = Color::DarkGray;
    pub const LIGHT_RED: Color = Color::LightRed;
    pub const LIGHT_GREEN: Color = Color::LightGreen;
    pub const LIGHT_YELLOW: Color = Color::LightYellow;
    pub const LIGHT_BLUE: Color = Color::LightBlue;
    pub const LIGHT_MAGENTA: Color = Color::LightMagenta;
    pub const LIGHT_CYAN: Color = Color::LightCyan;
    pub const GRAY: Color = Color::Gray;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_key_conversion() {
        // Test character key
        let key_event = KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        let legacy_key = LegacyKey::from_crossterm(key_event);
        assert_eq!(legacy_key, LegacyKey::Char('a'));
        
        // Test control key
        let ctrl_key_event = KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        let legacy_ctrl_key = LegacyKey::from_crossterm(ctrl_key_event);
        assert_eq!(legacy_ctrl_key, LegacyKey::Ctrl('c'));
        
        // Test round-trip conversion
        if let Some(converted_back) = legacy_key.to_crossterm() {
            assert_eq!(converted_back.code, KeyCode::Char('a'));
            assert_eq!(converted_back.modifiers, KeyModifiers::NONE);
        } else {
            panic!("Should be able to convert back");
        }
    }
    
    #[test]
    fn test_key_name_parsing() {
        assert_eq!(from_keyname("ctrl-c"), Some(LegacyKey::Ctrl('c')));
        assert_eq!(from_keyname("alt-a"), Some(LegacyKey::Alt('a')));
        assert_eq!(from_keyname("space"), Some(LegacyKey::Char(' ')));
        assert_eq!(from_keyname("enter"), Some(LegacyKey::Enter));
        assert_eq!(from_keyname("f1"), Some(LegacyKey::F(1)));
        assert_eq!(from_keyname("f12"), Some(LegacyKey::F(12)));
        assert_eq!(from_keyname("a"), Some(LegacyKey::Char('a')));
        
        // Test invalid names
        assert_eq!(from_keyname("invalid"), None);
        assert_eq!(from_keyname("f13"), None); // Out of range
    }
    
    #[test]
    fn test_key_properties() {
        let char_key = LegacyKey::Char('x');
        assert!(char_key.is_char());
        assert_eq!(char_key.get_char(), Some('x'));
        assert!(!char_key.has_ctrl());
        assert!(!char_key.has_alt());
        
        let ctrl_key = LegacyKey::Ctrl('c');
        assert!(ctrl_key.is_char());
        assert_eq!(ctrl_key.get_char(), Some('c'));
        assert!(ctrl_key.has_ctrl());
        assert!(!ctrl_key.has_alt());
        
        let alt_key = LegacyKey::Alt('a');
        assert!(alt_key.is_char());
        assert_eq!(alt_key.get_char(), Some('a'));
        assert!(!alt_key.has_ctrl());
        assert!(alt_key.has_alt());
        
        let special_key = LegacyKey::Enter;
        assert!(!special_key.is_char());
        assert_eq!(special_key.get_char(), None);
    }
    
    #[test]
    fn test_display_formatting() {
        assert_eq!(format!("{}", LegacyKey::Char('a')), "a");
        assert_eq!(format!("{}", LegacyKey::Ctrl('c')), "Ctrl+c");
        assert_eq!(format!("{}", LegacyKey::Alt('x')), "Alt+x");
        assert_eq!(format!("{}", LegacyKey::Enter), "Enter");
        assert_eq!(format!("{}", LegacyKey::F(1)), "F1");
    }
}