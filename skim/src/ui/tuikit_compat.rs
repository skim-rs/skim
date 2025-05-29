//! Minimal tuikit compatibility layer
//!
//! This module provides just enough compatibility to allow existing tuikit code
//! to compile during the migration period. These are minimal shims that will
//! be replaced with proper ratatui implementations.

use ratatui::style::Modifier;

// Re-export our legacy types with tuikit-compatible names
pub use crate::ui::{LegacyAttr as Attr, LegacyKey as Key, SkimEvent};
// Re-export Event from the right place for compatibility

// Create our own Effect type to avoid external type implementation issues
pub type Effect = Modifier;

// Effect constants for compatibility
pub mod effect {
    use super::*;
    
    pub const BOLD: Effect = Modifier::BOLD;
    pub const DIM: Effect = Modifier::DIM;
    pub const UNDERLINED: Effect = Modifier::UNDERLINED;
    pub const UNDERLINE: Effect = Modifier::UNDERLINED; // Alias
    pub const BLINK: Effect = Modifier::SLOW_BLINK;
    pub const REVERSE: Effect = Modifier::REVERSED;
    pub const ITALIC: Effect = Modifier::ITALIC;
    pub const STRIKETHROUGH: Effect = Modifier::CROSSED_OUT;
}

/// Mock Canvas trait - simplified version for compatibility
pub trait Canvas {
    fn put_cell(&mut self, row: usize, col: usize, ch: char, attr: Attr);
    fn print_with_attr(&mut self, row: usize, col: usize, text: &str, attr: Attr) -> DrawResult<usize>;
    fn clear(&mut self) -> DrawResult<()>;
    fn height(&self) -> usize;
    fn width(&self) -> usize;
    
    /// Get canvas size as (width, height) tuple
    fn size(&self) -> DrawResult<(usize, usize)> {
        Ok((self.width(), self.height()))
    }
    
    /// Print text at position (compatibility method)
    fn print(&mut self, row: usize, col: usize, text: &str) -> DrawResult<()> {
        self.print_with_attr(row, col, text, Attr::default())?;
        Ok(())
    }
    
    /// Put single character with attributes (returns width for chaining)
    fn put_char_with_attr(&mut self, row: usize, col: usize, ch: char, attr: Attr) -> DrawResult<usize> {
        self.put_cell(row, col, ch, attr);
        Ok(1) // Most characters have width 1
    }
    
    /// Set cursor position
    fn set_cursor(&mut self, _row: usize, _col: usize) -> DrawResult<()> {
        Ok(())
    }
    
    /// Show or hide cursor
    fn show_cursor(&mut self, _show: bool) -> DrawResult<()> {
        Ok(())
    }
}

/// Mock Draw trait for compatibility
pub trait Draw {
    fn draw(&mut self, canvas: &mut dyn Canvas) -> DrawResult<()>;
}

/// Mock DrawResult type
pub type DrawResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Mock Widget trait for compatibility
pub trait Widget<E> {
    fn on_event(&mut self, event: E, area: Rect) -> EventResult;
    
    /// Size hint for layout (optional method)
    fn size_hint(&self) -> (Option<usize>, Option<usize>) {
        (None, None)
    }
}

// Implement Widget for Box<dyn Widget>
impl<E> Widget<E> for Box<dyn Widget<E>> {
    fn on_event(&mut self, event: E, area: Rect) -> EventResult {
        self.as_mut().on_event(event, area)
    }
    
    fn size_hint(&self) -> (Option<usize>, Option<usize>) {
        self.as_ref().size_hint()
    }
}

/// Mock EventResult type  
#[derive(Debug, Clone)]
pub enum EventResult {
    Ignored,
    Consumed(Option<crate::event::Event>),
}

/// Mock Rect type for compatibility
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    // Legacy compatibility fields
    pub top: u16,
    pub left: u16,
}

impl Rect {
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self { 
            x, 
            y, 
            width, 
            height,
            top: y,
            left: x,
        }
    }
    
    pub fn area(&self) -> u16 {
        self.width * self.height
    }
}

/// Mock Term type for compatibility
pub struct Term {
    // Minimal fields for compatibility
}

impl Term {
    pub fn with_options(_options: TermOptions) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {})
    }
    
    pub fn enable_mouse_support(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    
    pub fn poll_event(&self) -> Result<SkimEvent, Box<dyn std::error::Error>> {
        // This is a stub - in practice would poll crossterm events
        Err("Not implemented".into())
    }
    
    pub fn send_event(&self, _event: SkimEvent) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    
    pub fn draw(&self, _widget: &dyn Widget<crate::event::Event>) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    
    pub fn present(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    
    pub fn restart(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    
    pub fn pause(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    
    pub fn term_size(&self) -> Result<(usize, usize), Box<dyn std::error::Error>> {
        // Return a default terminal size
        Ok((80, 24))
    }
}

/// Mock TermOptions type
#[derive(Default)]
pub struct TermOptions {
    // Minimal fields for compatibility
}

impl TermOptions {
    pub fn min_height(self, _height: Option<usize>) -> Self {
        self
    }
    
    pub fn height(self, _height: Option<usize>) -> Self {
        self
    }
    
    pub fn clear_on_exit(self, _clear: bool) -> Self {
        self
    }
    
    pub fn disable_alternate_screen(self, _disable: bool) -> Self {
        self
    }
    
    pub fn clear_on_start(self, _clear: bool) -> Self {
        self
    }
    
    pub fn hold(self, _hold: bool) -> Self {
        self
    }
}

/// Mock Win type for compatibility (layout wrapper)
pub struct Win<T> {
    content: T,
}

impl<T> Win<T> {
    pub fn new(content: T) -> Self {
        Self { content }
    }
    
    pub fn basis<S>(self, _basis: S) -> Self {
        self
    }
    
    pub fn grow(self, _grow: usize) -> Self {
        self
    }
    
    pub fn shrink(self, _shrink: usize) -> Self {
        self
    }
    
    pub fn border_attr(self, _attr: Attr) -> Self {
        self
    }
    
    pub fn border_bottom(self, _enabled: bool) -> Self {
        self
    }
    
    pub fn border_top(self, _enabled: bool) -> Self {
        self
    }
    
    pub fn border_left(self, _enabled: bool) -> Self {
        self
    }
    
    pub fn border_right(self, _enabled: bool) -> Self {
        self
    }
    
    pub fn margin_top<S>(self, _margin: S) -> Self {
        self
    }
    
    pub fn margin_bottom<S>(self, _margin: S) -> Self {
        self
    }
    
    pub fn margin_left<S>(self, _margin: S) -> Self {
        self
    }
    
    pub fn margin_right<S>(self, _margin: S) -> Self {
        self
    }
}

// Implement Widget for Win<T> where T implements Widget
impl<T, E> Widget<E> for Win<T> 
where 
    T: Widget<E>
{
    fn on_event(&mut self, event: E, area: Rect) -> EventResult {
        self.content.on_event(event, area)
    }
    
    fn size_hint(&self) -> (Option<usize>, Option<usize>) {
        self.content.size_hint()
    }
}

// Implement Canvas for Win<T> where T implements Canvas  
impl<T> Canvas for Win<T>
where 
    T: Canvas
{
    fn put_cell(&mut self, row: usize, col: usize, ch: char, attr: Attr) {
        self.content.put_cell(row, col, ch, attr)
    }
    
    fn print_with_attr(&mut self, row: usize, col: usize, text: &str, attr: Attr) -> DrawResult<usize> {
        self.content.print_with_attr(row, col, text, attr)
    }
    
    fn clear(&mut self) -> DrawResult<()> {
        self.content.clear()
    }
    
    fn height(&self) -> usize {
        self.content.height()
    }
    
    fn width(&self) -> usize {
        self.content.width()
    }
}

/// Mock HSplit type for compatibility
#[derive(Default)]
pub struct HSplit;

impl HSplit {
    pub fn basis(self, _basis: usize) -> Self {
        self
    }
    
    pub fn grow(self, _grow: usize) -> Self {
        self
    }
    
    pub fn shrink(self, _shrink: usize) -> Self {
        self
    }
    
    pub fn split<T>(self, _item: T) -> Self {
        self
    }
}

impl Widget<crate::event::Event> for HSplit {
    fn on_event(&mut self, _event: crate::event::Event, _area: Rect) -> EventResult {
        EventResult::Ignored
    }
}

impl Canvas for HSplit {
    fn put_cell(&mut self, _row: usize, _col: usize, _ch: char, _attr: Attr) {}
    fn print_with_attr(&mut self, _row: usize, _col: usize, _text: &str, _attr: Attr) -> DrawResult<usize> {
        Ok(_text.len())
    }
    fn clear(&mut self) -> DrawResult<()> { Ok(()) }
    fn height(&self) -> usize { 24 }
    fn width(&self) -> usize { 80 }
}

/// Mock VSplit type for compatibility
#[derive(Default)]
pub struct VSplit;

impl VSplit {
    pub fn basis(self, _basis: usize) -> Self {
        self
    }
    
    pub fn grow(self, _grow: usize) -> Self {
        self
    }
    
    pub fn shrink(self, _shrink: usize) -> Self {
        self
    }
    
    pub fn split<T>(self, _item: T) -> Self {
        self
    }
}

impl Widget<crate::event::Event> for VSplit {
    fn on_event(&mut self, _event: crate::event::Event, _area: Rect) -> EventResult {
        EventResult::Ignored
    }
}

impl Canvas for VSplit {
    fn put_cell(&mut self, _row: usize, _col: usize, _ch: char, _attr: Attr) {}
    fn print_with_attr(&mut self, _row: usize, _col: usize, _text: &str, _attr: Attr) -> DrawResult<usize> {
        Ok(_text.len())
    }
    fn clear(&mut self) -> DrawResult<()> { Ok(()) }
    fn height(&self) -> usize { 24 }
    fn width(&self) -> usize { 80 }
}

/// Mock MouseButton for compatibility
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Extended Key enum with mouse events for compatibility
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExtendedKey {
    // Re-export basic key variants
    Char(char),
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
    Ctrl(char),
    Alt(char),
    CtrlAlt(char),
    Null,
    Unknown,
    
    // Mouse events that some tuikit code expects
    SingleClick(MouseButton, u16, u16),
    DoubleClick(MouseButton, u16, u16),
    WheelUp(u16, u16, usize),
    WheelDown(u16, u16, usize),
}

// Provide conversion between our Key type and ExtendedKey
impl From<Key> for ExtendedKey {
    fn from(key: Key) -> Self {
        match key {
            Key::Char(c) => ExtendedKey::Char(c),
            Key::Backspace => ExtendedKey::Backspace,
            Key::Enter => ExtendedKey::Enter,
            Key::Left => ExtendedKey::Left,
            Key::Right => ExtendedKey::Right,
            Key::Up => ExtendedKey::Up,
            Key::Down => ExtendedKey::Down,
            Key::Home => ExtendedKey::Home,
            Key::End => ExtendedKey::End,
            Key::PageUp => ExtendedKey::PageUp,
            Key::PageDown => ExtendedKey::PageDown,
            Key::Tab => ExtendedKey::Tab,
            Key::BackTab => ExtendedKey::BackTab,
            Key::Delete => ExtendedKey::Delete,
            Key::Insert => ExtendedKey::Insert,
            Key::F(n) => ExtendedKey::F(n),
            Key::Esc => ExtendedKey::Esc,
            Key::Ctrl(c) => ExtendedKey::Ctrl(c),
            Key::Alt(c) => ExtendedKey::Alt(c),
            Key::CtrlAlt(c) => ExtendedKey::CtrlAlt(c),
            Key::Null => ExtendedKey::Null,
            Key::Unknown => ExtendedKey::Unknown,
            Key::MousePress(btn, x, y) => ExtendedKey::SingleClick(
                match btn {
                    crossterm::event::MouseButton::Left => MouseButton::Left,
                    crossterm::event::MouseButton::Right => MouseButton::Right,
                    crossterm::event::MouseButton::Middle => MouseButton::Middle,
                },
                x, y
            ),
            Key::WheelUp(x, y, count) => ExtendedKey::WheelUp(x, y, count),
            Key::WheelDown(x, y, count) => ExtendedKey::WheelDown(x, y, count),
            _ => ExtendedKey::Unknown,
        }
    }
}

/// Color constants for compatibility
pub mod color {
    pub use ratatui::style::Color::*;
}
/// Mock Size type for layout compatibility
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Size {
    #[default]
    Default,
    Fixed(u16),
    Percent(u16),
    Basis(u16),
}

impl Size {
    pub fn basis(value: u16) -> Self {
        Size::Basis(value)
    }
    
    pub fn fixed(value: u16) -> Self {
        Size::Fixed(value)
    }
    
    pub fn percent(value: u16) -> Self {
        Size::Percent(value)
    }
    
    pub fn calc_fixed_size(&self, total: usize, _default: usize) -> usize {
        match self {
            Size::Default => total,
            Size::Fixed(val) => *val as usize,
            Size::Percent(pct) => (total * (*pct as usize)) / 100,
            Size::Basis(val) => *val as usize,
        }
    }
}

/// Mock Direction type for layout compatibility
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}


/// Mock Rectangle type for compatibility (alias for Rect)
pub type Rectangle = Rect;

/// Mock TermHeight type for terminal height compatibility
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermHeight {
    Fixed(usize),
    Percent(usize),
}
