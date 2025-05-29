//! Ratatui-based UI module for skim
//!
//! This module provides a modern ratatui-based user interface as an alternative
//! to the legacy skim-tuikit interface. It's designed to be feature-complete
//! while providing better performance and maintainability.

use std::io;
use crate::{SkimOptions, SkimItemReceiver};

// Re-export for compatibility
pub use self::state::*;

mod coordinator;
mod state;

// Minimal compatibility types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyKey {
    Null,
    // Add more as needed
}

/// Minimal UI coordinator for progressive migration
pub struct MinimalUICoordinator {
    _options: String, // Store minimal option data for now
    ui_state: MinimalUIState,
}

impl MinimalUICoordinator {
    pub fn new(_options: &SkimOptions) -> io::Result<Self> {
        Ok(Self {
            _options: "placeholder".to_string(),
            ui_state: MinimalUIState::default(),
        })
    }
    
    pub fn set_item_source(&mut self, _source: SkimItemReceiver) {
        // TODO: Store source for processing
    }
    
    pub fn run(&mut self) -> io::Result<()> {
        // Minimal implementation - just exit immediately for now
        eprintln!("Ratatui UI system is not yet implemented");
        eprintln!("Use legacy system by unsetting SKIM_USE_RATATUI environment variable");
        Ok(())
    }
    
    pub fn ui_state(&self) -> &MinimalUIState {
        &self.ui_state
    }
}

// Use the actual coordinator implementation
pub use coordinator::UICoordinator;