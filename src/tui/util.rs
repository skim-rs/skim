use ratatui::text::{Line, Span, Text};
use unicode_display_width::is_double_width;

// Directly taken from https://docs.rs/unicode-display-width/0.3.0/src/unicode_display_width/lib.rs.html#77-81
#[inline]
pub fn char_display_width(c: char) -> usize {
    if c == '\u{FE0F}' || is_double_width(c) {
        return 2;
    }
    1
}

pub fn wrap_text(input: Text, width: usize) -> Text {
    if input.width() <= width {
        return input;
    }

    let mut output = Text::default();

    for input_line in input.iter() {
        let mut current_line = Line::default();
        let mut w = 0;
        for span in input_line.spans.iter() {
            let mut curr = Span::default().style(span.style);
            let mut curr_content = String::new();
            for c in span.content.chars() {
                if w + char_display_width(c) > width {
                    // Push current span and line before wrapping
                    if !curr_content.is_empty() {
                        curr.content = curr_content.into();
                        current_line.push_span(curr);
                    }
                    output.push_line(current_line);
                    // Reset for new line
                    current_line = Line::default();
                    curr = Span::default().style(span.style);
                    curr_content = String::new();
                    w = 0;
                }
                curr_content.push(c);
                w += char_display_width(c);
            }
            // Push remaining content in current span
            if !curr_content.is_empty() {
                curr.content = curr_content.into();
                current_line.push_span(curr);
            }
        }
        // Push remaining line
        if !current_line.spans.is_empty() {
            output.push_line(current_line);
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Style};

    #[test]
    fn test_wrap_text_no_wrap_needed() {
        // Text shorter than width should not be wrapped
        let input = Text::from("short");
        let result = wrap_text(input.clone(), 10);
        assert_eq!(result.lines.len(), 1);
        assert_eq!(result.lines[0].spans[0].content, "short");
    }

    #[test]
    fn test_wrap_text_exact_width() {
        // Text exactly at width should not be wrapped
        let input = Text::from("exact");
        let result = wrap_text(input, 5);
        assert_eq!(result.lines.len(), 1);
        assert_eq!(result.lines[0].spans[0].content, "exact");
    }

    #[test]
    fn test_wrap_text_simple_wrap() {
        // Text longer than width should wrap
        let input = Text::from("hello world");
        let result = wrap_text(input, 5);
        assert!(result.lines.len() > 1);
        assert_eq!(result.lines[0].spans[0].content, "hello");
        assert_eq!(result.lines[1].spans[0].content, " worl");
        assert_eq!(result.lines[2].spans[0].content, "d");
    }

    #[test]
    fn test_wrap_text_preserves_style() {
        // Create styled text
        let style = Style::default().fg(Color::Red);
        let span = Span::styled("hello world", style);
        let input = Text::from(Line::from(vec![span]));

        let result = wrap_text(input, 5);

        // Verify style is preserved across all spans
        for line in result.lines.iter() {
            for span in line.spans.iter() {
                assert_eq!(span.style.fg, Some(Color::Red));
            }
        }
    }

    #[test]
    fn test_wrap_text_multiple_spans() {
        // Create text with multiple spans
        let span1 = Span::styled("hello", Style::default().fg(Color::Red));
        let span2 = Span::styled(" world", Style::default().fg(Color::Blue));
        let input = Text::from(Line::from(vec![span1, span2]));

        let result = wrap_text(input, 5);

        // Should wrap into multiple lines
        assert!(result.lines.len() > 1);

        // Verify content is preserved
        let reconstructed: String = result
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(reconstructed, "hello world");
    }

    #[test]
    fn test_wrap_text_multiple_lines() {
        // Create text with multiple input lines
        let input = Text::from(vec![Line::from("first line"), Line::from("second line")]);

        let result = wrap_text(input, 5);

        // Each input line should be processed
        assert!(result.lines.len() >= 2);
    }

    #[test]
    fn test_wrap_text_unicode_characters() {
        // Test with wide Unicode characters
        let input = Text::from("こんにちは"); // Japanese characters (2 width each)
        let result = wrap_text(input, 6);

        // Should wrap correctly based on display width
        assert!(result.lines.len() > 1);
    }

    #[test]
    fn test_wrap_text_zero_width_characters() {
        // Test with combining characters
        let input = Text::from("a\u{0301}b"); // a with accent
        let result = wrap_text(input, 10);

        // Should handle zero-width characters
        assert_eq!(result.lines.len(), 1);
    }

    #[test]
    fn test_wrap_text_width_one() {
        // Edge case: wrap at width 1
        let input = Text::from("abc");
        let result = wrap_text(input, 1);

        // Each character should be on its own line
        assert_eq!(result.lines.len(), 3);
        assert_eq!(result.lines[0].spans[0].content, "a");
        assert_eq!(result.lines[1].spans[0].content, "b");
        assert_eq!(result.lines[2].spans[0].content, "c");
    }

    #[test]
    fn test_wrap_text_empty_input() {
        // Test with empty text
        let input = Text::default();
        let result = wrap_text(input, 10);

        // Should return empty text
        assert_eq!(result.lines.len(), 0);
    }

    #[test]
    fn test_wrap_text_preserves_multiple_styles() {
        // Create complex multi-styled text
        let red_style = Style::default().fg(Color::Red);
        let blue_style = Style::default().fg(Color::Blue);
        let green_style = Style::default().fg(Color::Green);

        let span1 = Span::styled("hello", red_style);
        let span2 = Span::styled("world", blue_style);
        let span3 = Span::styled("test", green_style);

        let input = Text::from(Line::from(vec![span1, span2, span3]));
        let result = wrap_text(input, 5);

        // Collect all styles from result
        let styles: Vec<_> = result
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.style.fg)
            .collect();

        // Should contain all original colors
        assert!(styles.contains(&Some(Color::Red)));
        assert!(styles.contains(&Some(Color::Blue)));
        assert!(styles.contains(&Some(Color::Green)));
    }
}
