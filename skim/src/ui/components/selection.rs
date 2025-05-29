// Selection component implementation

use std::cmp::min;
use std::collections::HashSet;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

use crate::ui::{SkimEvent, SkimMessage};
use crate::item::MatchedItem;
pub use crate::orderedvec::OrderedVec;

/// Selection component state for ratatui implementation
#[derive(Debug, Clone)]
pub struct SelectionState {
    // Items and selection
    pub items: Vec<MatchedItem>,
    pub filtered_indices: Vec<usize>, // Indices of items that match current filter
    
    // Cursor and scrolling
    pub selected: usize,              // Currently highlighted item index (in filtered list)
    pub item_cursor: usize,           // First visible item index (in filtered list)
    pub line_cursor: usize,           // Line position of selection on screen
    
    // Multi-selection
    pub multi_selection: bool,
    pub selected_items: HashSet<usize>, // Set of selected item indices (in original list)
    
    // Display options
    pub reverse: bool,
    pub height: usize,
    pub tabstop: usize,
    pub hscroll_offset: i64,
    pub no_hscroll: bool,
    pub keep_right: bool,
    
    // Compatibility fields for basic UI
    pub scroll_offset: usize,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            filtered_indices: Vec::new(),
            selected: 0,
            item_cursor: 0,
            line_cursor: 0,
            multi_selection: false,
            selected_items: HashSet::new(),
            reverse: false,
            height: 10,
            tabstop: 8,
            hscroll_offset: 0,
            no_hscroll: false,
            keep_right: false,
            scroll_offset: 0,
        }
    }
}

impl SelectionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_multi_selection(mut self, multi: bool) -> Self {
        self.multi_selection = multi;
        self
    }

    pub fn with_reverse(mut self, reverse: bool) -> Self {
        self.reverse = reverse;
        self
    }

    pub fn with_height(mut self, height: usize) -> Self {
        self.height = height;
        self
    }

    /// Set items and reset selection state
    pub fn set_items(&mut self, items: Vec<MatchedItem>) {
        self.items = items;
        self.filtered_indices = (0..self.items.len()).collect();
        self.selected = 0;
        self.item_cursor = 0;
        self.line_cursor = 0;
        self.scroll_offset = 0;
        // Keep existing selections that still exist
        self.selected_items.retain(|&idx| idx < self.items.len());
    }

    /// Append new items
    pub fn append_items(&mut self, mut new_items: Vec<MatchedItem>) {
        let start_idx = self.items.len();
        self.items.append(&mut new_items);
        
        // Add new indices to filtered list
        for i in start_idx..self.items.len() {
            self.filtered_indices.push(i);
        }
    }

    /// Apply filter to items (e.g., based on query)
    pub fn apply_filter(&mut self, filter_indices: Vec<usize>) {
        self.filtered_indices = filter_indices;
        // Adjust selection if current selection is no longer visible
        if self.selected >= self.filtered_indices.len() {
            self.selected = self.filtered_indices.len().saturating_sub(1);
        }
        self.update_cursors();
    }

    /// Get the currently selected item
    pub fn get_current_item(&self) -> Option<&MatchedItem> {
        self.filtered_indices
            .get(self.selected)
            .and_then(|&idx| self.items.get(idx))
    }

    /// Get all selected items (for multi-selection)
    pub fn get_selected_items(&self) -> Vec<&MatchedItem> {
        self.selected_items
            .iter()
            .filter_map(|&idx| self.items.get(idx))
            .collect()
    }

    /// Get number of visible items
    pub fn visible_items_count(&self) -> usize {
        self.filtered_indices.len()
    }

    /// Get the actual item index from selection index
    pub fn get_item_index(&self, selection_index: usize) -> Option<usize> {
        self.filtered_indices.get(selection_index).copied()
    }

    /// Toggle selection of current item (for multi-selection)
    pub fn toggle_current_selection(&mut self) {
        if !self.multi_selection {
            return;
        }
        
        if let Some(&item_idx) = self.filtered_indices.get(self.selected) {
            if self.selected_items.contains(&item_idx) {
                self.selected_items.remove(&item_idx);
            } else {
                self.selected_items.insert(item_idx);
            }
        }
    }

    /// Select all visible items
    pub fn select_all(&mut self) {
        if self.multi_selection {
            for &idx in &self.filtered_indices {
                self.selected_items.insert(idx);
            }
        }
    }

    /// Deselect all items
    pub fn deselect_all(&mut self) {
        self.selected_items.clear();
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.update_cursors();
        }
    }

    /// Move selection down  
    pub fn move_down(&mut self) {
        if self.selected < self.filtered_indices.len().saturating_sub(1) {
            self.selected += 1;
            self.update_cursors();
        }
    }

    /// Move to first item
    pub fn move_to_first(&mut self) {
        self.selected = 0;
        self.update_cursors();
    }

    /// Move to last item
    pub fn move_to_last(&mut self) {
        self.selected = self.filtered_indices.len().saturating_sub(1);
        self.update_cursors();
    }

    /// Page up
    pub fn page_up(&mut self) {
        let page_size = self.height.saturating_sub(1).max(1);
        self.selected = self.selected.saturating_sub(page_size);
        self.update_cursors();
    }

    /// Page down
    pub fn page_down(&mut self) {
        let page_size = self.height.saturating_sub(1).max(1);
        self.selected = min(
            self.selected + page_size,
            self.filtered_indices.len().saturating_sub(1)
        );
        self.update_cursors();
    }

    /// Move selection to next item (alias for move_down)
    pub fn select_next(&mut self) {
        self.move_down();
    }
    
    /// Move selection to previous item (alias for move_up)
    pub fn select_prev(&mut self) {
        self.move_up();
    }
    
    /// Toggle multi-selection mode
    pub fn toggle_multi_selection(&mut self) {
        self.multi_selection = !self.multi_selection;
    }
    
    /// Set multi-selection mode
    pub fn set_multi_selection(&mut self, enabled: bool) {
        self.multi_selection = enabled;
    }

    /// Update cursor positions for scrolling
    fn update_cursors(&mut self) {
        // Update item_cursor (first visible item) and line_cursor (position on screen)
        if self.selected < self.item_cursor {
            // Scroll up
            self.item_cursor = self.selected;
            self.line_cursor = 0;
        } else if self.selected >= self.item_cursor + self.height {
            // Scroll down
            self.item_cursor = self.selected.saturating_sub(self.height.saturating_sub(1));
            self.line_cursor = self.height.saturating_sub(1);
        } else {
            // No scrolling needed, just update line position
            self.line_cursor = self.selected - self.item_cursor;
        }
        
        // Update compatibility field
        self.scroll_offset = self.item_cursor;
    }

    /// Check if an item is selected (for multi-selection)
    pub fn is_item_selected(&self, item_index: usize) -> bool {
        self.selected_items.contains(&item_index)
    }

    /// Get items visible in current view
    pub fn get_visible_items(&self) -> Vec<(usize, &MatchedItem, bool)> {
        let mut visible = Vec::new();
        let start = self.item_cursor;
        let end = min(start + self.height, self.filtered_indices.len());
        
        for i in start..end {
            if let Some(&item_idx) = self.filtered_indices.get(i) {
                if let Some(item) = self.items.get(item_idx) {
                    let is_selected = self.is_item_selected(item_idx);
                    visible.push((i, item, is_selected));
                }
            }
        }
        
        visible
    }
}

/// Handle selection-specific events
pub fn handle_selection_event(state: &mut SelectionState, event: &SkimEvent) -> Option<SkimMessage> {
    match event {
        SkimEvent::Key(key) => {
            use crossterm::event::{KeyCode, KeyModifiers};
            
            match (key.code, key.modifiers) {
                // Navigation keys are handled by InputTranslator to avoid double processing
                (KeyCode::Home, KeyModifiers::NONE) | (KeyCode::Char('g'), KeyModifiers::NONE) => {
                    state.move_to_first();
                    None
                }
                (KeyCode::End, KeyModifiers::NONE) | (KeyCode::Char('G'), KeyModifiers::SHIFT) => {
                    state.move_to_last();
                    None
                }
                // Page navigation and Tab are handled by InputTranslator to avoid double processing
                
                // Select all / deselect all
                (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
                    if state.multi_selection {
                        state.select_all();
                    }
                    None
                }
                (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                    if state.multi_selection {
                        state.deselect_all();
                    }
                    None
                }
                
                // Accept current selection
                (KeyCode::Enter, KeyModifiers::NONE) => {
                    Some(SkimMessage::Accept)
                }
                
                // Navigation with vi-like keys
                (KeyCode::Char('j'), KeyModifiers::NONE) => {
                    Some(SkimMessage::SelectNext)
                }
                (KeyCode::Char('k'), KeyModifiers::NONE) => {
                    Some(SkimMessage::SelectPrev)
                }
                
                _ => None,
            }
        }
        
        // Handle mouse events for click selection
        SkimEvent::Mouse(mouse) => {
            use crossterm::event::{MouseEventKind, MouseButton};
            
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // Calculate which item was clicked based on mouse row
                    let clicked_line = mouse.row as usize;
                    if clicked_line < state.height {
                        let new_selection = state.item_cursor + clicked_line;
                        if new_selection < state.filtered_indices.len() {
                            state.selected = new_selection;
                            state.update_cursors();
                        }
                    }
                    None
                }
                MouseEventKind::ScrollDown => {
                    state.move_down();
                    None
                }
                MouseEventKind::ScrollUp => {
                    state.move_up();
                    None
                }
                _ => None,
            }
        }
        
        _ => None,
    }
}

/// Render the selection component with ratatui
pub fn render_selection(state: &SelectionState, frame: &mut Frame, area: Rect) {
    // Update state height to match available area
    let mut state = state.clone();
    state.height = area.height as usize;
    
    let visible_items = state.get_visible_items();
    
    let list_items: Vec<ListItem> = visible_items
        .iter()
        .enumerate()
        .rev()  // Reverse to show items from bottom to top like legacy skim
        .map(|(screen_idx, (list_idx, item, is_multi_selected))| {
            let is_current = *list_idx == state.selected;
            
            // Determine styling
            let mut style = Style::default();
            let mut prefix = if state.multi_selection && *is_multi_selected {
                "▶ "
            } else {
                "  "
            };
            
            if is_current {
                style = style.bg(Color::Blue).fg(Color::White);
                if !state.multi_selection || !*is_multi_selected {
                    prefix = "► ";
                }
            }
            
            // Get item text - for now use display text, later we'll add match highlighting
            let item_text = item.item.output();
            let full_text = format!("{}{}", prefix, item_text);
            
            ListItem::new(Line::from(full_text)).style(style)
        })
        .collect();
    
    let list = List::new(list_items)
        .block(Block::default().borders(Borders::NONE))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    
    frame.render_widget(list, area);
}

/// Render selection with match highlighting (more advanced version)
pub fn render_selection_with_highlighting(
    state: &SelectionState, 
    frame: &mut Frame, 
    area: Rect,
    query: &str
) {
    // Update state height to match available area
    let mut state = state.clone();
    state.height = area.height as usize;
    
    let visible_items = state.get_visible_items();
    
    let list_items: Vec<ListItem> = visible_items
        .iter()
        .enumerate()
        .rev()  // Reverse to show items from bottom to top like legacy skim
        .map(|(screen_idx, (list_idx, item, is_multi_selected))| {
            let is_current = *list_idx == state.selected;
            
            // Create spans for highlighting
            let mut spans = Vec::new();
            
            // Add prefix
            let prefix = if state.multi_selection && *is_multi_selected {
                "▶ "
            } else {
                "  "
            };
            
            let prefix_style = if is_current {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };
            
            spans.push(Span::styled(prefix, prefix_style));
            
            // Add item text with match highlighting
            let item_text = item.item.output();
            
            // For now, just add the full text - later we'll implement proper match highlighting
            let text_style = if is_current {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default()
            };
            
            spans.push(Span::styled(item_text, text_style));
            
            ListItem::new(Line::from(spans))
        })
        .collect();
    
    let list = List::new(list_items)
        .block(Block::default().borders(Borders::NONE));
    
    frame.render_widget(list, area);
}