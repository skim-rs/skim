//! Skim item helpers
//! Including the DefaultSkimItem
use crate::field::{FieldRange, parse_matching_fields, parse_transform_fields};
use crate::tui::util::merge_styles;
use crate::{DisplayContext, SkimItem};
use ansi_to_tui::IntoText;
use ratatui::text::{Line, Span};
use regex::Regex;
use std::borrow::Cow;

//------------------------------------------------------------------------------
/// An item will store everything that one line input will need to be operated and displayed.
///
/// What's special about an item?
/// The simplest version of an item is a line of string, but things are getting more complex:
/// - The conversion of lower/upper case is slow in rust, because it involds unicode.
/// - We may need to interpret the ANSI codes in the text.
/// - The text can be transformed and limited while searching.
///
/// About the ANSI, we made assumption that it is linewise, that means no ANSI codes will affect
/// more than one line.
#[derive(Debug)]
pub struct DefaultSkimItem {
    /// The text that will be shown on screen.
    text: Box<str>,

    /// The index, for use in matching
    index: usize,

    /// Metadata containing miscellaneous fields when special options are used
    metadata: Option<Box<DefaultSkimItemMetadata>>,
}

/// Additional metadata for a SkimItem
#[derive(Debug)]
pub struct DefaultSkimItemMetadata {
    /// The text that will be output when user press `enter`
    /// `Some(..)` => the original input is transformed, could not output `text` directly
    /// `None` => that it is safe to output `text` directly
    orig_text: Option<Box<str>>,

    /// The text stripped of all ansi sequences, used for matching
    /// Will be Some when ANSI is enabled, None otherwise
    stripped_text: Option<Box<str>>,

    /// A mapping of positions from stripped text to original text.
    /// Each element is (byte_position, char_position) in the original raw text.
    /// Will be empty if ansi is disabled.
    ansi_info: Option<Vec<(usize, usize)>>,

    // The ranges on which to perform matching
    matching_ranges: Option<Vec<(usize, usize)>>,
}

impl DefaultSkimItem {
    /// Create a new DefaultSkimItem from text
    pub fn new(
        orig_text: &str,
        ansi_enabled: bool,
        trans_fields: &[FieldRange],
        matching_fields: &[FieldRange],
        delimiter: &Regex,
        index: usize,
    ) -> Self {
        let using_transform_fields = !trans_fields.is_empty();

        //        transformed | ANSI             | output
        //------------------------------------------------------
        //                    +- T -> trans+ANSI | ANSI
        //                    |                  |
        //      +- T -> trans +- F -> trans      | orig
        // orig |                                |
        //      +- F -> orig  +- T -> ANSI     ==| ANSI
        //                    |                  |
        //                    +- F -> orig       | orig

        let (mut orig_text, mut temp_text): (Option<String>, Cow<str>) = match (using_transform_fields, ansi_enabled) {
            (true, true) => {
                let transformed = parse_transform_fields(delimiter, &orig_text, trans_fields);
                (Some(orig_text.into()), Cow::Owned(transformed))
            }
            (true, false) => {
                let transformed = parse_transform_fields(delimiter, &escape_ansi(&orig_text), trans_fields);
                (Some(orig_text.into()), Cow::Owned(transformed))
            }
            (false, true) => (None, Cow::Borrowed(orig_text)),
            (false, false) => (None, Cow::Owned(escape_ansi(&orig_text).into())),
        };

        // Keep track of whether we have null bytes for special handling
        let has_null_bytes = temp_text.contains('\0');

        // Preserve original text with null bytes for output if needed
        if has_null_bytes && orig_text.is_none() {
            orig_text = Some(temp_text.to_string());
        }

        // Strip null bytes from text used for display and matching
        // Null bytes are control characters that cause rendering issues (zero-width)
        // They are preserved in orig_text for output
        if has_null_bytes {
            temp_text = Cow::Owned(temp_text.to_string().replace('\0', ""));
        }

        let (stripped_text, ansi_info) = if ansi_enabled {
            let (stripped, info) = strip_ansi(&temp_text);
            (Some(stripped), Some(info))
        } else {
            (None, None)
        };

        // Calculate matching ranges on text WITHOUT null bytes (after stripping)
        // This ensures the byte positions match the actual text used for matching
        let matching_ranges = if !matching_fields.is_empty() {
            // Use stripped text for matching ranges when ANSI is enabled
            let text_for_matching = if ansi_enabled {
                stripped_text.as_ref().unwrap()
            } else {
                temp_text.as_ref()
            };

            // Parse the original text with null bytes to determine field boundaries
            // Then extract those fields, strip null bytes, and recalculate positions
            let orig_text_for_fields = if has_null_bytes {
                orig_text.as_deref().unwrap()
            } else {
                &text_for_matching
            };

            if has_null_bytes {
                // Extract each field from the original text (with null bytes)
                // then strip null bytes and build new ranges in the cleaned text
                let mut adjusted_ranges = Vec::new();

                for field in matching_fields {
                    // Get the field text from original (with null bytes)
                    if let Some(field_text) = crate::field::get_string_by_field(delimiter, &orig_text_for_fields, field)
                    {
                        // Strip null bytes from this field
                        let cleaned_field = field_text.replace('\0', "");

                        // Find this cleaned field in the cleaned full text
                        if let Some(pos) = text_for_matching.find(&cleaned_field) {
                            adjusted_ranges.push((pos, pos + cleaned_field.len()));
                        }
                    }
                }
                Some(adjusted_ranges)
            } else {
                Some(parse_matching_fields(delimiter, text_for_matching, matching_fields))
            }
        } else {
            None
        };

        let metadata =
            if orig_text.is_some() || stripped_text.is_some() || ansi_info.is_some() || matching_ranges.is_some() {
                Some(Box::new(DefaultSkimItemMetadata {
                    orig_text: orig_text.map(|inner| inner.into_boxed_str()),
                    stripped_text: stripped_text.map(|inner| inner.into_boxed_str()),
                    ansi_info,
                    matching_ranges,
                }))
            } else {
                None
            };

        DefaultSkimItem {
            text: Box::from(temp_text),
            index,
            metadata,
        }
    }
    /// Getter for stripped_text stored in the metadata
    pub fn stripped_text(&self) -> Option<&Box<str>> {
        if let Some(meta) = &self.metadata
            && let Some(stripped_text) = &meta.stripped_text
        {
            Some(stripped_text)
        } else {
            None
        }
    }

    /// Getter for orig_text stored in metadata
    pub fn orig_text(&self) -> Option<&Box<str>> {
        if let Some(meta) = &self.metadata
            && let Some(orig) = &meta.orig_text
        {
            Some(orig)
        } else {
            None
        }
    }

    /// Getter for ansi_info stored in metadata
    pub fn ansi_info(&self) -> Option<&Vec<(usize, usize)>> {
        if let Some(meta) = &self.metadata
            && let Some(info) = &meta.ansi_info
        {
            Some(info)
        } else {
            None
        }
    }

    /// Getter for matching_ranges stored in metadata
    pub fn matching_ranges(&self) -> Option<&[(usize, usize)]> {
        if let Some(meta) = &self.metadata {
            meta.matching_ranges.as_ref().map(|v| v.as_ref() as &[(usize, usize)])
        } else {
            None
        }
    }
}

impl DefaultSkimItem {
    /// Get the display text (with ANSI codes if present) for rendering purposes
    #[inline]
    #[allow(dead_code)]
    pub fn get_display_text(&self) -> &str {
        &self.text
    }
}

impl SkimItem for DefaultSkimItem {
    #[inline]
    fn text(&self) -> Cow<'_, str> {
        // Return stripped text for matching when ANSI is enabled
        if let Some(stripped) = self.stripped_text() {
            Cow::Borrowed(stripped)
        } else {
            Cow::Borrowed(&self.text)
        }
    }

    fn output(&self) -> Cow<'_, str> {
        if let Some(orig) = self.orig_text() {
            Cow::Borrowed(orig)
        } else {
            Cow::Borrowed(&self.text)
        }
    }

    fn get_matching_ranges(&self) -> Option<&[(usize, usize)]> {
        // Return matching ranges if present in metadata
        self.matching_ranges()
    }

    fn display<'a>(&'a self, context: DisplayContext) -> Line<'a> {
        // If we have ANSI info, we need to handle ANSI codes properly and map matches
        if self.ansi_info().is_some() {
            // Parse the ANSI text using ansi-to-tui to get proper styled spans
            let text_bytes = self.text.as_bytes().to_vec();
            let parsed_text = match text_bytes.into_text() {
                Ok(text) => text,
                Err(_) => {
                    // Fallback to plain text if parsing fails
                    return context.to_line(Cow::Borrowed(&self.text));
                }
            };

            // Extract all spans from the parsed text (should be a single line)
            let all_spans: Vec<Span> = parsed_text.lines.into_iter().flat_map(|line| line.spans).collect();

            // Now apply highlighting based on matched positions
            // We need to map match positions from stripped text to original text
            match context.matches {
                crate::Matches::CharIndices(ref indices) => {
                    // Indices are already in stripped text coordinates (same as parsed ANSI text)
                    // No need to remap since both matching and ANSI parsing strip the codes
                    let highlight_positions: std::collections::HashSet<usize> = indices.iter().copied().collect();

                    // Apply highlighting to characters at those positions
                    let mut new_spans = Vec::new();
                    let mut char_idx = 0;

                    for span in all_spans {
                        let mut current_content = String::new();
                        let mut highlighted_content = String::new();
                        let base_style = span.style;

                        for ch in span.content.chars() {
                            if highlight_positions.contains(&char_idx) {
                                // Flush normal content if any
                                if !current_content.is_empty() {
                                    // Combine ANSI style with context base_style
                                    new_spans.push(Span::styled(
                                        current_content.clone(),
                                        merge_styles(context.base_style, base_style),
                                    ));
                                    current_content.clear();
                                }
                                highlighted_content.push(ch);
                            } else {
                                // Flush highlighted content if any
                                if !highlighted_content.is_empty() {
                                    // Combine styles: use highlight bg, preserve ANSI fg and modifiers
                                    new_spans.push(Span::styled(
                                        highlighted_content.clone(),
                                        merge_styles(base_style, context.matched_syle),
                                    ));
                                    highlighted_content.clear();
                                }
                                current_content.push(ch);
                            }
                            char_idx += 1;
                        }

                        // Flush remaining content
                        if !current_content.is_empty() {
                            // Combine ANSI style with context base_style
                            new_spans.push(Span::styled(
                                current_content,
                                merge_styles(context.base_style, base_style),
                            ));
                        }
                        if !highlighted_content.is_empty() {
                            // Combine styles: use highlight bg, preserve ANSI fg and modifiers
                            new_spans.push(Span::styled(
                                highlighted_content,
                                merge_styles(base_style, context.matched_syle),
                            ));
                        }
                    }

                    Line::from(new_spans)
                }
                crate::Matches::CharRange(start, end) => {
                    // Positions are already in stripped text coordinates (same as parsed ANSI text)
                    // No need to remap since both matching and ANSI parsing strip the codes

                    // Apply highlighting to the range
                    let mut new_spans = Vec::new();
                    let mut char_idx = 0;

                    for span in all_spans {
                        let mut before = String::new();
                        let mut highlighted = String::new();
                        let mut after = String::new();
                        let base_style = span.style;

                        for ch in span.content.chars() {
                            if char_idx < start {
                                before.push(ch);
                            } else if char_idx < end {
                                highlighted.push(ch);
                            } else {
                                after.push(ch);
                            }
                            char_idx += 1;
                        }

                        if !before.is_empty() {
                            // Combine ANSI style with context base_style
                            new_spans.push(Span::styled(before, merge_styles(context.base_style, base_style)));
                        }
                        if !highlighted.is_empty() {
                            // Combine ANSI style with context matched_syle
                            new_spans.push(Span::styled(
                                highlighted,
                                merge_styles(base_style, context.matched_syle),
                            ));
                        }
                        if !after.is_empty() {
                            // Combine ANSI style with context base_style
                            new_spans.push(Span::styled(after, merge_styles(context.base_style, base_style)));
                        }
                    }

                    Line::from(new_spans)
                }
                crate::Matches::ByteRange(start, end) => {
                    // Convert byte positions to char positions in stripped text
                    let stripped = self.stripped_text().unwrap();
                    let char_start = stripped.get(0..start).map(|s| s.chars().count()).unwrap_or(0);
                    let char_end = stripped
                        .get(0..end)
                        .map(|s| s.chars().count())
                        .unwrap_or(stripped.chars().count());

                    // Apply highlighting to the range
                    let mut new_spans = Vec::new();
                    let mut char_idx = 0;

                    for span in all_spans {
                        let mut before = String::new();
                        let mut highlighted = String::new();
                        let mut after = String::new();
                        let base_style = span.style;

                        for ch in span.content.chars() {
                            if char_idx < char_start {
                                before.push(ch);
                            } else if char_idx < char_end {
                                highlighted.push(ch);
                            } else {
                                after.push(ch);
                            }
                            char_idx += 1;
                        }

                        if !before.is_empty() {
                            // Combine ANSI style with context base_style
                            new_spans.push(Span::styled(before, merge_styles(context.base_style, base_style)));
                        }
                        if !highlighted.is_empty() {
                            // Combine ANSI style with context matched_syle
                            new_spans.push(Span::styled(
                                highlighted,
                                merge_styles(base_style, context.matched_syle),
                            ));
                        }
                        if !after.is_empty() {
                            // Combine ANSI style with context base_style
                            new_spans.push(Span::styled(after, merge_styles(context.base_style, base_style)));
                        }
                    }

                    Line::from(new_spans)
                }
                crate::Matches::None => Line::from(all_spans),
            }
        } else {
            // No ANSI mapping needed, use text as-is
            context.to_line(Cow::Borrowed(&self.text))
        }
    }

    fn get_index(&self) -> usize {
        self.index
    }

    fn set_index(&mut self, index: usize) {
        self.index = index;
    }
}

/// Strip ANSI escape sequences from a string
///
/// This function removes all ANSI escape codes (CSI sequences, OSC sequences, etc.)
/// from the input string, leaving only the visible text.
///
/// Returns the stripped string as well as a mapping of positions. Each element in the
/// mapping vector is a tuple `(byte_position, char_position)` where:
/// - `byte_position`: The byte offset in the original raw string
/// - `char_position`: The character index in the original raw string
///
/// For the character at position `i` in the stripped string:
/// - `mapping[i].0` gives its byte position in the original string
/// - `mapping[i].1` gives its character index in the original string
///
/// Examples of ANSI codes that are stripped:
/// - `\x1b[31m` (set foreground color to red)
/// - `\x1b[01;32m` (bold green)
/// - `\x1b[0m` (reset)
/// - `\x1b]0;title\x07` (OSC sequences)
pub fn strip_ansi(text: &str) -> (String, Vec<(usize, usize)>) {
    let mut result = String::with_capacity(text.len());
    let mut index_mapping = Vec::new();
    let mut chars = text.char_indices().peekable();
    let mut char_idx = 0;

    while let Some((byte_pos, ch)) = chars.next() {
        if ch == '\x1b' {
            // ESC sequence detected
            if let Some(&(_, next_ch)) = chars.peek() {
                match next_ch {
                    '[' => {
                        // CSI sequence: ESC [ ... (ending with a letter)
                        chars.next(); // consume '['
                        char_idx += 1;
                        while let Some(&(_, c)) = chars.peek() {
                            chars.next();
                            char_idx += 1;
                            if c.is_ascii_alphabetic() {
                                break;
                            }
                        }
                    }
                    ']' => {
                        // OSC sequence: ESC ] ... (ending with BEL or ESC \)
                        chars.next(); // consume ']'
                        char_idx += 1;
                        while let Some((_, c)) = chars.next() {
                            char_idx += 1;
                            if c == '\x07' {
                                // BEL
                                break;
                            }
                            if c == '\x1b'
                                && let Some(&(_, '\\')) = chars.peek()
                            {
                                chars.next(); // consume '\'
                                char_idx += 1;
                                break;
                            }
                        }
                    }
                    '(' | ')' | '#' | '%' => {
                        // Other escape sequences
                        chars.next(); // consume the next char
                        char_idx += 1;
                        chars.next(); // and one more
                        char_idx += 1;
                    }
                    _ => {
                        // Unknown escape sequence, consume next char
                        chars.next();
                        char_idx += 1;
                    }
                }
            }
        } else {
            result.push(ch);
            index_mapping.push((byte_pos, char_idx));
        }
        char_idx += 1;
    }

    (result, index_mapping)
}

/// Replace the ANSI ESC code by a ?
///
/// Unsafe: bytes are parsed back from the original string or b'?'
/// No risk associated
fn escape_ansi(raw: &str) -> String {
    unsafe { String::from_utf8_unchecked(raw.bytes().map(|b| if b == 27 { b'?' } else { b }).collect()) }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use super::*;

    #[test]
    fn test_strip_ansi() {
        // Test basic ANSI color codes
        // "\x1b[31mred\x1b[0m" has chars at positions: 0=ESC, 1=[, 2=3, 3=1, 4=m, 5=r, 6=e, 7=d, 8=ESC, 9=[, 10=0, 11=m
        let (text, mapping) = strip_ansi("\x1b[31mred\x1b[0m");
        assert_eq!(text, "red");
        assert_eq!(mapping, vec![(5, 5), (6, 6), (7, 7)]);

        let (text, mapping) = strip_ansi("\x1b[01;32mgreen\x1b[0m");
        assert_eq!(text, "green");
        assert_eq!(mapping, vec![(8, 8), (9, 9), (10, 10), (11, 11), (12, 12)]);

        let (text, mapping) = strip_ansi("\x1b[01;34mblue\x1b[0m");
        assert_eq!(text, "blue");
        assert_eq!(mapping, vec![(8, 8), (9, 9), (10, 10), (11, 11)]);

        // Test text without ANSI codes
        let (text, mapping) = strip_ansi("plain text");
        assert_eq!(text, "plain text");
        assert_eq!(
            mapping,
            vec![
                (0, 0),
                (1, 1),
                (2, 2),
                (3, 3),
                (4, 4),
                (5, 5),
                (6, 6),
                (7, 7),
                (8, 8),
                (9, 9)
            ]
        );

        // Test multiple ANSI sequences
        let (text, mapping) = strip_ansi("\x1b[31mred\x1b[0m and \x1b[32mgreen\x1b[0m");
        assert_eq!(text, "red and green");
        assert_eq!(
            mapping,
            vec![
                (5, 5),
                (6, 6),
                (7, 7),
                (12, 12),
                (13, 13),
                (14, 14),
                (15, 15),
                (16, 16),
                (22, 22),
                (23, 23),
                (24, 24),
                (25, 25),
                (26, 26)
            ]
        );

        // Test ANSI codes in the middle of text
        let (text, mapping) = strip_ansi("be\x1b[01;34mf\x1b[0more");
        assert_eq!(text, "before");
        assert_eq!(mapping, vec![(0, 0), (1, 1), (10, 10), (15, 15), (16, 16), (17, 17)]);

        // Test real ls --color output
        let (text, mapping) = strip_ansi("\x1b[01;32mbench.sh\x1b[0m");
        assert_eq!(text, "bench.sh");
        assert_eq!(
            mapping,
            vec![
                (8, 8),
                (9, 9),
                (10, 10),
                (11, 11),
                (12, 12),
                (13, 13),
                (14, 14),
                (15, 15)
            ]
        );

        let (text, mapping) = strip_ansi("\x1b[01;34mbin\x1b[0m");
        assert_eq!(text, "bin");
        assert_eq!(mapping, vec![(8, 8), (9, 9), (10, 10)]);

        // Test with multi-byte UTF-8 characters to verify byte vs char position difference
        // "😀" is 4 bytes but 1 char - when followed by ANSI codes, byte and char positions diverge
        let (text, mapping) = strip_ansi("😀\x1b[32mtext\x1b[0m");
        assert_eq!(text, "😀text");
        // Original: "😀\x1b[32mtext\x1b[0m"
        // byte positions: 😀=0-3, \x1b=4, [=5, 3=6, 2=7, m=8, t=9, e=10, x=11, t=12, \x1b=13, [=14, 0=15, m=16
        // char positions: 😀=0, \x1b=1, [=2, 3=3, 2=4, m=5, t=6, e=7, x=8, t=9, \x1b=10, [=11, 0=12, m=13
        // After stripping: "😀text"
        // stripped[0]='😀' -> (byte=0, char=0)
        // stripped[1]='t' -> (byte=9, char=6) <- Here byte and char positions differ!
        assert_eq!(mapping, vec![(0, 0), (9, 6), (10, 7), (11, 8), (12, 9)]);
    }

    #[test]
    fn test_ansi_matching_and_display() {
        use crate::{DisplayContext, Matches, SkimItem};
        use ratatui::style::{Color, Style};
        use regex::Regex;

        // Create an item with ANSI codes
        let input = "\x1b[32mgreen\x1b[0m text";
        let delimiter = Regex::new(r"\s+").unwrap();
        let item = DefaultSkimItem::new(
            input,
            true, // ansi_enabled
            &[],
            &[],
            &delimiter,
            0,
        );

        // text() should return stripped text for matching
        assert_eq!(item.text(), "green text");

        // Verify we have ANSI info
        assert!(item.ansi_info().is_some());

        // Create a match context as if we matched "text" (positions 6-10 in stripped string)
        let context = DisplayContext {
            score: 100,
            matches: Matches::CharRange(6, 10),
            container_width: 80,
            base_style: Style::default(),
            matched_syle: Style::default().fg(Color::Yellow),
        };

        // display() should map the match positions back to the original ANSI text
        let line = item.display(context);

        // The line should have the original ANSI codes intact
        // We can't easily verify the exact ANSI codes in the output, but we can check
        // that it's not empty and has multiple spans (original text + highlighted match)
        assert!(!line.spans.is_empty());
    }

    #[test]
    fn test_ansi_char_indices_mapping() {
        use crate::{DisplayContext, Matches, SkimItem};
        use ratatui::style::{Color, Style};
        use regex::Regex;

        // Create an item with ANSI codes: "😀\x1b[32mtext\x1b[0m"
        let input = "😀\x1b[32mtext\x1b[0m";
        let delimiter = Regex::new(r"\s+").unwrap();
        let item = DefaultSkimItem::new(
            input,
            true, // ansi_enabled
            &[],
            &[],
            &delimiter,
            0,
        );

        // text() should return "😀text"
        assert_eq!(item.text(), "😀text");

        // Match indices 1,2 in stripped text (the 't' and 'e')
        let context = DisplayContext {
            score: 100,
            matches: Matches::CharIndices(vec![1, 2]),
            container_width: 80,
            base_style: Style::default(),
            matched_syle: Style::default().fg(Color::Yellow),
        };

        // display() should map these to positions 6,7 in original text
        let line = item.display(context);
        assert!(!line.spans.is_empty());
    }

    #[test]
    fn test_text_returns_stripped() {
        use crate::SkimItem;
        use regex::Regex;

        let delimiter = Regex::new(r"\s+").unwrap();

        // Test with ANSI enabled
        let item_ansi = DefaultSkimItem::new(
            "\x1b[31mred\x1b[0m",
            true, // ansi_enabled
            &[],
            &[],
            &delimiter,
            0,
        );
        assert_eq!(
            item_ansi.text(),
            "red",
            "text() should return stripped text when ANSI is enabled"
        );

        // Test with ANSI disabled
        let item_no_ansi = DefaultSkimItem::new(
            "\x1b[31mred\x1b[0m",
            false, // ansi_enabled
            &[],
            &[],
            &delimiter,
            0,
        );
        assert_eq!(
            item_no_ansi.text(),
            "?[31mred?[0m",
            "text() should return text with ? when ANSI is disabled"
        );
    }

    #[test]
    fn test_highlighting_applied() {
        use crate::{DisplayContext, Matches, SkimItem};
        use ratatui::style::{Color, Style};
        use regex::Regex;

        let delimiter = Regex::new(r"\s+").unwrap();

        // Create item with ANSI codes: "\x1b[32mgreen\x1b[0m"
        let item = DefaultSkimItem::new(
            "\x1b[32mgreen\x1b[0m",
            true, // ansi_enabled
            &[],
            &[],
            &delimiter,
            0,
        );

        // Create display context with yellow background highlight for character 0 (the 'g')
        let context = DisplayContext {
            score: 100,
            matches: Matches::CharIndices(vec![0]),
            container_width: 80,
            base_style: Style::default(),
            matched_syle: Style::default().bg(Color::Yellow),
        };

        let line = item.display(context);

        // The line should have spans with highlighting
        // At least one span should have the yellow background
        let has_highlight = line.spans.iter().any(|span| span.style.bg == Some(Color::Yellow));
        assert!(has_highlight, "Highlighted character should have yellow background");

        // The green foreground from ANSI should be preserved in at least one span
        let has_green_fg = line.spans.iter().any(|span| span.style.fg == Some(Color::Green));
        assert!(has_green_fg, "ANSI green foreground should be preserved");
    }

    #[test]
    fn test_char_range_highlighting() {
        use crate::{DisplayContext, Matches, SkimItem};
        use ratatui::style::{Color, Style};
        use regex::Regex;

        let delimiter = Regex::new(r"\s+").unwrap();

        // Create item with ANSI codes: "\x1b[32mgreen\x1b[0m"
        let item = DefaultSkimItem::new(
            "\x1b[32mgreen\x1b[0m",
            true, // ansi_enabled
            &[],
            &[],
            &delimiter,
            0,
        );

        // Create display context with yellow background highlight for characters 1-3 ('re')
        let context = DisplayContext {
            score: 100,
            matches: Matches::CharRange(1, 3),
            container_width: 80,
            base_style: Style::default(),
            matched_syle: Style::default().bg(Color::Yellow),
        };

        let line = item.display(context);

        // Should have multiple spans: before, highlighted, after
        assert!(line.spans.len() >= 2, "Should have multiple spans for highlighting");

        // At least one span should have the yellow background (the highlighted portion)
        let has_highlight = line.spans.iter().any(|span| span.style.bg == Some(Color::Yellow));
        assert!(has_highlight, "Highlighted characters should have yellow background");

        // The green foreground from ANSI should be preserved
        let has_green_fg = line.spans.iter().any(|span| span.style.fg == Some(Color::Green));
        assert!(has_green_fg, "ANSI green foreground should be preserved");
    }

    #[test]
    fn test_byte_range_highlighting() {
        use crate::{DisplayContext, Matches, SkimItem};
        use ratatui::style::{Color, Style};
        use regex::Regex;

        let delimiter = Regex::new(r"\s+").unwrap();

        // Create item with ANSI codes: "\x1b[32mgreen\x1b[0m"
        let item = DefaultSkimItem::new(
            "\x1b[32mgreen\x1b[0m",
            true, // ansi_enabled
            &[],
            &[],
            &delimiter,
            0,
        );

        // Create display context with yellow background highlight for bytes 1-3 ('re' in stripped text)
        let context = DisplayContext {
            score: 100,
            matches: Matches::ByteRange(1, 3),
            container_width: 80,
            base_style: Style::default(),
            matched_syle: Style::default().bg(Color::Yellow),
        };

        let line = item.display(context);

        // Should have multiple spans for highlighting
        assert!(!line.spans.is_empty(), "Should have spans");

        // At least one span should have the yellow background (the highlighted portion)
        let has_highlight = line.spans.iter().any(|span| span.style.bg == Some(Color::Yellow));
        assert!(has_highlight, "Highlighted bytes should have yellow background");

        // The green foreground from ANSI should be preserved
        let has_green_fg = line.spans.iter().any(|span| span.style.fg == Some(Color::Green));
        assert!(has_green_fg, "ANSI green foreground should be preserved");
    }

    #[test]
    fn test_matching_with_ansi_basic() {
        use crate::SkimItem;
        use regex::Regex;

        let delimiter = Regex::new(r"\s+").unwrap();

        // Create item with ANSI codes: "\x1b[32mgreen_text\x1b[0m"
        let item = DefaultSkimItem::new(
            "\x1b[32mgreen_text\x1b[0m",
            true, // ansi_enabled
            &[],
            &[], // no matching fields restriction
            &delimiter,
            0,
        );

        // text() should return stripped text "green_text"
        assert_eq!(item.text(), "green_text");

        // With no matching_fields, get_matching_ranges should return None (match whole text)
        assert!(item.get_matching_ranges().is_none());

        // Verify the stripped_text and ansi_info are populated correctly
        assert!(item.stripped_text().is_some());
        assert!(item.ansi_info().is_some());
        assert_eq!(item.stripped_text().unwrap().as_ref(), "green_text");
    }

    #[test]
    fn test_null_delimiter_with_matching_fields() {
        use crate::SkimItem;
        use crate::field::FieldRange;
        use regex::Regex;

        // Test with null byte delimiter and matching_fields
        let delimiter = Regex::new("\x00").unwrap();
        let text = "a\x00b\x00c";

        // Create item with matching field 2
        let item = DefaultSkimItem::new(
            text,
            false,                    // no ansi
            &[],                      // no transform fields
            &[FieldRange::Single(2)], // match field 2
            &delimiter,
            0,
        );

        // text() should return text with null bytes stripped for display
        assert_eq!(item.text(), "abc");

        // get_matching_ranges should return the range for field 2 in the stripped text
        let ranges = item.get_matching_ranges().expect("Should have matching ranges");
        assert_eq!(ranges.len(), 1, "Should have one matching range");

        // Field 2 is "b" which is at position 1 in the stripped text "abc"
        assert_eq!(ranges[0], (1, 2), "Field 2 should be at position 1-2 in stripped text");

        // Verify the substring matches what we expect
        let stripped_text = item.text();
        let field_text = &stripped_text[ranges[0].0..ranges[0].1];
        assert_eq!(field_text, "b", "Field text should be 'b'");
    }
}
