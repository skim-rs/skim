use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent};

/// Events that can be sent to the UI system
#[derive(Debug, Clone)]
pub enum SkimEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Tick,
}

/// Messages that represent state changes in the application
#[derive(Debug, Clone, PartialEq)]
pub enum SkimMessage {
    // Query messages
    UpdateQuery(String),
    MoveCursor(isize),
    ClearQuery,
    
    // Selection messages
    SelectNext,
    SelectPrev,
    SelectFirst,
    SelectLast,
    ToggleSelection,
    SelectAll,
    DeselectAll,
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
    
    // Preview messages
    PreviewScroll,
    PreviewToggleWrap,
    
    // Global actions
    Accept,
    Abort,
    TogglePreview,
    ToggleSort,
    
    // Internal events
    Resize(u16, u16),
    Redraw,
    
    // Custom user events
    Custom(String),
}

/// Result of event handling
#[derive(Debug, Clone)]
pub enum EventResult {
    Consumed,
    Ignored,
    Exit(SkimMessage),
}

/// Convert crossterm events to skim events
impl From<crossterm::event::Event> for SkimEvent {
    fn from(event: crossterm::event::Event) -> Self {
        match event {
            crossterm::event::Event::Key(key) => SkimEvent::Key(key),
            crossterm::event::Event::Mouse(mouse) => SkimEvent::Mouse(mouse),
            crossterm::event::Event::Resize(w, h) => SkimEvent::Resize(w, h),
            crossterm::event::Event::FocusGained | crossterm::event::Event::FocusLost => {
                SkimEvent::Tick
            }
            crossterm::event::Event::Paste(content) => {
                // Handle paste events by converting to text input
                // For now, treat as a tick event since paste handling requires query integration
                SkimEvent::Tick
            }
        }
    }
}

/// Helper functions for key event matching
impl SkimEvent {
    pub fn is_key(&self, code: KeyCode) -> bool {
        matches!(self, SkimEvent::Key(KeyEvent { code: c, .. }) if *c == code)
    }
    
    pub fn is_key_with_mod(&self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        matches!(self, SkimEvent::Key(KeyEvent { code: c, modifiers: m, .. }) if *c == code && *m == modifiers)
    }
    
    pub fn is_char(&self, ch: char) -> bool {
        matches!(self, SkimEvent::Key(KeyEvent { code: KeyCode::Char(c), .. }) if *c == ch)
    }
    
    pub fn is_ctrl(&self, ch: char) -> bool {
        self.is_key_with_mod(KeyCode::Char(ch), KeyModifiers::CONTROL)
    }
    
    pub fn is_alt(&self, ch: char) -> bool {
        self.is_key_with_mod(KeyCode::Char(ch), KeyModifiers::ALT)
    }
    
    pub fn is_shift(&self, code: KeyCode) -> bool {
        self.is_key_with_mod(code, KeyModifiers::SHIFT)
    }
}