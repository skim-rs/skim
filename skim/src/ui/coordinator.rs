//! UI Coordinator for orchestrating all UI components
//!
//! This module provides the main UI coordination logic for the ratatui-based interface.
//! It manages the event loop, component state updates, and rendering coordination.

use crate::ui::{
    events::{SkimEvent, SkimMessage},
    InputTranslator,
    SkimUI, SkimTerminal,
    handle_query_event, handle_selection_event, handle_header_event, 
    handle_status_event, handle_previewer_event,
    layout::{self, LayoutAreas},
};
use crate::{SkimOptions, SkimItemReceiver, SkimItem, MatchEngineFactory, CaseMatching};
use crate::item::{MatchedItem, ItemPool};
use crate::engine::factory::{ExactOrFuzzyEngineFactory, AndOrEngineFactory};
use crate::matcher::{Matcher, MatcherControl};
use defer_drop::DeferDrop;
use std::rc::Rc;
use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyModifiers};
use ratatui::Frame;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Main UI coordinator that manages the application state and event loop
pub struct UICoordinator {
    /// UI state containing all component states
    ui_state: SkimUI,
    /// Terminal wrapper for ratatui
    terminal: SkimTerminal,
    /// Input translator for key mapping
    input_translator: InputTranslator,
    /// Whether the application should exit
    should_exit: bool,
    /// Last frame render time
    last_render: Instant,
    /// Minimum frame time (for FPS limiting)
    min_frame_time: Duration,
    /// Optional item source for receiving data
    item_source: Option<SkimItemReceiver>,
    /// Legacy matcher for filtering items (same as legacy system)
    matcher: Matcher,
    /// Item pool for storing all source items (same as legacy system)
    item_pool: Arc<DeferDrop<ItemPool>>,
    /// Current matcher control for running matcher
    matcher_control: Option<MatcherControl>,
}

impl UICoordinator {
    /// Create a new UI coordinator
    pub fn new(options: &SkimOptions) -> io::Result<Self> {
        let mut ui_state = SkimUI::default();
        
        // Configure UI state from options
        ui_state.query_state.with_options(options);
        ui_state.header_state.with_options(options);
        
        // Configure layout
        ui_state.layout_config.layout_mode = options.layout.clone();
        
        // Only enable preview if explicitly requested
        if options.preview.is_some() {
            ui_state.layout_config.preview_direction = Some(ratatui::layout::Direction::Horizontal);
            ui_state.layout_config.preview_size = 50; // Default 50%
            ui_state.previewer_state.set_visible(true);
        }
        
        // Configure partial screen mode
        if !options.height.is_empty() && options.height != "100%" {
            if let Ok(percentage) = options.height.trim_end_matches('%').parse::<u16>() {
                ui_state.layout_config.partial_screen = Some(percentage);
            }
        }
        
        // UI state initialized with defaults
        
        let terminal = SkimTerminal::new()?;
        let input_translator = InputTranslator::new();
        
        // Create matcher with proper rank builder (exactly like legacy system)
        let rank_builder = Arc::new(crate::item::RankBuilder::new(options.tiebreak.clone()));
        let exact_or_fuzzy = ExactOrFuzzyEngineFactory::builder()
            .exact_mode(options.exact)
            .rank_builder(rank_builder)
            .build();
        let engine_factory: Rc<dyn MatchEngineFactory> = Rc::new(AndOrEngineFactory::new(exact_or_fuzzy));
        let matcher = Matcher::builder(engine_factory).case(options.case).build();
        
        // Create item pool (same as legacy system)
        let item_pool = Arc::new(DeferDrop::new(ItemPool::new().lines_to_reserve(options.header_lines)));
        
        Ok(Self {
            ui_state,
            terminal,
            input_translator,
            should_exit: false,
            last_render: Instant::now(),
            min_frame_time: Duration::from_millis(16), // ~60 FPS limit
            item_source: None,
            matcher,
            item_pool,
            matcher_control: None,
        })
    }
    
    /// Run the main event loop
    pub fn run(&mut self) -> io::Result<()> {
        // Initial render (no initial filtering needed - matcher will handle it)
        self.render()?;
        
        loop {
            // Process matcher results (same as legacy system heartbeat)
            self.process_matcher_results();
            
            // Handle events
            if self.handle_events()? {
                break;
            }
            
            // Render if needed (with FPS limiting)
            let now = Instant::now();
            if now.duration_since(self.last_render) >= self.min_frame_time {
                self.render()?;
                self.last_render = now;
            }
            
            if self.should_exit {
                break;
            }
            
            // Small sleep to prevent busy waiting
            std::thread::sleep(Duration::from_millis(10));
        }
        
        // Cleanup matcher before exit
        if let Some(ctrl) = self.matcher_control.take() {
            ctrl.kill();
        }
        
        // Explicit cleanup to ensure terminal is properly restored
        self.terminal.shutdown()?;
        
        Ok(())
    }
    
    /// Handle input events and update UI state
    fn handle_events(&mut self) -> io::Result<bool> {
        // Check for incoming items first
        self.process_incoming_items();
        
        // Check for crossterm events with timeout
        if crossterm::event::poll(Duration::from_millis(50))? {
            let event = crossterm::event::read()?;
            let skim_event = SkimEvent::from(event);
            
            // Translate event to messages using input translator
            let messages = self.input_translator.translate_event(&skim_event);
            
            // Process all messages
            for message in messages {
                match message {
                    SkimMessage::Accept | SkimMessage::Abort => {
                        self.should_exit = true;
                        return Ok(true);
                    }
                    SkimMessage::Resize(width, height) => {
                        self.terminal.resize(width, height)?;
                    }
                    _ => {
                        self.handle_component_message(message);
                    }
                }
            }
            
            // Also route raw events to components for any component-specific handling
            self.route_event_to_components(&skim_event);
        }
        
        Ok(false)
    }
    
    /// Handle global events that affect the entire application
    fn handle_global_events(&mut self, event: &SkimEvent) -> Option<SkimMessage> {
        match event {
            SkimEvent::Key(key_event) => {
                match (key_event.code, key_event.modifiers) {
                    // Global exit keys
                    (KeyCode::Esc, _) => Some(SkimMessage::Abort),
                    (KeyCode::Enter, _) => Some(SkimMessage::Accept),
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => Some(SkimMessage::Abort),
                    (KeyCode::Char('d'), KeyModifiers::CONTROL) => Some(SkimMessage::Abort),
                    
                    // Global toggle keys
                    (KeyCode::Tab, _) => {
                        self.ui_state.selection_state.toggle_multi_selection();
                        Some(SkimMessage::ToggleSelection)
                    }
                    
                    // Preview toggle
                    (KeyCode::F(2), _) => {
                        let visible = !self.ui_state.previewer_state.visible;
                        self.ui_state.previewer_state.set_visible(visible);
                        Some(SkimMessage::TogglePreview)
                    }
                    
                    _ => None,
                }
            }
            SkimEvent::Resize(width, height) => {
                Some(SkimMessage::Resize(*width, *height))
            }
            _ => None,
        }
    }
    
    /// Route events to appropriate components
    fn route_event_to_components(&mut self, event: &SkimEvent) {
        // Route to query component (has highest priority for text input)
        if let Some(message) = handle_query_event(&mut self.ui_state.query_state, event) {
            self.handle_component_message(message);
        }
        
        // Route to selection component
        if let Some(message) = handle_selection_event(&mut self.ui_state.selection_state, event) {
            self.handle_component_message(message);
        }
        
        // Route to header component
        if let Some(message) = handle_header_event(&mut self.ui_state.header_state, event) {
            self.handle_component_message(message);
        }
        
        // Route to status component
        if let Some(message) = handle_status_event(&mut self.ui_state.status_state, event) {
            self.handle_component_message(message);
        }
        
        // Route to previewer component
        if let Some(message) = handle_previewer_event(&mut self.ui_state.previewer_state, event) {
            self.handle_component_message(message);
        }
    }
    
    /// Handle messages from components
    fn handle_component_message(&mut self, message: SkimMessage) {
        match message {
            SkimMessage::UpdateQuery(new_query) => {
                // Update query and trigger search
                self.ui_state.query_state.content = new_query;
                // Restart matcher with new query (same as legacy system)
                self.restart_matcher();
            }
            SkimMessage::SelectNext => {
                self.ui_state.selection_state.select_next();
            }
            SkimMessage::SelectPrev => {
                self.ui_state.selection_state.select_prev();
            }
            SkimMessage::ToggleSelection => {
                self.ui_state.selection_state.toggle_current_selection();
            }
            SkimMessage::Accept => {
                self.should_exit = true;
            }
            SkimMessage::Abort => {
                self.should_exit = true;
            }
            SkimMessage::PreviewScroll | SkimMessage::PreviewToggleWrap => {
                // Preview events are handled by the previewer component internally
            }
            _ => {
                // Handle other messages as needed
            }
        }
    }
    
    /// Render the UI
    fn render(&mut self) -> io::Result<()> {
        let ui_state = &self.ui_state;
        self.terminal.draw(|frame| {
            Self::render_frame_static(frame, ui_state);
        })
    }
    
    /// Render a single frame
    fn render_frame_static(frame: &mut Frame, ui_state: &SkimUI) {
        // Calculate layout
        let layout_areas = layout::calculate_layout(frame.size(), &ui_state.layout_config);
        
        // Render components in order
        Self::render_components_static(frame, &layout_areas, ui_state);
    }
    
    /// Render all UI components
    fn render_components_static(frame: &mut Frame, layout: &LayoutAreas, ui_state: &SkimUI) {
        // Render header if present
        if let Some(header_area) = layout.header {
            crate::ui::render_header(&ui_state.header_state, frame, header_area);
        }
        
        // Render selection
        crate::ui::render_selection(&ui_state.selection_state, frame, layout.selection);
        
        // Render status
        crate::ui::render_status(&ui_state.status_state, frame, layout.status);
        
        // Render query
        crate::ui::render_query(&ui_state.query_state, frame, layout.query);
        
        // Render preview if visible
        if ui_state.previewer_state.visible {
            if let Some(preview_area) = layout.preview {
                crate::ui::render_preview(&ui_state.previewer_state, frame, preview_area);
            }
        }
    }
    
    /// Get the current UI state (for external access)
    pub fn ui_state(&self) -> &SkimUI {
        &self.ui_state
    }
    
    /// Get mutable UI state (for external updates)
    pub fn ui_state_mut(&mut self) -> &mut SkimUI {
        &mut self.ui_state
    }
    
    /// Set the item source for receiving data
    pub fn set_item_source(&mut self, item_source: SkimItemReceiver) {
        self.item_source = Some(item_source);
    }
    
    
    /// Update status information
    pub fn update_status(&mut self, total: usize, matched: usize, selected: usize, current: usize) {
        self.ui_state.status_state.update_status(total, matched, 0, selected, current);
    }
    
    /// Process incoming items from the item source
    fn process_incoming_items(&mut self) {
        // Collect items first to avoid borrowing issues
        let mut new_items = Vec::new();
        
        if let Some(ref item_source) = self.item_source {
            // Try to receive items without blocking
            while let Ok(item) = item_source.try_recv() {
                new_items.push(item);
            }
        }
        
        // Only restart matcher if we actually received new items (batch processing like legacy)
        if !new_items.is_empty() {
            // Add all items to the pool at once
            let _ = self.item_pool.append(new_items);
            
            // Only restart matcher once for the entire batch (like legacy system)
            self.restart_matcher();
        }
    }
    
    /// Add a single item to the item pool (legacy system approach)
    fn add_item(&mut self, item: Arc<dyn SkimItem>) {
        // Add item to the item pool (same as legacy system)
        let _ = self.item_pool.append(vec![item]);
        
        // Note: Don't restart matcher here - let process_incoming_items handle batching
        // This prevents excessive matcher restarts for every single item
    }
    
    /// Restart matcher with current query (legacy system approach)
    fn restart_matcher(&mut self) {
        let query = &self.ui_state.query_state.content;
        
        // Kill existing matcher if running (same as legacy system)
        if let Some(ctrl) = self.matcher_control.take() {
            ctrl.kill();
        }
        
        // Run the matcher on the item pool with current query (same as legacy system)
        let item_pool_clone = self.item_pool.clone();
        let matcher_control = self.matcher.run(query, item_pool_clone, |_matched_items_lock| {
            // This callback is called when matcher has results
            // We'll process results in the main loop via process_matcher_results
        });
        
        self.matcher_control = Some(matcher_control);
    }
    
    /// Process matcher results and update UI (same as legacy system heartbeat)
    fn process_matcher_results(&mut self) {
        if let Some(ref ctrl) = self.matcher_control {
            if ctrl.stopped() {
                // Matcher has finished, get the results
                let ctrl = self.matcher_control.take().unwrap();
                let matched_items_lock = ctrl.into_items();
                let mut matched_items = matched_items_lock.lock();
                let items = std::mem::take(&mut *matched_items);
                
                // Update selection state with matched items
                self.ui_state.selection_state.set_items(items.clone());
                
                // Update status 
                let total = self.item_pool.len();
                let matched_count = items.len();
                let selected_count = self.ui_state.selection_state.selected_items.len();
                let current_item = self.ui_state.selection_state.selected;
                self.ui_state.status_state.update_status(total, matched_count, 0, selected_count, current_item);
            }
        }
    }
    
    /// Set items for selection
    pub fn set_items(&mut self, items: Vec<String>) {
        // Convert strings to skim items and add to item pool
        self.item_pool.clear();
        let skim_items: Vec<Arc<dyn SkimItem>> = items
            .into_iter()
            .map(|text| Arc::new(text) as Arc<dyn SkimItem>)
            .collect();
        let _ = self.item_pool.append(skim_items);
        
        // Restart matcher with current query (this is appropriate since it's a full reset)
        self.restart_matcher();
    }
    
    /// Set preview content
    pub fn set_preview_content(&mut self, content: String) {
        self.ui_state.previewer_state.set_plain_text(content);
    }
    
    /// Get mutable reference to input translator for customization
    pub fn input_translator_mut(&mut self) -> &mut InputTranslator {
        &mut self.input_translator
    }
    
    /// Get reference to input translator
    pub fn input_translator(&self) -> &InputTranslator {
        &self.input_translator
    }
    
    /// Add a custom key binding
    pub fn bind_key(&mut self, key_str: &str, actions: Vec<SkimMessage>) {
        if let Some(key_combination) = InputTranslator::parse_key_combination(key_str) {
            self.input_translator.bind_key(key_combination, actions);
        }
    }
    
    /// Remove a key binding
    pub fn unbind_key(&mut self, key_str: &str) {
        if let Some(key_combination) = InputTranslator::parse_key_combination(key_str) {
            self.input_translator.unbind_key(&key_combination);
        }
    }
}

impl Drop for UICoordinator {
    fn drop(&mut self) {
        // Terminal cleanup is handled by SkimTerminal's Drop implementation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::SkimOptionsBuilder;

    #[test]
    fn test_coordinator_creation() {
        // Skip this test in environments without a proper terminal
        if std::env::var("CI").is_ok() || 
           std::env::var("TERM").unwrap_or_default().is_empty() {
            return; // Skip in environments without proper terminal
        }
        
        let options = SkimOptionsBuilder::default().build().unwrap();
        
        // This test would need to be run in a terminal environment
        // For now, just test that the creation logic doesn't panic
        assert!(UICoordinator::new(&options).is_ok());
    }
    
    #[test]
    fn test_global_event_handling() {
        // Skip this test in environments without a proper terminal
        if std::env::var("CI").is_ok() || 
           std::env::var("TERM").unwrap_or_default().is_empty() {
            return; // Skip in environments without proper terminal
        }
        
        let options = SkimOptionsBuilder::default().build().unwrap();
        let mut coordinator = UICoordinator::new(&options).unwrap();
        
        // Test escape key
        let esc_event = SkimEvent::Key(crossterm::event::KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        
        let result = coordinator.handle_global_events(&esc_event);
        assert!(matches!(result, Some(SkimMessage::Abort)));
        
        // Test enter key
        let enter_event = SkimEvent::Key(crossterm::event::KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        
        let result = coordinator.handle_global_events(&enter_event);
        assert!(matches!(result, Some(SkimMessage::Accept)));
    }
    
    #[test]
    fn test_status_update() {
        // Skip this test in environments without a proper terminal
        if std::env::var("CI").is_ok() || 
           std::env::var("TERM").unwrap_or_default().is_empty() {
            return; // Skip in environments without proper terminal
        }
        
        let options = SkimOptionsBuilder::default().build().unwrap();
        let mut coordinator = UICoordinator::new(&options).unwrap();
        
        coordinator.update_status(100, 50, 5, 10);
        
        let status = &coordinator.ui_state().status_state;
        assert_eq!(status.total, 100);
        assert_eq!(status.matched, 50);
        assert_eq!(status.selected, 5);
        assert_eq!(status.current_item_idx, 10);
    }
}