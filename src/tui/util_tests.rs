use super::*;
use ansi_to_tui::IntoText as _;
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
    for line in &result.lines {
        for span in &line.spans {
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

#[test]
fn test_clip_line_to_chars_basic() {
    let line = Line::from("hello world");
    let clipped = clip_line_to_chars(line, 5);
    let content: String = clipped.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(content, "hello");
}

#[test]
fn test_clip_line_to_chars_exact_length() {
    let line = Line::from("hello");
    let clipped = clip_line_to_chars(line, 5);
    let content: String = clipped.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(content, "hello");
}

#[test]
fn test_clip_line_to_chars_longer_than_input() {
    let line = Line::from("hi");
    let clipped = clip_line_to_chars(line, 100);
    let content: String = clipped.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(content, "hi");
}

#[test]
fn test_clip_line_to_chars_zero() {
    let line = Line::from("hello");
    let clipped = clip_line_to_chars(line, 0);
    let content: String = clipped.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(content, "");
}

#[test]
fn test_clip_line_to_chars_preserves_styles() {
    use ratatui::style::Color;
    let red = Style::default().fg(Color::Red);
    let blue = Style::default().fg(Color::Blue);
    let line = Line::from(vec![Span::styled("abc", red), Span::styled("def", blue)]);
    // Clip at the span boundary.
    let clipped = clip_line_to_chars(line, 3);
    assert_eq!(clipped.spans.len(), 1);
    assert_eq!(clipped.spans[0].content.as_ref(), "abc");
    assert_eq!(clipped.spans[0].style.fg, Some(Color::Red));
}

#[test]
fn test_clip_line_to_chars_splits_span() {
    use ratatui::style::Color;
    let red = Style::default().fg(Color::Red);
    let blue = Style::default().fg(Color::Blue);
    let line = Line::from(vec![Span::styled("abcde", red), Span::styled("fghij", blue)]);
    // Clip inside the first span.
    let clipped = clip_line_to_chars(line, 3);
    assert_eq!(clipped.spans.len(), 1);
    assert_eq!(clipped.spans[0].content.as_ref(), "abc");
    assert_eq!(clipped.spans[0].style.fg, Some(Color::Red));
}

#[test]
fn test_clip_line_to_chars_splits_across_spans() {
    use ratatui::style::Color;
    let red = Style::default().fg(Color::Red);
    let blue = Style::default().fg(Color::Blue);
    let line = Line::from(vec![Span::styled("abc", red), Span::styled("def", blue)]);
    // Clip into the second span.
    let clipped = clip_line_to_chars(line, 5);
    assert_eq!(clipped.spans.len(), 2);
    assert_eq!(clipped.spans[0].content.as_ref(), "abc");
    assert_eq!(clipped.spans[1].content.as_ref(), "de");
    assert_eq!(clipped.spans[1].style.fg, Some(Color::Blue));
}

#[test]
fn test_clip_line_to_chars_unicode() {
    // Each kanji is one char.
    let line = Line::from("日本語テスト");
    let clipped = clip_line_to_chars(line, 3);
    let content: String = clipped.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(content, "日本語");
}

#[test]
fn test_merge_styles() {
    use ratatui::style::Color::*;
    use ratatui::style::Modifier;
    let input = "before \x1b[1;34mline1\x1b[0m nocol";
    let styled = input.into_text().unwrap().lines[0].clone();
    let red = Style::new().red();
    let underline = Style::new().underlined();
    assert_eq!(merge_styles(red, styled.spans[0].style).fg, Some(Red));
    assert_eq!(merge_styles(red, styled.spans[1].style).fg, Some(Blue));
    assert_eq!(merge_styles(red, styled.spans[1].style).add_modifier, Modifier::BOLD);
    assert_eq!(merge_styles(red, styled.spans[2].style).fg, Some(Red));
    assert_eq!(
        merge_styles(underline, styled.spans[0].style).add_modifier & Modifier::UNDERLINED,
        Modifier::UNDERLINED
    );
    assert_eq!(
        merge_styles(underline, styled.spans[1].style).add_modifier & Modifier::UNDERLINED,
        Modifier::UNDERLINED
    );
    assert_eq!(
        merge_styles(underline, styled.spans[2].style).add_modifier & Modifier::UNDERLINED,
        Modifier::UNDERLINED
    );
}

#[test]
fn test_style_text() {
    use ratatui::style::Color::*;
    use ratatui::style::Modifier;
    let input = "before \x1b[1;34mline1\x1b[0m nocol";
    let mut styled = input.into_text().unwrap();
    let red = Style::new().red();
    style_text(&mut styled, red);
    assert_eq!(styled.lines.len(), 1);
    let line = styled.lines[0].clone();
    assert_eq!(line.spans[0].style.fg, Some(Red));
    assert_eq!(line.spans[1].style.fg, Some(Blue));
    assert_eq!(line.spans[1].style.add_modifier, Modifier::BOLD);
    assert_eq!(line.spans[2].style.fg, Some(Red));
}

#[test]
fn test_char_display_width() {
    assert_eq!(char_display_width('a'), 1);
    // Variation selector forces double width.
    assert_eq!(char_display_width('\u{FE0F}'), 2);
    // Wide CJK character.
    assert_eq!(char_display_width('日'), 2);
}

#[test]
fn test_find_osc_end_bel_terminator() {
    // ESC ] ... BEL  (BEL at index 9, end is one past it)
    let data = b"\x1b]0;title\x07rest";
    assert_eq!(find_osc_end(data), Some(10));
}

#[test]
fn test_find_osc_end_st_terminator() {
    // ESC ] ... ESC \  (ESC at 9, backslash at 10, end is one past it)
    let data = b"\x1b]0;title\x1b\\rest";
    assert_eq!(find_osc_end(data), Some(11));
}

#[test]
fn test_find_osc_end_unterminated() {
    let data = b"\x1b]0;title";
    assert_eq!(find_osc_end(data), None);
}

#[test]
fn test_find_csi_end() {
    // ESC [ 6 n  -> terminator 'n' at index 3
    let data = b"\x1b[6n";
    assert_eq!(find_csi_end(data), Some(4));
    // Unterminated parameter bytes only.
    assert_eq!(find_csi_end(b"\x1b[12;34"), None);
}

/// A `Send` writer that captures everything written for assertions.
#[derive(Clone)]
struct SharedBuf(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl SharedBuf {
    fn new() -> Self {
        Self(std::sync::Arc::new(std::sync::Mutex::new(Vec::new())))
    }
    fn contents(&self) -> Vec<u8> {
        self.0.lock().unwrap().clone()
    }
}

impl std::io::Write for SharedBuf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_handle_osc_query_foreground() {
    let buf = SharedBuf::new();
    let mut writer: Box<dyn std::io::Write + Send> = Box::new(buf.clone());
    handle_osc_query(b"\x1b]10;?\x07", &mut writer);
    assert!(buf.contents().starts_with(b"\x1b]10;rgb:"));
}

#[test]
fn test_handle_osc_query_background() {
    let buf = SharedBuf::new();
    let mut writer: Box<dyn std::io::Write + Send> = Box::new(buf.clone());
    handle_osc_query(b"\x1b]11;?\x07", &mut writer);
    assert!(buf.contents().starts_with(b"\x1b]11;rgb:"));
}

#[test]
fn test_handle_osc_query_palette() {
    let buf = SharedBuf::new();
    let mut writer: Box<dyn std::io::Write + Send> = Box::new(buf.clone());
    handle_osc_query(b"\x1b]4;1;?\x07", &mut writer);
    let out = buf.contents();
    assert!(out.starts_with(b"\x1b]4;1;rgb:"));
}

#[test]
fn test_handle_osc_query_ignores_non_query() {
    let buf = SharedBuf::new();
    let mut writer: Box<dyn std::io::Write + Send> = Box::new(buf.clone());
    handle_osc_query(b"\x1b]0;just a title\x07", &mut writer);
    assert!(buf.contents().is_empty());
}

#[test]
fn test_handle_csi_query_device_attributes() {
    for (query, expected_prefix) in [
        (&b"\x1b[c"[..], &b"\x1b[?1;2c"[..]),
        (&b"\x1b[>c"[..], &b"\x1b[>0;0;0c"[..]),
        (&b"\x1b[5n"[..], &b"\x1b[0n"[..]),
        (&b"\x1b[6n"[..], &b"\x1b[1;1R"[..]),
    ] {
        let buf = SharedBuf::new();
        let mut writer: Box<dyn std::io::Write + Send> = Box::new(buf.clone());
        assert!(handle_csi_query(query, &mut writer));
        assert_eq!(buf.contents(), expected_prefix);
    }
}

#[test]
fn test_handle_csi_query_extended_cursor_report() {
    let buf = SharedBuf::new();
    let mut writer: Box<dyn std::io::Write + Send> = Box::new(buf.clone());
    assert!(handle_csi_query(b"\x1b[?6n", &mut writer));
    assert_eq!(buf.contents(), b"\x1b[?1;1;1R");
}

#[test]
fn test_handle_csi_query_non_query_returns_false() {
    let buf = SharedBuf::new();
    let mut writer: Box<dyn std::io::Write + Send> = Box::new(buf.clone());
    assert!(!handle_csi_query(b"\x1b[1;2H", &mut writer));
    assert!(buf.contents().is_empty());
}

#[test]
fn test_style_span_and_line() {
    use ratatui::style::Color;
    let red = Style::default().fg(Color::Red);
    let mut span = Span::raw("hi");
    style_span(&mut span, red);
    assert_eq!(span.style.fg, Some(Color::Red));

    let mut line = Line::from(vec![Span::raw("a"), Span::raw("b")]);
    style_line(&mut line, red);
    assert!(line.spans.iter().all(|s| s.style.fg == Some(Color::Red)));
}
