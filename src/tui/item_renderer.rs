use ratatui::text::{Line, Span};
use ratatui::widgets::{ListDirection, ListItem};
use unicode_display_width::width as display_width;

use crate::tui::item_list::ItemList;
use crate::tui::util::{char_display_width, wrap_text};
use crate::{DisplayContext, MatchRange, item::MatchedItem, theme::ColorTheme};

/// Rendering parameters that are constant across all items in one render pass.
pub(crate) struct ItemRenderer<'a> {
    pub theme: &'a ColorTheme,
    pub selector_icon: &'a str,
    pub multi_select_icon: &'a str,
    pub ellipsis: &'a str,
    pub container_width: usize,
    pub wrap: bool,
    pub multiline: Option<&'a str>,
    pub print_score: bool,
    pub multi_select: bool,
    pub tabstop: usize,
    pub no_hscroll: bool,
    pub keep_right: bool,
    pub manual_hscroll: i32,
    pub skip_to_pattern: Option<&'a regex::Regex>,
    /// When true, reverse the order of sub-lines within each multiline item
    /// before appending to the output. Required for BottomToTop list direction
    /// so that sub-line 0 appears visually above sub-line 1.
    pub reverse_sub_lines: bool,
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
            print_score: list.print_score,
            multi_select: list.multi_select,
            tabstop: list.tabstop,
            no_hscroll: list.no_hscroll,
            keep_right: list.keep_right,
            manual_hscroll: list.manual_hscroll,
            skip_to_pattern: list.skip_to_pattern.as_ref(),
            reverse_sub_lines: list.direction == ListDirection::BottomToTop,
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

        let sub_lines: Vec<&str> = if let Some(sep) = self.multiline {
            item_text.split(sep).collect()
        } else {
            vec![item_text.as_ref()]
        };

        // Match positions for hscroll on the first sub-line.
        let (match_start_char, match_end_char) = match &item.matched_range {
            Some(MatchRange::Chars(indices)) => {
                if !indices.is_empty() {
                    (indices[0], indices[indices.len() - 1] + 1)
                } else {
                    (0, 0)
                }
            }
            Some(MatchRange::ByteRange(start, end)) => {
                let msc = item_text[..*start].chars().count();
                let diff = item_text[*start..*end].chars().count();
                (msc, msc + diff)
            }
            None => (0, 0),
        };

        let mut added = 0usize;
        // Collect rows for this item into a temporary buffer so we can reverse
        // them when rendering in BottomToTop direction.
        let mut item_rows: Vec<ListItem<'static>> = Vec::new();

        for (sub_idx, sub_text) in sub_lines.iter().enumerate().skip(skip_subs) {
            if rows_used + added >= available_rows {
                break;
            }

            let is_first = sub_idx == skip_subs;

            // Prefix: cursor icon + selection icon [+ score].
            let mut prefix: Vec<Span<'static>> = Vec::with_capacity(3);
            prefix.push(Span::styled(
                if is_first && is_current {
                    self.selector_icon.to_owned()
                } else {
                    str::repeat(" ", self.selector_icon.chars().count())
                },
                self.theme.cursor,
            ));
            prefix.push(Span::styled(
                if is_first && self.multi_select && is_selected {
                    self.multi_select_icon.to_owned()
                } else {
                    str::repeat(" ", self.multi_select_icon.chars().count())
                },
                self.theme.selected,
            ));
            if self.print_score {
                let score_str = format!("[{}] ", item.rank.score);
                prefix.push(Span::styled(
                    if is_first {
                        score_str.clone()
                    } else {
                        str::repeat(" ", score_str.chars().count())
                    },
                    if is_current {
                        self.theme.current
                    } else {
                        self.theme.normal
                    },
                ));
            }

            // Content spans for this sub-line.
            let content_line: Line<'static> = if is_first {
                let first_sub_char_len = sub_lines[0].chars().count();
                let (shift, full_width, _, _) = self.calc_hscroll(sub_lines[0], match_start_char, match_end_char);

                let matches = match &item.matched_range {
                    Some(MatchRange::ByteRange(start, end)) => crate::Matches::ByteRange(*start, *end),
                    Some(MatchRange::Chars(chars)) => crate::Matches::CharIndices(chars.clone()),
                    None => crate::Matches::None,
                };
                let dl = item.item.display(DisplayContext {
                    score: item.rank.score,
                    matches,
                    container_width: self.container_width,
                    base_style: if is_current {
                        self.theme.current
                    } else {
                        self.theme.normal
                    },
                    matched_syle: if is_current {
                        self.theme.current_match
                    } else {
                        self.theme.matched
                    },
                });

                // Clip to first sub-line's chars only.
                let mut clipped: Vec<Span<'static>> = Vec::new();
                let mut chars_seen = 0usize;
                for span in dl.spans {
                    if chars_seen >= first_sub_char_len {
                        break;
                    }
                    let span_chars: Vec<char> = span.content.chars().collect();
                    let take = (first_sub_char_len - chars_seen).min(span_chars.len());
                    let text: String = span_chars[..take].iter().collect();
                    if !text.is_empty() {
                        clipped.push(Span::styled(text, span.style));
                    }
                    chars_seen += span_chars.len();
                }

                let mut line = Line::from(clipped);
                if !self.wrap {
                    line = self.apply_hscroll(line, shift, full_width);
                }
                line.spans
                    .into_iter()
                    .map(|s| Span::styled(s.content.into_owned(), s.style))
                    .collect()
            } else {
                let sub_str = sub_text.to_string();
                let (shift, full_width, _, _) = self.calc_hscroll(&sub_str, 0, 0);
                let base_style = if is_current {
                    self.theme.current
                } else {
                    self.theme.normal
                };
                let raw: Line<'static> = Line::from(vec![Span::styled(sub_str, base_style)]);
                if self.wrap {
                    raw
                } else {
                    let scrolled = self.apply_hscroll(raw, shift, full_width);
                    scrolled
                        .spans
                        .into_iter()
                        .map(|s| Span::styled(s.content.into_owned(), s.style))
                        .collect()
                }
            };

            // Ellipsis for partially-visible sub-lines.
            let is_top_cutoff = is_first && skip_subs > 0;
            let is_bottom_cutoff = rows_used + added + 1 >= available_rows && sub_idx + 1 < sub_lines.len();
            let needs_ellipsis = is_top_cutoff || is_bottom_cutoff;

            let mut all_spans: Vec<Span<'static>> = prefix;
            if needs_ellipsis {
                let ell_width = display_width(self.ellipsis) as usize;
                let base_style = if is_current {
                    self.theme.current
                } else {
                    self.theme.normal
                };
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
                trimmed.push(Span::styled(self.ellipsis.to_owned(), base_style));
                all_spans.extend(trimmed);
            } else {
                all_spans.extend(content_line.spans);
            }

            let list_item: ListItem<'static> = if self.wrap {
                wrap_text(
                    ratatui::text::Text::from(Line::from(all_spans)),
                    self.container_width + self.selector_icon.chars().count() + self.multi_select_icon.chars().count(),
                )
                .into()
            } else {
                Line::from(all_spans).into()
            };

            item_rows.push(list_item);
            added += 1;
        }

        if self.reverse_sub_lines {
            item_rows.reverse();
        }
        out.extend(item_rows);

        added
    }

    // ── hscroll helpers ──────────────────────────────────────────────────────

    fn calc_skip_width(&self, text: &str) -> usize {
        if let Some(regex) = self.skip_to_pattern
            && let Some(mat) = regex.find(text)
        {
            return display_width(&text[..mat.start()]) as usize;
        }
        0
    }

    fn calc_hscroll(&self, text: &str, match_start_char: usize, match_end_char: usize) -> (usize, usize, bool, bool) {
        let full_width = text.chars().fold(0usize, |acc, ch| {
            if ch == '\t' {
                acc + self.tabstop - (acc % self.tabstop)
            } else {
                acc + char_display_width(ch)
            }
        });

        let ell_w = display_width(self.ellipsis) as usize;
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

        let proposed = (base_shift as i32 + self.manual_hscroll).max(0) as usize;
        let shift = if full_width > available_width {
            proposed.min(full_width.saturating_sub(available_width))
        } else {
            proposed
        };

        (shift, full_width, shift > 0, shift + available_width < full_width)
    }

    fn apply_hscroll<'b>(&'b self, line: Line<'b>, shift: usize, full_width: usize) -> Line<'b> {
        let container_width = self.container_width;
        let has_left = shift > 0;
        let has_right = shift + container_width < full_width;

        let ell_w = display_width(self.ellipsis) as usize;
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
            current_width += display_width(span_text) as usize;
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
