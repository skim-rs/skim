//! Layout management for skim UI
//!
//! This module handles the spatial arrangement of UI components in skim.
//! It supports multiple layout modes, preview panels, and partial screen mode.
//!
//! ## Layout Modes
//! - **default**: Header -> Selection -> Status -> Query  
//! - **reverse**: Query -> Status -> Selection -> Header
//! - **reverse-list**: Header -> Query -> Status -> Selection

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
};

use crate::ui::LayoutConfig;

/// Calculate layout areas based on configuration
pub fn calculate_layout(area: Rect, config: &LayoutConfig) -> LayoutAreas {
    // Handle partial screen mode first
    let work_area = if let Some(height_percent) = config.partial_screen {
        let height = (area.height * height_percent / 100).max(5);
        Rect {
            x: area.x,
            y: area.height.saturating_sub(height),
            width: area.width,
            height,
        }
    } else {
        area
    };
    
    // Split for preview if enabled
    let (main_area, preview_area) = match config.preview_direction {
        Some(Direction::Horizontal) => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(100 - config.preview_size),
                    Constraint::Percentage(config.preview_size),
                ])
                .split(work_area);
            (chunks[0], Some(chunks[1]))
        }
        Some(Direction::Vertical) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(100 - config.preview_size),
                    Constraint::Percentage(config.preview_size),
                ])
                .split(work_area);
            (chunks[0], Some(chunks[1]))
        }
        None => (work_area, None),
    };
    
    // Create main UI layout
    let main_chunks = match config.layout_mode.as_str() {
        "reverse" => {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // query
                    Constraint::Length(1), // status
                    Constraint::Min(1),    // selection
                    Constraint::Length(1), // header (optional)
                ])
                .split(main_area)
        }
        "reverse-list" => {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // header (optional)
                    Constraint::Length(1), // query
                    Constraint::Length(1), // status
                    Constraint::Min(1),    // selection
                ])
                .split(main_area)
        }
        _ => {
            // default layout
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // header (optional)
                    Constraint::Min(1),    // selection
                    Constraint::Length(1), // status
                    Constraint::Length(1), // query
                ])
                .split(main_area)
        }
    };
    
    // Map chunks to layout areas based on layout mode
    match config.layout_mode.as_str() {
        "reverse" => LayoutAreas {
            query: main_chunks[0],
            status: main_chunks[1],
            selection: main_chunks[2],
            header: if main_chunks.len() > 3 { Some(main_chunks[3]) } else { None },
            preview: preview_area,
        },
        "reverse-list" => LayoutAreas {
            header: if main_chunks.len() > 3 { Some(main_chunks[0]) } else { None },
            query: main_chunks[1],
            status: main_chunks[2],
            selection: main_chunks[3],
            preview: preview_area,
        },
        _ => LayoutAreas {
            header: Some(main_chunks[0]),
            selection: main_chunks[1],
            status: main_chunks[2],
            query: main_chunks[3],
            preview: preview_area,
        },
    }
}

/// Layout areas for all UI components
#[derive(Debug, Clone)]
pub struct LayoutAreas {
    pub header: Option<Rect>,
    pub selection: Rect,
    pub status: Rect,
    pub query: Rect,
    pub preview: Option<Rect>,
}

/// Calculate preview position based on direction hint
pub fn calculate_preview_direction(hint: &str) -> Option<Direction> {
    match hint {
        "right" | "left" => Some(Direction::Horizontal),
        "up" | "down" => Some(Direction::Vertical),
        _ => None,
    }
}

/// Calculate optimal preview size based on terminal size
pub fn calculate_preview_size(terminal_size: (u16, u16), direction: Direction) -> u16 {
    match direction {
        Direction::Horizontal => {
            // For horizontal split, use 50% of width by default
            // Adjust based on terminal width for better usability
            if terminal_size.0 > 120 {
                40 // Use 40% for wide terminals
            } else {
                50 // Use 50% for normal terminals
            }
        }
        Direction::Vertical => {
            // For vertical split, use 40% of height by default
            // Adjust based on terminal height
            if terminal_size.1 > 30 {
                35 // Use 35% for tall terminals
            } else {
                40 // Use 40% for normal terminals
            }
        }
    }
}

/// Validate layout configuration and provide defaults
pub fn validate_layout_config(config: &mut LayoutConfig) {
    // Ensure preview size is reasonable
    config.preview_size = config.preview_size.clamp(10, 80);
    
    // Validate partial screen percentage
    if let Some(ref mut partial) = config.partial_screen {
        *partial = (*partial).clamp(10u16, 100u16);
    }
    
    // Normalize layout mode string
    match config.layout_mode.as_str() {
        "reverse" | "reverse-list" => {}, // Valid modes
        _ => config.layout_mode = "default".to_string(),
    }
}

/// Calculate minimum required terminal size for the given configuration
pub fn calculate_minimum_size(config: &LayoutConfig) -> (u16, u16) {
    let min_width = if config.preview_direction.is_some() {
        40 // Need space for both main view and preview
    } else {
        20 // Minimum usable width
    };
    
    let min_height = match config.layout_mode.as_str() {
        "reverse" | "reverse-list" => 4, // Query + Status + Selection + Header
        _ => 4, // Same minimum for all layouts
    };
    
    (min_width, min_height)
}