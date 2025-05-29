//! Demo application showcasing the new ratatui-based UI system
//!
//! This is a standalone demo that shows all UI components working together
//! without dependencies on the legacy tuikit-based code.

use crate::ui::{
    SkimUI, SkimTerminal, 
    events::SkimEvent,
    layout,
};
use crate::item::MatchedItem;
use crate::SkimItem;
use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyModifiers};
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Demo application for the new UI system
pub struct UIDemo {
    ui_state: SkimUI,
    terminal: SkimTerminal,
    should_exit: bool,
}

impl UIDemo {
    /// Create a new demo instance
    pub fn new() -> io::Result<Self> {
        // Create a simple options structure (bypassing SkimOptionsBuilder for now)
        let header_text = "Skim Ratatui Demo - Press 'q' or Esc to quit, F2 to toggle preview";
        
        let mut ui_state = SkimUI::default();
        
        // Configure UI state manually
        ui_state.layout_config.layout_mode = "default".to_string();
        
        // Set header content manually
        ui_state.header_state.header_lines = vec![
            crate::ansi::AnsiString::parse(header_text)
        ];
        
        // Create demo data with proper MatchedItem construction
        let demo_items: Vec<MatchedItem> = (0..20)
            .map(|i| {
                let text = format!("Demo item {} - this is a sample line for testing", i + 1);
                let item: Arc<dyn SkimItem> = Arc::new(text);
                MatchedItem {
                    item,
                    rank: Default::default(),
                    matched_range: None,
                    item_idx: i as u32,
                }
            })
            .collect();
        
        ui_state.selection_state.set_items(demo_items);
        ui_state.selection_state.set_multi_selection(true);
        
        // Configure status
        ui_state.status_state.update_status(100, 5, 0, 2, 0);
        ui_state.status_state.set_matcher_running(true);
        
        // Configure preview
        ui_state.layout_config.preview_direction = Some(ratatui::layout::Direction::Horizontal);
        ui_state.layout_config.preview_size = 40;
        ui_state.previewer_state.set_visible(true);
        ui_state.previewer_state.set_plain_text(
            "This is a preview of the selected file.\n\
             Line 2 of the preview.\n\
             Line 3 with some content.\n\
             Line 4...\n\
             Line 5...\n\
             And more lines to demonstrate scrolling.\n\
             Line 7\n\
             Line 8\n\
             Line 9\n\
             Line 10".to_string()
        );
        
        let terminal = SkimTerminal::new()?;
        
        Ok(Self {
            ui_state,
            terminal,
            should_exit: false,
        })
    }
    
    /// Run the demo
    pub fn run(&mut self) -> io::Result<()> {
        loop {
            // Render
            self.terminal.draw(|frame| {
                let layout_areas = layout::calculate_layout(frame.size(), &self.ui_state.layout_config);
                crate::ui::view(&self.ui_state, frame);
            })?;
            
            // Handle events
            if crossterm::event::poll(Duration::from_millis(100))? {
                let event = crossterm::event::read()?;
                let skim_event = SkimEvent::from(event);
                
                if self.handle_event(&skim_event) {
                    break;
                }
            }
            
            if self.should_exit {
                break;
            }
        }
        
        Ok(())
    }
    
    /// Handle a single event
    fn handle_event(&mut self, event: &SkimEvent) -> bool {
        match event {
            SkimEvent::Key(key_event) => {
                match (key_event.code, key_event.modifiers) {
                    // Exit keys
                    (KeyCode::Esc, _) | (KeyCode::Char('q'), _) => {
                        self.should_exit = true;
                        return true;
                    }
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        self.should_exit = true;
                        return true;
                    }
                    
                    // Navigation
                    (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                        self.ui_state.selection_state.select_next();
                    }
                    (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                        self.ui_state.selection_state.select_prev();
                    }
                    
                    // Selection
                    (KeyCode::Tab, _) => {
                        self.ui_state.selection_state.toggle_current_selection();
                    }
                    
                    // Preview toggle
                    (KeyCode::F(2), _) => {
                        let visible = !self.ui_state.previewer_state.visible;
                        self.ui_state.previewer_state.set_visible(visible);
                    }
                    
                    // Pass other events to components
                    _ => {
                        // Route to query component
                        if let Some(_) = crate::ui::handle_query_event(&mut self.ui_state.query_state, event) {
                            // Update status with new query
                            if !self.ui_state.query_state.content.is_empty() {
                                self.ui_state.status_state.set_matcher_mode(format!("fuzzy: {}", self.ui_state.query_state.content));
                            }
                        }
                        
                        // Route to previewer for scrolling
                        crate::ui::handle_previewer_event(&mut self.ui_state.previewer_state, event);
                    }
                }
            }
            SkimEvent::Resize(width, height) => {
                // Handle resize
                let _ = self.terminal.resize(*width, *height);
            }
            _ => {}
        }
        
        false
    }
}

/// Run the UI demo
pub fn run_demo() -> io::Result<()> {
    let mut demo = UIDemo::new()?;
    demo.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demo_creation() {
        // This test needs a terminal environment to run
        // Skip in CI, headless environments, or when running as a test
        if std::env::var("CI").is_ok() || 
           std::env::var("TERM").unwrap_or_default().is_empty() ||
           std::env::var("DISPLAY").is_err() {
            return; // Skip in environments without proper terminal
        }
        
        let result = UIDemo::new();
        assert!(result.is_ok());
    }
}