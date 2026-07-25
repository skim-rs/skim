use std::sync::Arc;

use crossterm::event::{KeyEvent, MouseEvent};

pub use super::actions::{Action, ActionCallback, parse_action};

/// Events that can occur during skim's execution
#[derive(Clone, Debug)]
pub enum Event {
    /// Quit the application
    Quit,
    /// An error occurred
    Error(String),
    /// Close the application
    Close,
    /// Timer tick event
    Tick,
    /// Render the UI
    Render,
    /// A key was pressed
    Key(KeyEvent),
    /// Text was pasted (bracketed paste)
    Paste(String),
    /// A mouse event occurred
    Mouse(MouseEvent),
    /// Preview content is ready to display
    PreviewReady,
    /// Invalid input received
    InvalidInput,
    /// An action was triggered
    Action(Action),
    /// Append items to the pool
    AppendItems(Vec<Arc<dyn crate::SkimItem>>),
    /// Clear all items
    ClearItems,
    /// Clear the screen
    Clear,
    /// Heartbeat event
    Heartbeat,
    /// Run the preview command
    RunPreview,
    /// Run a command in the foreground, handing it the terminal
    ///
    /// Carries the already-expanded command line. Handled by the TUI event
    /// loop (which has access to the [`Tui`](crate::tui::Tui)) rather than by
    /// `handle_action`, because running a foreground process requires
    /// suspending skim's own input reader and toggling terminal modes.
    RunExecute(String),
    /// Redraw the screen
    Redraw,
    /// Reload with a new command
    Reload(String),
    /// Terminal was resized to (columns, rows)
    Resize(u16, u16),
}
