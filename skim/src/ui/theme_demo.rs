//! Theme system demonstration
//! 
//! This shows how to use the new ratatui theme system

use crate::ui::RatatuiTheme;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Demo function showing theme usage
pub fn demo_theme_usage() {
    // Create different themes
    let dark_theme = RatatuiTheme::dark256();
    let bw_theme = RatatuiTheme::bw();
    let custom_theme = RatatuiTheme::from_options("fg:red,bg:blue,matched:green");
    
    // Use theme styles in text rendering
    let _demo_spans = vec![
        Span::styled("Normal text", dark_theme.normal_style()),
        Span::styled("Matched text", dark_theme.matched_style()),
        Span::styled("Current item", dark_theme.current_style()),
        Span::styled("Query text", dark_theme.query_style()),
    ];
    
    // Demonstrate color parsing
    let _parsed_theme = RatatuiTheme::from_options(
        "fg:#ffffff,bg:#000000,matched:#00ff00,current:#ffff00,query:#00ffff"
    );
    
    println!("Theme system demo - themes created successfully!");
}

/// Helper function to create a themed style
pub fn create_themed_style(theme: &RatatuiTheme, style_type: &str) -> Style {
    match style_type {
        "normal" => theme.normal_style(),
        "matched" => theme.matched_style(),
        "current" => theme.current_style(),
        "current_match" => theme.current_match_style(),
        "query" => theme.query_style(),
        "spinner" => theme.spinner_style(),
        "info" => theme.info_style(),
        "prompt" => theme.prompt_style(),
        "cursor" => theme.cursor_style(),
        "selected" => theme.selected_style(),
        "header" => theme.header_style(),
        "border" => theme.border_style(),
        _ => Style::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_demo() {
        // Just verify the demo function doesn't panic
        demo_theme_usage();
    }
    
    #[test]
    fn test_themed_style_creation() {
        let theme = RatatuiTheme::dark256();
        
        let normal_style = create_themed_style(&theme, "normal");
        assert_eq!(normal_style.fg, Some(Color::Reset));
        
        let matched_style = create_themed_style(&theme, "matched");
        assert_eq!(matched_style.fg, Some(Color::Indexed(108)));
        
        let current_style = create_themed_style(&theme, "current");
        assert_eq!(current_style.fg, Some(Color::Indexed(254)));
        assert_eq!(current_style.bg, Some(Color::Indexed(236)));
    }
}