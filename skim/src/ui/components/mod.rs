// Core ratatui imports are handled in individual component modules

// Component states are defined in their respective modules

pub mod query;
pub mod selection;
pub mod header;
pub mod status;
pub mod previewer;

pub use query::{QueryState, QueryMode, handle_query_event, render_query};
pub use selection::{SelectionState, handle_selection_event, render_selection, render_selection_with_highlighting};
pub use crate::item::MatchedItem;
pub use header::{HeaderState, handle_header_event, render_header};
pub use status::{StatusState, InfoDisplay, handle_status_event, render_status};
pub use previewer::{PreviewerState, handle_previewer_event, render_preview};

// Component rendering is now handled in individual component modules