
use ratatui::Frame;

pub mod components;
pub mod coordinator;
pub mod demo;
pub mod events;
pub mod input_translator;
pub mod layout;
pub mod legacy_bridge;
pub mod terminal_wrapper;
pub mod theme;
pub(crate) mod tuikit_compat;

pub use events::*;
pub use input_translator::InputTranslator;
pub use legacy_bridge::{LegacyKey, LegacyTermEvent, LegacyAttr, LegacyEffect, from_keyname};
// Re-export necessary compatibility items for internal use
pub use tuikit_compat::{Size, TermHeight};
pub use terminal_wrapper::*;
pub use theme::RatatuiTheme;

/// Main UI state containing all component states
#[derive(Debug)]
pub struct SkimUI {
    pub query_state: QueryState,
    pub selection_state: SelectionState,
    pub header_state: HeaderState,
    pub status_state: StatusState,
    pub previewer_state: PreviewerState,
    pub layout_config: LayoutConfig,
    pub theme: RatatuiTheme,
}

// Re-export component states and handlers
pub use components::{
    QueryState, QueryMode, handle_query_event, render_query,
    SelectionState, MatchedItem, handle_selection_event, render_selection, render_selection_with_highlighting,
    HeaderState, handle_header_event, render_header,
    StatusState, InfoDisplay, handle_status_event, render_status,
    PreviewerState, handle_previewer_event, render_preview,
};
pub use coordinator::UICoordinator;

// Component states are now defined in their respective modules

/// Layout configuration
#[derive(Debug)]
pub struct LayoutConfig {
    pub layout_mode: String,
    pub preview_direction: Option<Direction>,
    pub preview_size: u16,
    pub partial_screen: Option<u16>, // percentage
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            layout_mode: "default".to_string(),
            preview_direction: None,
            preview_size: 50,
            partial_screen: None,
        }
    }
}

impl Default for SkimUI {
    fn default() -> Self {
        Self {
            query_state: QueryState::default(),
            selection_state: SelectionState::default(),
            header_state: HeaderState::default(),
            status_state: StatusState::default(),
            previewer_state: PreviewerState::default(),
            layout_config: LayoutConfig::default(),
            theme: RatatuiTheme::default(),
        }
    }
}

/// Main view function that renders the entire UI
pub fn view(ui_state: &SkimUI, frame: &mut Frame) {
    let main_layout = layout::calculate_layout(frame.size(), &ui_state.layout_config);
    
    // Render header
    if let Some(header_area) = main_layout.header {
        render_header(&ui_state.header_state, frame, header_area);
    }
    
    // Render selection
    components::render_selection(&ui_state.selection_state, frame, main_layout.selection);
    
    // Render status
    render_status(&ui_state.status_state, frame, main_layout.status);
    
    // Render query
    components::render_query(&ui_state.query_state, frame, main_layout.query);
    
    // Render preview if visible
    if ui_state.previewer_state.visible {
        if let Some(preview_area) = main_layout.preview {
            render_preview(&ui_state.previewer_state, frame, preview_area);
        }
    }
}

