//! Ratatui-based UI module for skim
//!
//! This module provides a modern ratatui-based user interface as an alternative
//! to the legacy skim-tuikit interface. It's designed to be feature-complete
//! while providing better performance and maintainability.

mod coordinator;
mod state;

// Re-export the state types for compatibility
pub use self::state::*;

// Export the main UI coordinator
pub use coordinator::UICoordinator;