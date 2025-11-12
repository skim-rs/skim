use std::{collections::HashSet, sync::Arc};

use ratatui::{
    style::Modifier,
    text::{Line, Span},
};
use ratatui::widgets::{List, ListDirection, ListState, StatefulWidget};
use regex::Regex;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use unicode_width::UnicodeWidthStr;

use crate::{
    DisplayContext, MatchRange, SkimItem, SkimOptions,
    item::{MatchedItem, RankBuilder},
    theme::ColorTheme,
    tui::options::TuiLayout,
    tui::widget::{SkimRender, SkimWidget},
};

pub struct ItemList {
    pub(crate) items: Vec<MatchedItem>,
    pub(crate) selection: HashSet<MatchedItem>,
    pub(crate) tx: UnboundedSender<Vec<MatchedItem>>,
    rank_builder: RankBuilder,
    rx: UnboundedReceiver<Vec<MatchedItem>>,
    pub(crate) direction: ListDirection,
    pub(crate) offset: usize,
    pub(crate) current: usize,
    pub(crate) height: u16,
    pub(crate) theme: std::sync::Arc<crate::theme::ColorTheme>,
    pub(crate) multi_select: bool,
    reserved: usize,
    no_hscroll: bool,
    keep_right: bool,
    skip_to_pattern: Option<Regex>,
    tabstop: usize,
}

impl Default for ItemList {
    fn default() -> Self {
        let (tx, rx) = unbounded_channel();
        Self {
            tx,
            rx,
            direction: ListDirection::BottomToTop,
            items: Default::default(),
            selection: Default::default(),
            rank_builder: Default::default(),
            offset: Default::default(),
            current: Default::default(),
            height: Default::default(),
            theme: Arc::new(ColorTheme::default()),
            multi_select: false,
            reserved: 0,
            no_hscroll: false,
            keep_right: false,
            skip_to_pattern: None,
            tabstop: 8,
        }
    }
}

impl ItemList {
    fn cursor(&self) -> usize {
        trace!("{:?}", self.selection);
        self.current
    }
    pub fn selected(&self) -> Option<Arc<dyn SkimItem>> {
        self.items.get(self.cursor()).map(|x| x.item.clone())
    }

    /// Calculate the width to skip when using skip_to_pattern
    /// Returns the actual skip width (not accounting for ".." - that's handled in apply_hscroll)
    fn calc_skip_width(&self, text: &str) -> usize {
        if let Some(ref regex) = self.skip_to_pattern {
            if let Some(mat) = regex.find(text) {
                return text[..mat.start()].width_cjk();
            }
        }
        0
    }

    /// Calculate horizontal scroll offset for displaying a line with matches
    /// Returns (shift, full_width, has_left_overflow, has_right_overflow)
    fn calc_hscroll(
        &self,
        text: &str,
        container_width: usize,
        match_start_char: usize,
        match_end_char: usize,
    ) -> (usize, usize, bool, bool) {
        // Calculate display width considering tab expansion
        let full_width = text.chars().fold(0, |acc, ch| {
            if ch == '\t' {
                acc + self.tabstop - (acc % self.tabstop)
            } else {
                acc + ch.to_string().width_cjk()
            }
        });

        // Reserve 2 chars for ".." indicators
        let available_width = if container_width >= 2 {
            container_width
        } else {
            return (0, full_width, false, false);
        };

        let shift = if self.no_hscroll {
            // No horizontal scroll: always start from beginning
            0
        } else if match_start_char == 0 && match_end_char == 0 {
            // No match to center on (empty query or no matches)
            let skip_width = self.calc_skip_width(text);
            if skip_width > 0 {
                // skip_to_pattern is set and found a match
                skip_width
            } else if self.keep_right {
                // Show the right end
                full_width.saturating_sub(available_width)
            } else {
                // Start from beginning
                0
            }
        } else {
            // Calculate shift to show the match
            // Calculate display widths for match positions
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
                    current_width += ch.to_string().width_cjk();
                }
            }
            
            // If we didn't find the end, use the current width
            if found_start && !found_end {
                match_end_width = current_width;
            }
            
            let match_width = match_end_width.saturating_sub(match_start_width);
            
            // Try to center the match, but ensure we show as much of it as possible
            if match_width >= available_width {
                // Match itself is too long, show from start of match
                match_start_width
            } else {
                // Center the match in the available space
                let desired_shift = match_start_width.saturating_sub((available_width - match_width) / 2);
                // But don't shift more than necessary
                let max_shift = full_width.saturating_sub(available_width);
                desired_shift.min(max_shift)
            }
        };

        let has_left_overflow = shift > 0;
        let has_right_overflow = shift + available_width < full_width;

        (shift, full_width, has_left_overflow, has_right_overflow)
    }

    /// Apply horizontal scrolling to a line, adding ".." indicators as needed
    fn apply_hscroll<'a>(&self, line: Line<'a>, shift: usize, container_width: usize, full_width: usize) -> Line<'a> {
        // If no shift and text fits, return as-is
        if shift == 0 && full_width <= container_width {
            return line;
        }

        let has_left_overflow = shift > 0;
        let has_right_overflow = shift + container_width < full_width;

        // Reserve space for overflow indicators
        let left_indicator_width = if has_left_overflow { 2 } else { 0 };
        let right_indicator_width = if has_right_overflow { 2 } else { 0 };
        let content_width = container_width.saturating_sub(left_indicator_width + right_indicator_width);

        // Extract the visible portion of the line
        let mut result = Line::default();
        
        // Add left indicator if needed
        if has_left_overflow {
            result.push_span(Span::raw(".."));
        }

        // Calculate which part of the text to show
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        let mut current_width = 0;
        let mut visible_chars = String::new();
        
        for ch in text.chars() {
            let ch_width = if ch == '\t' {
                self.tabstop - (current_width % self.tabstop)
            } else {
                ch.to_string().width_cjk()
            };
            
            if current_width >= shift && current_width + ch_width <= shift + content_width {
                if ch == '\t' {
                    visible_chars.push_str(&" ".repeat(ch_width));
                } else {
                    visible_chars.push(ch);
                }
            }
            
            current_width += ch_width;
            
            if current_width >= shift + content_width {
                break;
            }
        }
        
        result.push_span(Span::raw(visible_chars));

        // Add right indicator if needed
        if has_right_overflow {
            result.push_span(Span::raw(".."));
        }

        result
    }

    /// Render the item list using the theme colors.
    pub fn render_with_theme(&mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) -> bool {
        self.height = area.height;
        if self.current < self.offset {
            self.offset = self.current;
        } else if self.offset + area.height as usize <= self.current {
            self.offset = self.current - area.height as usize + 1;
        }
        let items_updated = if let Ok(items) = self.rx.try_recv() {
            debug!("Got {} items to put in list", items.len());
            self.items = items;
            self.items.sort_by_key(|item| item.rank);
            true
        } else {
            false
        };

        if self.items.is_empty() {
            return items_updated;
        }

        let theme = &self.theme;

        let list = List::new(
            self.items
                .iter()
                .enumerate()
                .skip(self.offset)
                .take(area.height as usize)
                .map(|(idx, item)| {
                    let is_current = idx == self.current;
                    let is_selected = self.selection.contains(item);

                    // Reserve 2 characters for cursor indicators ("> " or " >")
                    let container_width = (area.width as usize).saturating_sub(2);

                    // Get item text for hscroll calculation
                    let item_text = item.item.text();
                    
                    // Calculate match positions for hscroll
                    let (match_start_char, match_end_char) = match &item.matched_range {
                        Some(MatchRange::Chars(matched_indices)) => {
                            if !matched_indices.is_empty() {
                                (matched_indices[0], matched_indices[matched_indices.len() - 1] + 1)
                            } else {
                                (0, 0)
                            }
                        }
                        Some(MatchRange::ByteRange(match_start, match_end)) => {
                            let match_start_char = item_text[..*match_start].chars().count();
                            let diff = item_text[*match_start..*match_end].chars().count();
                            (match_start_char, match_start_char + diff)
                        }
                        None => (0, 0),
                    };

                    // Calculate horizontal scroll
                    let (shift, full_width, _has_left, _has_right) = 
                        self.calc_hscroll(&item_text, container_width, match_start_char, match_end_char);

                    // Get display content from item
                    let mut display_line = item.item.display(DisplayContext {
                        score: item.rank[0],
                        matches: match &item.matched_range {
                            Some(MatchRange::ByteRange(start, end)) => crate::Matches::ByteRange(*start, *end),
                            Some(MatchRange::Chars(chars)) => crate::Matches::CharIndices(chars.clone()),
                            None => crate::Matches::None,
                        },
                        container_width,
                        style: if is_current { theme.current() } else { theme.normal() },
                    });

                    // Apply horizontal scrolling to the display content
                    display_line = self.apply_hscroll(display_line, shift, container_width, full_width);

                    // Prepend cursor indicators
                    let mut spans: Vec<Span> = vec![
                        if is_current {
                            Span::styled(">", theme.selected().add_modifier(Modifier::BOLD))
                        } else {
                            Span::raw(" ")
                        },
                        if self.multi_select && is_selected {
                            Span::raw(">")
                        } else {
                            Span::raw(" ")
                        },
                    ];
                    spans.extend(display_line.spans);
                    
                    Line::from(spans)
                })
                .collect::<Vec<Line>>(),
        )
        .direction(self.direction);

        StatefulWidget::render(
            list,
            area,
            buf,
            &mut ListState::default().with_selected(Some(self.current.saturating_sub(self.offset))),
        );

        items_updated
    }
    pub fn toggle_item(&mut self, item: &MatchedItem) {
        if self.selection.contains(item) {
            self.selection.remove(item);
        } else {
            self.selection.insert(item.clone());
        }
    }

    pub fn toggle_at(&mut self, index: usize) {
        let item = self.items[index].clone();
        trace!("Toggled item {} at index {}", item.text(), index);
        self.toggle_item(&item);
        trace!(
            "Selection is now {:#?}",
            self.selection.iter().map(|item| item.item.text()).collect::<Vec<_>>()
        );
    }
    pub fn toggle(&mut self) {
        self.toggle_at(self.cursor());
    }
    pub fn toggle_all(&mut self) {
        for item in self.items.clone() {
            self.toggle_item(&item);
        }
    }

    /// Add row at cursor to selection
    pub fn select(&mut self) {
        debug!("{}", self.cursor());
        self.select_row(self.cursor())
    }

    /// Add row to selection
    pub fn select_row(&mut self, index: usize) {
        let item = self.items[index].clone();
        self.selection.insert(item);
    }
    pub fn select_all(&mut self) {
        for item in self.items.clone() {
            self.selection.insert(item.clone());
        }
    }
    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }
    pub fn scroll_by(&mut self, offset: i32) {
        self.current = self
            .current
            .saturating_add_signed(offset as isize)
            .min(self.items.len() - 1)
            .max(self.reserved);
        debug!("Scrolled to {}", self.current);
        debug!("Selection: {:?}", self.selection);
    }
    pub fn select_previous(&mut self) {
        self.current = self
            .current
            .saturating_sub(1)
            .min(self.items.len() - 1)
            .max(self.reserved);
    }
    pub fn select_next(&mut self) {
        self.current = self
            .current
            .saturating_add(1)
            .min(self.items.len() - 1)
            .max(self.reserved);
    }
}

impl SkimWidget for ItemList {
    fn from_options(options: &SkimOptions, theme: Arc<ColorTheme>) -> Self {
        let skip_to_pattern = options.skip_to_pattern.as_ref().and_then(|pattern| {
            Regex::new(pattern).ok()
        });

        Self {
            reserved: options.header_lines,
            direction: match options.layout {
                TuiLayout::Default => ratatui::widgets::ListDirection::BottomToTop,
                TuiLayout::Reverse | TuiLayout::ReverseList => ratatui::widgets::ListDirection::TopToBottom,
            },
            current: options.header_lines,
            theme,
            multi_select: options.multi,
            no_hscroll: options.no_hscroll,
            keep_right: options.keep_right,
            skip_to_pattern,
            tabstop: options.tabstop.max(1),
            ..Default::default()
        }
    }

    fn render(&mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) -> SkimRender {
        let items_updated = self.render_with_theme(area, buf);
        SkimRender { items_updated }
    }
}
