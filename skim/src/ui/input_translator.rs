//! Input Translation Layer
//!
//! This module provides translation between different event systems:
//! - Crossterm events ↔ Skim events
//! - Legacy tuikit Key types ↔ Crossterm KeyEvent
//! - Skim Events ↔ Modern SkimMessage
//!
//! This enables gradual migration from tuikit to ratatui while maintaining
//! compatibility with the existing input system.

use crate::ui::events::{SkimEvent, SkimMessage};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::collections::HashMap;

/// Maps legacy skim event strings to modern SkimMessage
#[derive(Debug, Clone)]
pub struct InputTranslator {
    /// Key bindings from key combinations to action chains
    keymap: HashMap<KeyCombination, Vec<SkimMessage>>,
}

/// Represents a key combination for mapping
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct KeyCombination {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyCombination {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }
    
    pub fn char(ch: char) -> Self {
        Self::new(KeyCode::Char(ch), KeyModifiers::NONE)
    }
    
    pub fn ctrl(ch: char) -> Self {
        Self::new(KeyCode::Char(ch), KeyModifiers::CONTROL)
    }
    
    pub fn alt(ch: char) -> Self {
        Self::new(KeyCode::Char(ch), KeyModifiers::ALT)
    }
    
    pub fn key(code: KeyCode) -> Self {
        Self::new(code, KeyModifiers::NONE)
    }
}

impl From<KeyEvent> for KeyCombination {
    fn from(key_event: KeyEvent) -> Self {
        Self::new(key_event.code, key_event.modifiers)
    }
}

impl InputTranslator {
    /// Create a new input translator with default key bindings
    pub fn new() -> Self {
        Self {
            keymap: Self::create_default_keymap(),
        }
    }
    
    /// Translate a crossterm event to skim messages
    pub fn translate_event(&self, event: &SkimEvent) -> Vec<SkimMessage> {
        match event {
            SkimEvent::Key(key_event) => self.translate_key_event(key_event),
            SkimEvent::Mouse(mouse_event) => self.translate_mouse_event(mouse_event),
            SkimEvent::Resize(width, height) => vec![SkimMessage::Resize(*width, *height)],
            SkimEvent::Tick => vec![], // No action for tick events
        }
    }
    
    /// Translate a key event to skim messages
    fn translate_key_event(&self, key_event: &KeyEvent) -> Vec<SkimMessage> {
        let combination = KeyCombination::from(*key_event);
        
        // Check for explicit key bindings first
        if let Some(actions) = self.keymap.get(&combination) {
            return actions.clone();
        }
        
        // Character input should be handled by the query component, not here
        // InputTranslator only handles special key combinations that are already mapped
        vec![]
    }
    
    /// Translate mouse events to skim messages
    fn translate_mouse_event(&self, mouse_event: &MouseEvent) -> Vec<SkimMessage> {
        match mouse_event.kind {
            MouseEventKind::ScrollUp => vec![SkimMessage::ScrollUp],
            MouseEventKind::ScrollDown => vec![SkimMessage::ScrollDown],
            MouseEventKind::Down(MouseButton::Left) => {
                // Handle mouse click selection
                vec![SkimMessage::Custom(format!("click:{},{}", mouse_event.column, mouse_event.row))]
            }
            _ => vec![], // Other mouse events not handled yet
        }
    }
    
    /// Create the default keymap matching skim's behavior
    fn create_default_keymap() -> HashMap<KeyCombination, Vec<SkimMessage>> {
        let mut keymap = HashMap::new();
        
        // Navigation keys
        keymap.insert(KeyCombination::key(KeyCode::Up), vec![SkimMessage::SelectPrev]);
        keymap.insert(KeyCombination::key(KeyCode::Down), vec![SkimMessage::SelectNext]);
        keymap.insert(KeyCombination::ctrl('p'), vec![SkimMessage::SelectPrev]);
        keymap.insert(KeyCombination::ctrl('n'), vec![SkimMessage::SelectNext]);
        keymap.insert(KeyCombination::ctrl('k'), vec![SkimMessage::SelectPrev]);
        keymap.insert(KeyCombination::ctrl('j'), vec![SkimMessage::SelectNext]);
        
        // Vi-style navigation
        keymap.insert(KeyCombination::char('k'), vec![SkimMessage::SelectPrev]);
        keymap.insert(KeyCombination::char('j'), vec![SkimMessage::SelectNext]);
        
        // Page navigation
        keymap.insert(KeyCombination::key(KeyCode::PageUp), vec![SkimMessage::PageUp]);
        keymap.insert(KeyCombination::key(KeyCode::PageDown), vec![SkimMessage::PageDown]);
        keymap.insert(KeyCombination::ctrl('u'), vec![SkimMessage::PageUp]);
        keymap.insert(KeyCombination::ctrl('d'), vec![SkimMessage::PageDown]);
        
        // Selection
        keymap.insert(KeyCombination::key(KeyCode::Tab), vec![SkimMessage::ToggleSelection]);
        keymap.insert(KeyCombination::key(KeyCode::BackTab), vec![SkimMessage::DeselectAll]);
        
        // Accept/Abort
        keymap.insert(KeyCombination::key(KeyCode::Enter), vec![SkimMessage::Accept]);
        keymap.insert(KeyCombination::key(KeyCode::Esc), vec![SkimMessage::Abort]);
        keymap.insert(KeyCombination::ctrl('c'), vec![SkimMessage::Abort]);
        keymap.insert(KeyCombination::ctrl('g'), vec![SkimMessage::Abort]);
        keymap.insert(KeyCombination::ctrl('q'), vec![SkimMessage::Abort]);
        
        // Query editing
        keymap.insert(KeyCombination::key(KeyCode::Backspace), vec![SkimMessage::Custom("backspace".to_string())]);
        keymap.insert(KeyCombination::ctrl('h'), vec![SkimMessage::Custom("backspace".to_string())]);
        keymap.insert(KeyCombination::key(KeyCode::Delete), vec![SkimMessage::Custom("delete".to_string())]);
        keymap.insert(KeyCombination::ctrl('w'), vec![SkimMessage::Custom("kill-word".to_string())]);
        keymap.insert(KeyCombination::ctrl('u'), vec![SkimMessage::ClearQuery]);
        
        // Cursor movement
        keymap.insert(KeyCombination::key(KeyCode::Left), vec![SkimMessage::MoveCursor(-1)]);
        keymap.insert(KeyCombination::key(KeyCode::Right), vec![SkimMessage::MoveCursor(1)]);
        keymap.insert(KeyCombination::ctrl('b'), vec![SkimMessage::MoveCursor(-1)]);
        keymap.insert(KeyCombination::ctrl('f'), vec![SkimMessage::MoveCursor(1)]);
        keymap.insert(KeyCombination::key(KeyCode::Home), vec![SkimMessage::Custom("beginning-of-line".to_string())]);
        keymap.insert(KeyCombination::key(KeyCode::End), vec![SkimMessage::Custom("end-of-line".to_string())]);
        keymap.insert(KeyCombination::ctrl('a'), vec![SkimMessage::Custom("beginning-of-line".to_string())]);
        keymap.insert(KeyCombination::ctrl('e'), vec![SkimMessage::Custom("end-of-line".to_string())]);
        
        // Word movement
        keymap.insert(KeyCombination::alt('b'), vec![SkimMessage::Custom("backward-word".to_string())]);
        keymap.insert(KeyCombination::alt('f'), vec![SkimMessage::Custom("forward-word".to_string())]);
        
        // Toggle actions
        keymap.insert(KeyCombination::key(KeyCode::F(2)), vec![SkimMessage::TogglePreview]);
        keymap.insert(KeyCombination::ctrl('r'), vec![SkimMessage::ToggleSort]);
        
        // Selection actions
        keymap.insert(KeyCombination::ctrl('a'), vec![SkimMessage::SelectAll]);
        keymap.insert(KeyCombination::alt('a'), vec![SkimMessage::SelectAll]);
        keymap.insert(KeyCombination::alt('d'), vec![SkimMessage::DeselectAll]);
        
        keymap
    }
    
    /// Add a custom key binding
    pub fn bind_key(&mut self, key_combination: KeyCombination, actions: Vec<SkimMessage>) {
        self.keymap.insert(key_combination, actions);
    }
    
    /// Remove a key binding
    pub fn unbind_key(&mut self, key_combination: &KeyCombination) {
        self.keymap.remove(key_combination);
    }
    
    /// Get all current key bindings
    pub fn get_bindings(&self) -> &HashMap<KeyCombination, Vec<SkimMessage>> {
        &self.keymap
    }
    
    /// Parse a key combination from a string (e.g., "ctrl-c", "alt-enter")
    pub fn parse_key_combination(key_str: &str) -> Option<KeyCombination> {
        let key_str = key_str.to_lowercase();
        let parts: Vec<&str> = key_str.split('-').collect();
        
        if parts.is_empty() {
            return None;
        }
        
        let mut modifiers = KeyModifiers::NONE;
        let key_part = parts.last().unwrap();
        
        // Parse modifiers
        for part in &parts[..parts.len() - 1] {
            match *part {
                "ctrl" => modifiers |= KeyModifiers::CONTROL,
                "alt" => modifiers |= KeyModifiers::ALT,
                "shift" => modifiers |= KeyModifiers::SHIFT,
                _ => return None, // Unknown modifier
            }
        }
        
        // Parse key code
        let code = match *key_part {
            "space" => KeyCode::Char(' '),
            "tab" => KeyCode::Tab,
            "enter" => KeyCode::Enter,
            "backspace" | "bs" => KeyCode::Backspace,
            "delete" | "del" => KeyCode::Delete,
            "esc" => KeyCode::Esc,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            key if key.starts_with('f') && key.len() > 1 => {
                // Function keys (f1, f2, etc.)
                if let Ok(num) = key[1..].parse::<u8>() {
                    if num >= 1 && num <= 12 {
                        KeyCode::F(num)
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            key if key.len() == 1 => {
                // Single character
                KeyCode::Char(key.chars().next().unwrap())
            }
            _ => return None, // Unknown key
        };
        
        Some(KeyCombination::new(code, modifiers))
    }
    
    /// Parse an action string to SkimMessage
    pub fn parse_action(action_str: &str) -> Option<SkimMessage> {
        match action_str {
            "abort" => Some(SkimMessage::Abort),
            "accept" => Some(SkimMessage::Accept),
            "select-next" | "down" => Some(SkimMessage::SelectNext),
            "select-prev" | "up" => Some(SkimMessage::SelectPrev),
            "page-down" => Some(SkimMessage::PageDown),
            "page-up" => Some(SkimMessage::PageUp),
            "scroll-down" => Some(SkimMessage::ScrollDown),
            "scroll-up" => Some(SkimMessage::ScrollUp),
            "toggle-selection" | "toggle" => Some(SkimMessage::ToggleSelection),
            "select-all" => Some(SkimMessage::SelectAll),
            "deselect-all" => Some(SkimMessage::DeselectAll),
            "toggle-preview" => Some(SkimMessage::TogglePreview),
            "toggle-sort" => Some(SkimMessage::ToggleSort),
            "clear-query" => Some(SkimMessage::ClearQuery),
            "redraw" => Some(SkimMessage::Redraw),
            action if action.starts_with("execute:") => {
                let command = &action[8..];
                Some(SkimMessage::Custom(format!("execute:{}", command)))
            }
            _ => {
                // Custom action
                Some(SkimMessage::Custom(action_str.to_string()))
            }
        }
    }
}

impl Default for InputTranslator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_combination_creation() {
        let ctrl_c = KeyCombination::ctrl('c');
        assert_eq!(ctrl_c.code, KeyCode::Char('c'));
        assert_eq!(ctrl_c.modifiers, KeyModifiers::CONTROL);
        
        let alt_enter = KeyCombination::new(KeyCode::Enter, KeyModifiers::ALT);
        assert_eq!(alt_enter.code, KeyCode::Enter);
        assert_eq!(alt_enter.modifiers, KeyModifiers::ALT);
    }
    
    #[test]
    fn test_key_combination_parsing() {
        let ctrl_c = InputTranslator::parse_key_combination("ctrl-c").unwrap();
        assert_eq!(ctrl_c.code, KeyCode::Char('c'));
        assert_eq!(ctrl_c.modifiers, KeyModifiers::CONTROL);
        
        let alt_enter = InputTranslator::parse_key_combination("alt-enter").unwrap();
        assert_eq!(alt_enter.code, KeyCode::Enter);
        assert_eq!(alt_enter.modifiers, KeyModifiers::ALT);
        
        let f1 = InputTranslator::parse_key_combination("f1").unwrap();
        assert_eq!(f1.code, KeyCode::F(1));
        assert_eq!(f1.modifiers, KeyModifiers::NONE);
        
        // Test invalid combinations
        assert!(InputTranslator::parse_key_combination("invalid-key").is_none());
        assert!(InputTranslator::parse_key_combination("ctrl-invalid").is_none());
    }
    
    #[test]
    fn test_action_parsing() {
        assert!(matches!(InputTranslator::parse_action("abort"), Some(SkimMessage::Abort)));
        assert!(matches!(InputTranslator::parse_action("accept"), Some(SkimMessage::Accept)));
        assert!(matches!(InputTranslator::parse_action("select-next"), Some(SkimMessage::SelectNext)));
        assert!(matches!(InputTranslator::parse_action("toggle"), Some(SkimMessage::ToggleSelection)));
        
        // Test custom actions
        if let Some(SkimMessage::Custom(action)) = InputTranslator::parse_action("custom-action") {
            assert_eq!(action, "custom-action");
        } else {
            panic!("Should parse custom action");
        }
        
        // Test execute actions
        if let Some(SkimMessage::Custom(action)) = InputTranslator::parse_action("execute:echo hello") {
            assert_eq!(action, "execute:echo hello");
        } else {
            panic!("Should parse execute action");
        }
    }
    
    #[test]
    fn test_default_keymap() {
        let translator = InputTranslator::new();
        
        // Test basic navigation
        let up_key = KeyCombination::key(KeyCode::Up);
        assert_eq!(translator.keymap.get(&up_key), Some(&vec![SkimMessage::SelectPrev]));
        
        let down_key = KeyCombination::key(KeyCode::Down);
        assert_eq!(translator.keymap.get(&down_key), Some(&vec![SkimMessage::SelectNext]));
        
        // Test control keys
        let ctrl_c = KeyCombination::ctrl('c');
        assert_eq!(translator.keymap.get(&ctrl_c), Some(&vec![SkimMessage::Abort]));
        
        let enter_key = KeyCombination::key(KeyCode::Enter);
        assert_eq!(translator.keymap.get(&enter_key), Some(&vec![SkimMessage::Accept]));
    }
    
    #[test]
    fn test_event_translation() {
        let translator = InputTranslator::new();
        
        // Test key event translation
        let key_event = KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        let skim_event = SkimEvent::Key(key_event);
        let messages = translator.translate_event(&skim_event);
        assert_eq!(messages, vec![SkimMessage::SelectPrev]);
        
        // Test character input
        let char_event = KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        let skim_event = SkimEvent::Key(char_event);
        let messages = translator.translate_event(&skim_event);
        assert_eq!(messages, vec![SkimMessage::UpdateQuery("a".to_string())]);
        
        // Test resize event
        let resize_event = SkimEvent::Resize(80, 24);
        let messages = translator.translate_event(&resize_event);
        assert_eq!(messages, vec![SkimMessage::Resize(80, 24)]);
    }
    
    #[test]
    fn test_custom_bindings() {
        let mut translator = InputTranslator::new();
        
        // Add custom binding
        let custom_key = KeyCombination::ctrl('z');
        let custom_actions = vec![SkimMessage::Custom("suspend".to_string())];
        translator.bind_key(custom_key.clone(), custom_actions.clone());
        
        assert_eq!(translator.keymap.get(&custom_key), Some(&custom_actions));
        
        // Remove binding
        translator.unbind_key(&custom_key);
        assert_eq!(translator.keymap.get(&custom_key), None);
    }
}