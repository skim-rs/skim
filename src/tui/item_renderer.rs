use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::{ListDirection, ListItem};
use unicode_display_width::width as display_width;

use crate::tui::item_list::ItemList;
use crate::tui::util::{char_display_width, clip_line_to_chars, wrap_text};
use crate::{DisplayContext, MatchRange, item::MatchedItem, theme::ColorTheme};

struct SubLineState {
    is_current: bool,
    is_selected: bool,
    is_first: bool,
    needs_ellipsis: bool,
}

/// Rendering parameters that are constant across all items in one render pass.
pub(crate) struct ItemRenderer<'a> {
    pub theme: &'a ColorTheme,
    pub selector_icon: &'a str,
    pub multi_select_icon: &'a str,
    pub ellipsis: &'a str,
    pub container_width: usize,
    pub wrap: bool,
    pub multiline: Option<&'a str>,
    pub show_score: bool,
    pub show_index: bool,
    pub multi_select: bool,
    pub tabstop: usize,
    pub no_hscroll: bool,
    pub keep_right: bool,
    pub manual_hscroll: i32,
    pub skip_to_pattern: Option<&'a regex::Regex>,
    /// When true, reverse the order of sub-lines within each multiline item
    /// before appending to the output. Required for `BottomToTop` list direction
    /// so that sub-line 0 appears visually above sub-line 1.
    pub reverse_sub_lines: bool,
    /// When true, fill the rest of the current line with the `current` background color
    pub highlight_line: bool,
}

impl<'a> ItemRenderer<'a> {
    /// Build a renderer from an [`ItemList`] and the pre-computed `container_width`
    /// (inner area width minus the icon prefix columns).
    pub fn new_for(list: &'a ItemList, container_width: usize) -> Self {
        Self {
            theme: &list.theme,
            selector_icon: &list.selector_icon,
            multi_select_icon: &list.multi_select_icon,
            ellipsis: &list.ellipsis,
            container_width,
            wrap: list.wrap,
            multiline: list.multiline.as_deref(),
            show_score: list.show_score,
            show_index: list.show_index,
            multi_select: list.multi_select,
            tabstop: list.tabstop,
            no_hscroll: list.no_hscroll,
            keep_right: list.keep_right,
            manual_hscroll: list.manual_hscroll,
            skip_to_pattern: list.skip_to_pattern.as_ref(),
            reverse_sub_lines: list.direction == ListDirection::BottomToTop,
            highlight_line: list.highlight_line,
        }
    }

    /// Render a single `MatchedItem` into one or more flat `ListItem`s (one per
    /// visible sub-line), appending them to `out`.
    ///
    /// `is_current` / `is_selected` control cursor/selection styling.
    /// `skip_subs` leading sub-lines are omitted (for partial top-of-screen scroll).
    /// `available_rows` is how many rows remain; rendering stops when exhausted.
    /// Returns the number of rows appended.
    #[allow(clippy::too_many_arguments)]
    pub fn render_item(
        &self,
        item: &MatchedItem,
        is_current: bool,
        is_selected: bool,
        skip_subs: usize,
        available_rows: usize,
        rows_used: usize,
        out: &mut Vec<ListItem<'static>>,
    ) -> usize {
        let item_text = item.item.text();
        let sub_lines = self.split_sub_lines(item_text.as_ref());
        let (match_start_char, match_end_char) = Self::matched_range(item_text.as_ref(), item.matched_range.as_ref());

        let mut added = 0usize;
        // Collect rows for this item into a temporary buffer so we can reverse
        // them when rendering in BottomToTop direction.
        let mut item_rows: Vec<ListItem<'static>> = Vec::new();

        for (sub_idx, sub_text) in sub_lines.iter().enumerate().skip(skip_subs) {
            if rows_used + added >= available_rows {
                break;
            }

            let is_first = sub_idx == skip_subs;
            let is_top_cutoff = is_first && skip_subs > 0;
            let is_bottom_cutoff = rows_used + added + 1 >= available_rows && sub_idx + 1 < sub_lines.len();
            let list_item = self.render_sub_line(
                item,
                sub_text,
                &sub_lines,
                &SubLineState {
                    is_current,
                    is_selected,
                    is_first,
                    needs_ellipsis: is_top_cutoff || is_bottom_cutoff,
                },
                match_start_char,
                match_end_char,
            );

            item_rows.push(list_item);
            added += 1;
        }

        if self.reverse_sub_lines {
            item_rows.reverse();
        }
        out.extend(item_rows);

        added
    }

    fn split_sub_lines<'b>(&self, item_text: &'b str) -> Vec<&'b str> {
        if let Some(sep) = self.multiline {
            item_text.split(sep).collect()
        } else {
            vec![item_text]
        }
    }

    fn matched_range(item_text: &str, matched_range: Option<&MatchRange>) -> (usize, usize) {
        match matched_range {
            Some(MatchRange::Chars(indices)) => {
                if indices.is_empty() {
                    (0, 0)
                } else {
                    (indices[0], indices[indices.len() - 1] + 1)
                }
            }
            Some(MatchRange::ByteRange(start, end)) => {
                let start_char = item_text[..*start].chars().count();
                let len = item_text[*start..*end].chars().count();
                (start_char, start_char + len)
            }
            Some(MatchRange::CharRange(start, end)) => (*start, *end),
            None => (0, 0),
        }
    }

    fn render_sub_line(
        &self,
        item: &MatchedItem,
        sub_text: &str,
        sub_lines: &[&str],
        state: &SubLineState,
        match_start_char: usize,
        match_end_char: usize,
    ) -> ListItem<'static> {
        let mut all_spans = self.prefix_spans(item, state);
        let content_line = self.content_line(item, sub_text, sub_lines, state, match_start_char, match_end_char);

        if state.needs_ellipsis {
            all_spans.extend(self.trim_with_ellipsis(content_line, state.is_current));
        } else {
            all_spans.extend(content_line.spans);
        }

        self.list_item_from_spans(all_spans, state.is_current)
    }

    fn prefix_spans(&self, item: &MatchedItem, state: &SubLineState) -> Vec<Span<'static>> {
        let mut prefix: Vec<Span<'static>> = Vec::with_capacity(4);
        // When highlight_line is active for the current item, the line-level style fills the
        // entire row with the current background. Reset the bg on prefix spans so the
        // selector/marker columns are not highlighted.
        let prefix_cursor_style = if self.highlight_line && state.is_current {
            self.theme.cursor.bg(self.theme.cursor.bg.unwrap_or(Color::Reset))
        } else {
            self.theme.cursor
        };
        let prefix_selected_style = if self.highlight_line && state.is_current {
            self.theme.selected.bg(self.theme.selected.bg.unwrap_or(Color::Reset))
        } else {
            self.theme.selected
        };

        prefix.push(Span::styled(
            if state.is_first && state.is_current {
                self.selector_icon.to_owned()
            } else {
                str::repeat(" ", self.selector_icon.chars().count())
            },
            prefix_cursor_style,
        ));
        prefix.push(Span::styled(
            if state.is_first && self.multi_select && state.is_selected {
                self.multi_select_icon.to_owned()
            } else {
                str::repeat(" ", self.multi_select_icon.chars().count())
            },
            prefix_selected_style,
        ));
        if self.show_score {
            self.push_first_line_field(&mut prefix, state, item.rank.score);
        }
        if self.show_index {
            self.push_first_line_field(&mut prefix, state, item.rank.index);
        }

        prefix
    }

    fn push_first_line_field(
        &self,
        prefix: &mut Vec<Span<'static>>,
        state: &SubLineState,
        value: impl std::fmt::Display,
    ) {
        let value = format!("[{value}] ");
        prefix.push(Span::styled(
            if state.is_first {
                value.clone()
            } else {
                str::repeat(" ", value.chars().count())
            },
            self.base_style(state.is_current),
        ));
    }

    fn content_line(
        &self,
        item: &MatchedItem,
        sub_text: &str,
        sub_lines: &[&str],
        state: &SubLineState,
        match_start_char: usize,
        match_end_char: usize,
    ) -> Line<'static> {
        if state.is_first {
            self.first_sub_line_content(item, sub_lines[0], state.is_current, match_start_char, match_end_char)
        } else {
            self.continuation_sub_line_content(sub_text, state.is_current)
        }
    }

    fn first_sub_line_content(
        &self,
        item: &MatchedItem,
        first_sub_line: &str,
        is_current: bool,
        match_start_char: usize,
        match_end_char: usize,
    ) -> Line<'static> {
        let first_sub_char_len = first_sub_line.chars().count();
        let dl = item.item.display(DisplayContext {
            score: item.rank.score,
            matches: Self::display_matches(item.matched_range.as_ref()),
            container_width: self.container_width,
            base_style: self.base_style(is_current),
            matched_style: if is_current {
                self.theme.current_match
            } else {
                self.theme.matched
            },
        });

        // In multiline mode the first rendered line must be clipped to the first text sub-line.
        // Without multiline, custom display text may legitimately be longer than item.text().
        let mut line = if self.multiline.is_some() {
            clip_line_to_chars(dl, first_sub_char_len)
        } else {
            dl
        };
        if !self.wrap {
            let (shift, full_width, _, _) = if self.multiline.is_some() {
                self.calc_hscroll(first_sub_line, match_start_char, match_end_char)
            } else {
                self.calc_line_hscroll(&line, first_sub_line, match_start_char, match_end_char)
            };
            line = self.apply_hscroll(line, shift, full_width);
        }
        Self::into_static_line(line)
    }

    fn continuation_sub_line_content(&self, sub_text: &str, is_current: bool) -> Line<'static> {
        let sub_str = sub_text.to_string();
        let (shift, full_width, _, _) = self.calc_hscroll(&sub_str, 0, 0);
        let raw: Line<'static> = Line::from(vec![Span::styled(sub_str, self.base_style(is_current))]);
        if self.wrap {
            raw
        } else {
            let scrolled = self.apply_hscroll(raw, shift, full_width);
            Self::into_static_line(scrolled)
        }
    }

    fn display_matches(matched_range: Option<&MatchRange>) -> crate::Matches {
        match matched_range {
            Some(MatchRange::ByteRange(start, end)) => crate::Matches::ByteRange(*start, *end),
            Some(MatchRange::CharRange(start, end)) => crate::Matches::CharRange(*start, *end),
            Some(MatchRange::Chars(chars)) => crate::Matches::CharIndices(chars.clone()),
            None => crate::Matches::None,
        }
    }

    fn trim_with_ellipsis(&self, content_line: Line<'static>, is_current: bool) -> Vec<Span<'static>> {
        let ell_width = usize::try_from(display_width(self.ellipsis)).unwrap();
        let available = self.container_width.saturating_sub(ell_width);
        let mut trimmed: Vec<Span<'static>> = Vec::new();
        let mut used = 0usize;

        'trim: for span in content_line.spans {
            let span_chars: Vec<char> = span.content.chars().collect();
            for (i, ch) in span_chars.iter().enumerate() {
                let w = char_display_width(*ch);
                if used + w > available {
                    let partial: String = span_chars[..i].iter().collect();
                    if !partial.is_empty() {
                        trimmed.push(Span::styled(partial, span.style));
                    }
                    break 'trim;
                }
                used += w;
            }
            trimmed.push(Span::styled(span.content.into_owned(), span.style));
        }
        trimmed.push(Span::styled(self.ellipsis.to_owned(), self.base_style(is_current)));
        trimmed
    }

    fn list_item_from_spans(&self, spans: Vec<Span<'static>>, is_current: bool) -> ListItem<'static> {
        if self.wrap {
            wrap_text(
                ratatui::text::Text::from(Line::from(spans)),
                self.container_width + self.selector_icon.chars().count() + self.multi_select_icon.chars().count(),
            )
            .into()
        } else {
            let mut line = Line::from(spans);
            // When highlight_line is enabled, set the line's style to the current theme
            // so ratatui fills the entire row width with the current background color.
            if self.highlight_line && is_current {
                line = line.style(self.theme.current);
            }
            line.into()
        }
    }

    fn base_style(&self, is_current: bool) -> ratatui::style::Style {
        if is_current {
            self.theme.current
        } else {
            self.theme.normal
        }
    }

    fn into_static_line(line: Line<'_>) -> Line<'static> {
        line.spans
            .into_iter()
            .map(|span| Span::styled(span.content.into_owned(), span.style))
            .collect()
    }

    // ── hscroll helpers ──────────────────────────────────────────────────────

    fn calc_skip_width(&self, text: &str) -> usize {
        if let Some(regex) = self.skip_to_pattern
            && let Some(mat) = regex.find(text)
        {
            return usize::try_from(display_width(&text[..mat.start()])).unwrap();
        }
        0
    }

    fn calc_hscroll(&self, text: &str, match_start_char: usize, match_end_char: usize) -> (usize, usize, bool, bool) {
        let full_width = self.text_display_width(text);

        self.calc_hscroll_for_width(text, match_start_char, match_end_char, full_width)
    }

    fn calc_line_hscroll(
        &self,
        line: &Line<'_>,
        text: &str,
        match_start_char: usize,
        match_end_char: usize,
    ) -> (usize, usize, bool, bool) {
        self.calc_hscroll_for_width(text, match_start_char, match_end_char, self.line_display_width(line))
    }

    fn calc_hscroll_for_width(
        &self,
        text: &str,
        match_start_char: usize,
        match_end_char: usize,
        full_width: usize,
    ) -> (usize, usize, bool, bool) {
        let ell_w = usize::try_from(display_width(self.ellipsis)).unwrap();
        let available_width = if self.container_width >= ell_w {
            self.container_width
        } else {
            return (0, full_width, false, false);
        };

        let base_shift = if self.no_hscroll {
            0
        } else if match_start_char == 0 && match_end_char == 0 {
            let skip_width = self.calc_skip_width(text);
            if skip_width > 0 {
                skip_width
            } else if self.keep_right {
                full_width.saturating_sub(available_width)
            } else {
                0
            }
        } else {
            let mut match_start_width = 0;
            let mut match_end_width = 0;
            let mut current_width = 0;
            let mut found_start = false;
            let mut found_end = false;

            for (idx, ch) in text.chars().enumerate() {
                if idx == match_start_char {
                    match_start_width = current_width;
                    found_start = true;
                }
                if idx == match_end_char {
                    match_end_width = current_width;
                    found_end = true;
                    break;
                }
                if ch == '\t' {
                    current_width += self.tabstop - (current_width % self.tabstop);
                } else {
                    current_width += char_display_width(ch);
                }
            }
            if found_start && !found_end {
                match_end_width = current_width;
            }

            let match_width = match_end_width.saturating_sub(match_start_width);
            if match_width >= available_width {
                match_start_width
            } else {
                let desired = match_start_width.saturating_sub((available_width - match_width) / 2);
                desired.min(full_width.saturating_sub(available_width))
            }
        };

        let proposed = (i32::try_from(base_shift).unwrap_or(i32::MAX) + self.manual_hscroll)
            .max(0)
            .unsigned_abs() as usize;
        let shift = if full_width > available_width {
            proposed.min(full_width.saturating_sub(available_width))
        } else {
            proposed
        };

        (shift, full_width, shift > 0, shift + available_width < full_width)
    }

    fn text_display_width(&self, text: &str) -> usize {
        text.chars().fold(0usize, |width, ch| self.add_char_width(width, ch))
    }

    fn line_display_width(&self, line: &Line<'_>) -> usize {
        line.spans.iter().fold(0usize, |width, span| {
            span.content
                .chars()
                .fold(width, |span_width, ch| self.add_char_width(span_width, ch))
        })
    }

    fn add_char_width(&self, width: usize, ch: char) -> usize {
        if ch == '\t' {
            width + self.tabstop - (width % self.tabstop)
        } else {
            width + char_display_width(ch)
        }
    }

    fn apply_hscroll<'b>(&'b self, line: Line<'b>, shift: usize, full_width: usize) -> Line<'b> {
        let container_width = self.container_width;
        let has_left = shift > 0;
        let has_right = shift + container_width < full_width;

        let ell_w = usize::try_from(display_width(self.ellipsis)).unwrap();
        let left_w = if has_left { ell_w } else { 0 };
        let right_w = if has_right { ell_w } else { 0 };
        let content_width = container_width.saturating_sub(left_w + right_w);

        let mut result = Line::default();
        if has_left {
            result.push_span(Span::raw(self.ellipsis));
        }

        let mut current_char_index = 0;
        let mut current_width = 0;
        let shift_char_start = self.char_index_at_width(&line, shift);
        let shift_char_end = self.char_index_at_width(&line, shift + content_width);

        for span in line.spans {
            let span_text = span.content.as_ref();
            let span_chars: Vec<char> = span_text.chars().collect();
            let span_start = current_char_index;
            let span_end = current_char_index + span_chars.len();

            if span_end > shift_char_start && span_start < shift_char_end {
                let vis_start = shift_char_start.saturating_sub(span_start);
                let vis_end = if span_end > shift_char_end {
                    shift_char_end - span_start
                } else {
                    span_chars.len()
                };
                if vis_start < vis_end && vis_start < span_chars.len() {
                    let visible: String = span_chars[vis_start..vis_end.min(span_chars.len())].iter().collect();
                    let processed = if visible.contains('\t') {
                        self.expand_tabs(&visible, current_width)
                    } else {
                        visible
                    };
                    if !processed.is_empty() {
                        result.push_span(Span::styled(processed, span.style));
                    }
                }
            }
            current_char_index += span_chars.len();
            current_width += usize::try_from(display_width(span_text)).unwrap();
        }

        if has_right {
            result.push_span(Span::raw(self.ellipsis));
        }
        result
    }

    fn char_index_at_width(&self, line: &Line<'_>, target_width: usize) -> usize {
        let mut current_width = 0;
        let mut char_index = 0;
        for span in &line.spans {
            for ch in span.content.chars() {
                let ch_width = if ch == '\t' {
                    self.tabstop - (current_width % self.tabstop)
                } else {
                    char_display_width(ch)
                };
                if current_width >= target_width {
                    return char_index;
                }
                current_width += ch_width;
                char_index += 1;
            }
        }
        char_index
    }

    fn expand_tabs(&self, text: &str, start_width: usize) -> String {
        let mut result = String::new();
        let mut current_width = start_width;
        for ch in text.chars() {
            if ch == '\t' {
                let tab_width = self.tabstop - (current_width % self.tabstop);
                result.push_str(&" ".repeat(tab_width));
                current_width += tab_width;
            } else {
                result.push(ch);
                current_width += char_display_width(ch);
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::sync::Arc;

    use ratatui::style::Style;

    use super::*;
    use crate::item::RankBuilder;
    use crate::{Rank, SkimItem};

    fn renderer(theme: &ColorTheme) -> ItemRenderer<'_> {
        ItemRenderer {
            theme,
            selector_icon: ">",
            multi_select_icon: "*",
            ellipsis: "..",
            container_width: 6,
            wrap: false,
            multiline: None,
            show_score: false,
            show_index: false,
            multi_select: false,
            tabstop: 4,
            no_hscroll: false,
            keep_right: false,
            manual_hscroll: 0,
            skip_to_pattern: None,
            reverse_sub_lines: false,
            highlight_line: false,
        }
    }

    fn matched_item(text: &str, matched_range: Option<MatchRange>) -> MatchedItem {
        MatchedItem::new(
            Arc::new(text.to_owned()) as Arc<dyn SkimItem>,
            Rank::default(),
            matched_range,
            &RankBuilder::default(),
        )
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|span| span.content.as_ref()).collect()
    }

    fn spans_text(spans: &[Span<'_>]) -> String {
        spans.iter().map(|span| span.content.as_ref()).collect()
    }

    struct DisplayLongerThanText;

    impl SkimItem for DisplayLongerThanText {
        fn text(&self) -> Cow<'_, str> {
            Cow::Borrowed("ab")
        }

        fn display(&self, _context: DisplayContext) -> Line<'_> {
            Line::from("abcdefghi")
        }
    }

    #[test]
    fn split_sub_lines_uses_configured_separator() {
        let theme = ColorTheme::default();
        let mut renderer = renderer(&theme);

        assert_eq!(renderer.split_sub_lines("alpha|beta"), vec!["alpha|beta"]);

        renderer.multiline = Some("|");
        assert_eq!(
            renderer.split_sub_lines("alpha|beta|gamma"),
            vec!["alpha", "beta", "gamma"]
        );
    }

    #[test]
    fn matched_range_as_char_range_normalizes_supported_ranges() {
        assert_eq!(
            ItemRenderer::matched_range("aébc", Some(&MatchRange::ByteRange(1, 3))),
            (1, 2)
        );
        assert_eq!(
            ItemRenderer::matched_range("abcdef", Some(&MatchRange::Chars(vec![1, 3, 4]))),
            (1, 5)
        );
        assert_eq!(
            ItemRenderer::matched_range("abcdef", Some(&MatchRange::Chars(vec![]))),
            (0, 0)
        );
        assert_eq!(
            ItemRenderer::matched_range("abcdef", Some(&MatchRange::CharRange(2, 4))),
            (2, 4)
        );
        assert_eq!(ItemRenderer::matched_range("abcdef", None), (0, 0));
    }

    #[test]
    fn display_matches_preserves_original_match_kind() {
        assert!(matches!(
            ItemRenderer::display_matches(Some(&MatchRange::ByteRange(1, 3))),
            crate::Matches::ByteRange(1, 3)
        ));
        assert!(matches!(
            ItemRenderer::display_matches(Some(&MatchRange::CharRange(2, 4))),
            crate::Matches::CharRange(2, 4)
        ));
        assert!(matches!(
            ItemRenderer::display_matches(Some(&MatchRange::Chars(vec![0, 2]))),
            crate::Matches::CharIndices(indices) if indices == vec![0, 2]
        ));
        assert!(matches!(ItemRenderer::display_matches(None), crate::Matches::None));
    }

    #[test]
    fn prefix_spans_uses_state_for_icons_and_first_line_fields() {
        let theme = ColorTheme::default();
        let mut renderer = renderer(&theme);
        renderer.multi_select = true;
        renderer.show_score = true;
        renderer.show_index = true;

        let mut item = matched_item("alpha", None);
        item.rank.score = 42;
        item.rank.index = 7;

        let current_selected = SubLineState {
            is_current: true,
            is_selected: true,
            is_first: true,
            needs_ellipsis: false,
        };
        let continuation = SubLineState {
            is_current: true,
            is_selected: true,
            is_first: false,
            needs_ellipsis: false,
        };

        assert_eq!(
            spans_text(&renderer.prefix_spans(&item, &current_selected)),
            ">*[42] [7] "
        );
        assert_eq!(spans_text(&renderer.prefix_spans(&item, &continuation)), "           ");
    }

    #[test]
    fn trim_with_ellipsis_reserves_space_for_marker() {
        let theme = ColorTheme::default();
        let renderer = renderer(&theme);
        let line = Line::from(vec![
            Span::styled("abc", Style::default()),
            Span::styled("def", Style::default()),
        ]);

        let trimmed = renderer.trim_with_ellipsis(line, false);

        assert_eq!(spans_text(&trimmed), "abcd..");
    }

    #[test]
    fn continuation_sub_line_content_applies_hscroll() {
        let theme = ColorTheme::default();
        let mut renderer = renderer(&theme);
        renderer.manual_hscroll = 2;

        let line = renderer.continuation_sub_line_content("abcdefgh", false);

        assert_eq!(line_text(&line), "..cdef");
    }

    #[test]
    fn first_sub_line_content_keeps_display_longer_than_text() {
        let theme = ColorTheme::default();
        let renderer = renderer(&theme);
        let item = MatchedItem::new(
            Arc::new(DisplayLongerThanText) as Arc<dyn SkimItem>,
            Rank::default(),
            None,
            &RankBuilder::default(),
        );

        let line = renderer.first_sub_line_content(&item, "ab", false, 0, 0);

        assert_eq!(line_text(&line), "abcd..");
    }
}
