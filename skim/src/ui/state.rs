//! UI state management for the ratatui-based interface

use crate::item::MatchedItem;

/// Minimal UI state structure
#[derive(Default)]
pub struct MinimalUIState {
    pub query_state: QueryState,
    pub selection_state: SelectionState,
}

/// Query input state
#[derive(Default)]
pub struct QueryState {
    pub content: String,
}

/// Selection state
#[derive(Default)]
pub struct SelectionState;

impl SelectionState {
    pub fn get_selected_items(&self) -> Vec<MatchedItem> {
        // TODO: Return actual selected items
        Vec::new()
    }
}