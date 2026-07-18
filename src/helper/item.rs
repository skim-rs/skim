//! Skim item helpers
//! Including the `DefaultSkimItem`
use crate::field::{FieldRange, parse_matching_fields, parse_transform_fields};
use crate::tui::util::merge_styles;
use crate::{DisplayContext, Matches, SkimItem};
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

    /// Metadata containing miscellaneous fields when special options are used
    metadata: Option<Box<DefaultSkimItemMetadata>>,
}

/// Additional metadata for a `SkimItem`
#[derive(Debug, Default)]
pub struct DefaultSkimItemMetadata {
    /// The text that will be output when user press `enter`
    /// `Some(..)` => the original input is transformed, could not output `text` directly
    /// `None` => that it is safe to output `text` directly
    orig_text: Option<Box<str>>,

    /// The text stripped of all ansi sequences, used for matching
    /// Will be Some when ANSI is enabled, None otherwise
    stripped_text: Option<Box<str>>,

    /// A mapping of positions from stripped text to original text.
    /// Each element is (`byte_position`, `char_position`) in the original raw text.
    /// Will be empty if ansi is disabled.
    ansi_info: Option<Vec<(usize, usize)>>,

    /// The ranges on which to perform matching
    matching_ranges: Option<Vec<(usize, usize)>>,

    /// Byte ranges (in the display/matching text) of fields hidden via `--hide-nth`.
    /// Characters inside these ranges are removed from the rendered line and ignored
    /// for match highlighting and horizontal scrolling, but remain part of the text
    /// used for matching so they stay searchable.
    hidden_ranges: Option<Vec<(usize, usize)>>,

    /// Whether the item should be disabled or not
    disabled: bool,
}

impl DefaultSkimItem {
    /// Create a new `DefaultSkimItem` from text
    #[must_use]
    pub fn new(
        orig_text: &str,
        ansi_enabled: bool,
        trans_fields: &[FieldRange],
        matching_fields: &[FieldRange],
        delimiter: &Regex,
    ) -> Self {
        let using_transform_fields = !trans_fields.is_empty();
        let contains_ansi = Self::contains_ansi_escape(orig_text);

        //        transformed | ANSI             | output
        //------------------------------------------------------
        //                    +- T -> trans+ANSI | ANSI
        //                    |                  |
        //      +- T -> trans +- F -> trans      | orig
        // orig |                                |
        //      +- F -> orig  +- T -> ANSI     ==| ANSI
        //                    |                  |
        //                    +- F -> orig       | orig

        let (mut orig_text, mut temp_text): (Option<String>, Box<str>) = match (using_transform_fields, ansi_enabled) {
            (true, true) => {
                let transformed = parse_transform_fields(delimiter, orig_text, trans_fields);
                (Some(orig_text.into()), Box::from(transformed))
            }
            (true, false) => {
                let transformed = parse_transform_fields(delimiter, &escape_ansi(orig_text), trans_fields);
                (Some(orig_text.into()), Box::from(transformed))
            }
            (false, false) if contains_ansi => (None, escape_ansi(orig_text).into()),
            (false, true | false) => (None, Box::from(orig_text)),
        };

        // Keep track of whether we have null bytes for special handling
        let has_null_bytes = memchr::memchr(b'\0', temp_text.as_bytes()).is_some();

        // Preserve original text with null bytes for output if needed
        if has_null_bytes && orig_text.is_none() {
            orig_text = Some(temp_text.to_string());
        }

        // Strip null bytes from text used for display and matching
        // Null bytes are control characters that cause rendering issues (zero-width)
        // They are preserved in orig_text for output
        if has_null_bytes {
            temp_text = temp_text.to_string().replace('\0', "").into_boxed_str();
        }

        let (stripped_text, ansi_info) = if ansi_enabled && contains_ansi {
            let (stripped, info) = strip_ansi(&temp_text);
            (Some(stripped), Some(info))
        } else {
            (None, None)
        };

        // Calculate matching ranges on text WITHOUT null bytes (after stripping)
        // This ensures the byte positions match the actual text used for matching
        let matching_ranges = if matching_fields.is_empty() {
            None
        } else {
            // Use stripped text for matching ranges when ANSI is enabled
            let text_for_matching = if let Some(stripped) = stripped_text.as_ref() {
                stripped
            } else {
                temp_text.as_ref()
            };

            // Parse the original text with null bytes to determine field boundaries
            // Then extract those fields, strip null bytes, and recalculate positions
            // When has_null_bytes is true, orig_text was set to Some above, so unwrap is safe.
            let orig_text_for_fields = if has_null_bytes {
                orig_text.as_deref().unwrap_or(text_for_matching)
            } else {
                text_for_matching
            };

            if has_null_bytes {
                // Extract each field from the original text (with null bytes)
                // then strip null bytes and build new ranges in the cleaned text
                let mut adjusted_ranges = Vec::new();

                for field in matching_fields {
                    // Get the field text from original (with null bytes)
                    if let Some(field_text) = crate::field::get_string_by_field(delimiter, orig_text_for_fields, field)
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
        };

        let metadata =
            if orig_text.is_some() || stripped_text.is_some() || ansi_info.is_some() || matching_ranges.is_some() {
                Some(Box::new(DefaultSkimItemMetadata {
                    orig_text: orig_text.map(std::string::String::into_boxed_str),
                    stripped_text: stripped_text.map(std::string::String::into_boxed_str),
                    ansi_info,
                    matching_ranges,
                    hidden_ranges: None,
                    disabled: false,
                }))
            } else {
                None
            };

        DefaultSkimItem {
            text: temp_text,
            metadata,
        }
    }

    /// Builder-style setter for the fields hidden from display (via `--hide-nth`).
    ///
    /// The fields are resolved against the item's display/matching text — which is
    /// exactly what [`text()`](Self::text) returns (the ANSI-stripped text under
    /// `--ansi`, otherwise the raw text) — so this must be called after construction.
    /// The requested fields stay part of `text()` (and therefore searchable); they are
    /// only removed from the rendered line and ignored for highlighting and hscroll.
    ///
    /// A no-op when `hidden_fields` is empty or resolves to no ranges.
    #[must_use]
    pub fn hidden_fields(mut self, hidden_fields: &[FieldRange], delimiter: &Regex) -> Self {
        if hidden_fields.is_empty() {
            return self;
        }
        // Resolve the ranges before touching `self.metadata`; the `text()` borrow must
        // end before the mutable borrow below.
        let ranges = {
            let text = self.text();
            normalize_ranges(&parse_matching_fields(delimiter, text.as_ref(), hidden_fields))
        };
        if !ranges.is_empty() {
            self.metadata.get_or_insert_default().hidden_ranges = Some(ranges);
        }
        self
    }

    fn contains_ansi_escape(s: &str) -> bool {
        memchr::memchr(b'\x1b', s.as_bytes()).is_some()
    }

    /// Mark the item as disabled
    pub fn disable(&mut self) {
        self.metadata.get_or_insert_default().disabled = true;
    }

    /// Getter for `stripped_text` stored in the metadata
    #[must_use]
    pub fn stripped_text(&self) -> Option<&str> {
        if let Some(meta) = &self.metadata
            && let Some(stripped_text) = &meta.stripped_text
        {
            Some(stripped_text.as_ref())
        } else {
            None
        }
    }

    /// Getter for `orig_text` stored in metadata
    #[must_use]
    pub fn orig_text(&self) -> Option<&str> {
        if let Some(meta) = &self.metadata
            && let Some(orig) = &meta.orig_text
        {
            Some(orig.as_ref())
        } else {
            None
        }
    }

    /// Getter for `ansi_info` stored in metadata
    #[must_use]
    pub fn ansi_info(&self) -> Option<&Vec<(usize, usize)>> {
        if let Some(meta) = &self.metadata
            && let Some(info) = &meta.ansi_info
        {
            Some(info)
        } else {
            None
        }
    }

    /// Getter for `matching_ranges` stored in metadata
    #[must_use]
    pub fn matching_ranges(&self) -> Option<&[(usize, usize)]> {
        if let Some(meta) = &self.metadata {
            meta.matching_ranges.as_ref().map(|v| v.as_ref() as &[(usize, usize)])
        } else {
            None
        }
    }

    /// Getter for `hidden_ranges` stored in metadata
    #[must_use]
    pub fn hidden_ranges(&self) -> Option<&[(usize, usize)]> {
        if let Some(meta) = &self.metadata {
            meta.hidden_ranges.as_ref().map(|v| v.as_ref() as &[(usize, usize)])
        } else {
            None
        }
    }
}

impl DefaultSkimItem {
    /// Get the display text (with ANSI codes if present) for rendering purposes
    #[inline]
    #[allow(dead_code)]
    #[must_use]
    pub fn get_display_text(&self) -> &str {
        &self.text
    }
}

impl From<String> for DefaultSkimItem {
    fn from(value: String) -> Self {
        Self {
            text: Box::from(value),
            metadata: None,
        }
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

    fn hidden_ranges(&self) -> Option<&[(usize, usize)]> {
        self.hidden_ranges()
    }

    // The display function handles ANSI stripping, field highlighting, and match
    // rendering in a single pass; splitting it would require duplicating context handling.
    #[allow(clippy::too_many_lines)]
    fn display(&self, context: DisplayContext) -> Line<'_> {
        // If we have ANSI info, we need to handle ANSI codes properly and map matches
        if self.ansi_info().is_some() {
            // Parse the ANSI text using ansi-to-tui to get proper styled spans
            let text_bytes = self.text.as_bytes().to_vec();
            let Ok(parsed_text) = text_bytes.into_text() else {
                // Fallback to plain text if parsing fails
                return context.to_line(Cow::Borrowed(&self.text));
            };

            // Extract all spans from the parsed text (should be a single line)
            let all_spans: Vec<Span> = parsed_text.lines.into_iter().flat_map(|line| line.spans).collect();

            // When fields are hidden (--hide-nth), drop the hidden characters from the
            // parsed spans while preserving their ANSI styling, and remap the match
            // positions into the resulting visible coordinate space. The remaining
            // highlighting logic then runs unchanged on visible-coordinate CharIndices.
            let (all_spans, matches) = if let Some(hidden) = self.hidden_ranges() {
                let stripped = self.text();
                let (_, map) = project_visible_text(stripped.as_ref(), hidden);
                let visible_spans = retain_visible_spans(all_spans, &map);
                let indices = project_match_indices(stripped.as_ref(), &context.matches, &map);
                (visible_spans, Matches::CharIndices(indices))
            } else {
                (all_spans, context.matches.clone())
            };

            // Now apply highlighting based on matched positions
            // We need to map match positions from stripped text to original text
            match matches {
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
                                        merge_styles(base_style, context.matched_style),
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
                                merge_styles(base_style, context.matched_style),
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
                            // Combine ANSI style with context matched_style
                            new_spans.push(Span::styled(
                                highlighted,
                                merge_styles(base_style, context.matched_style),
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
                    let char_start = stripped.get(0..start).map_or(0, |s| s.chars().count());
                    let char_end = stripped
                        .get(0..end)
                        .map_or(stripped.chars().count(), |s| s.chars().count());

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
                            // Combine ANSI style with context matched_style
                            new_spans.push(Span::styled(
                                highlighted,
                                merge_styles(base_style, context.matched_style),
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
        } else if let Some(hidden) = self.hidden_ranges() {
            // Non-ANSI hidden path: remove the hidden characters and remap the match
            // highlight positions into the visible coordinate space.
            let (visible, map) = project_visible_text(&self.text, hidden);
            let indices = project_match_indices(&self.text, &context.matches, &map);
            DisplayContext {
                score: context.score,
                matches: Matches::CharIndices(indices),
                container_width: context.container_width,
                base_style: context.base_style,
                matched_style: context.matched_style,
            }
            .to_line(Cow::Owned(visible))
        } else {
            // No ANSI mapping needed, use text as-is
            context.to_line(Cow::Borrowed(&self.text))
        }
    }

    fn disabled(&self) -> bool {
        self.metadata.as_ref().is_some_and(|x| x.disabled)
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
#[must_use]
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

/// Sort and merge a list of byte ranges into a canonical, non-overlapping form.
///
/// Empty ranges are dropped. Overlapping or touching ranges are merged so callers
/// can iterate the result assuming disjoint, ascending ranges.
#[must_use]
pub(crate) fn normalize_ranges(ranges: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut sorted: Vec<(usize, usize)> = ranges.iter().copied().filter(|(s, e)| e > s).collect();
    sorted.sort_unstable();

    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(sorted.len());
    for (start, end) in sorted {
        if let Some(last) = merged.last_mut()
            && start <= last.1
        {
            last.1 = last.1.max(end);
        } else {
            merged.push((start, end));
        }
    }
    merged
}

/// Remove the hidden byte ranges from `text`, returning the visible string and a
/// map from each original char index to its `Some(visible char index)`, or `None`
/// when that char falls inside a hidden range.
///
/// `hidden` must be normalized (see [`normalize_ranges`]): sorted, disjoint byte ranges.
#[must_use]
pub(crate) fn project_visible_text(text: &str, hidden: &[(usize, usize)]) -> (String, Vec<Option<usize>>) {
    let mut visible = String::with_capacity(text.len());
    let mut map = Vec::new();
    let mut vis_idx = 0usize;
    let mut hi = 0usize;

    for (byte_pos, ch) in text.char_indices() {
        while hi < hidden.len() && byte_pos >= hidden[hi].1 {
            hi += 1;
        }
        let is_hidden = hi < hidden.len() && byte_pos >= hidden[hi].0 && byte_pos < hidden[hi].1;
        if is_hidden {
            map.push(None);
        } else {
            map.push(Some(vis_idx));
            visible.push(ch);
            vis_idx += 1;
        }
    }

    (visible, map)
}

/// Drop hidden characters from already-parsed styled spans while preserving each
/// span's style, keeping the ANSI colors of the surviving characters intact.
///
/// `map` is the per-char index map produced by [`project_visible_text`]; the spans
/// are iterated in the same (stripped-text) char order the map is indexed by. Spans
/// that become empty after filtering are dropped.
#[must_use]
pub(crate) fn retain_visible_spans(spans: Vec<Span<'_>>, map: &[Option<usize>]) -> Vec<Span<'static>> {
    let mut out = Vec::with_capacity(spans.len());
    let mut char_idx = 0usize;
    for span in spans {
        let mut content = String::new();
        for ch in span.content.chars() {
            if map.get(char_idx).copied().flatten().is_some() {
                content.push(ch);
            }
            char_idx += 1;
        }
        if !content.is_empty() {
            out.push(Span::styled(content, span.style));
        }
    }
    out
}

/// Convert the matched character positions of `matches` (in full-text coordinates)
/// into visible-text char indices, dropping any that fall inside hidden ranges.
///
/// `map` is the per-char index map produced by [`project_visible_text`]. The result
/// is sorted ascending and deduplicated, ready to feed a `Matches::CharIndices`.
#[must_use]
pub(crate) fn project_match_indices(text: &str, matches: &Matches, map: &[Option<usize>]) -> Vec<usize> {
    let full_indices: Vec<usize> = match matches {
        Matches::CharIndices(indices) => indices.clone(),
        Matches::CharRange(start, end) => (*start..*end).collect(),
        Matches::ByteRange(start, end) => text
            .char_indices()
            .enumerate()
            .filter(|(_, (byte_pos, _))| *byte_pos >= *start && *byte_pos < *end)
            .map(|(char_idx, _)| char_idx)
            .collect(),
        Matches::None => Vec::new(),
    };

    let mut visible: Vec<usize> = full_indices
        .into_iter()
        .filter_map(|ci| map.get(ci).copied().flatten())
        .collect();
    visible.sort_unstable();
    visible.dedup();
    visible
}

#[cfg(test)]
#[path = "item_tests.rs"]
mod test;
