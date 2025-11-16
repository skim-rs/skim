use std::{collections::HashSet, rc::Rc, sync::Arc};

use ratatui::widgets::{List, ListDirection, ListState, StatefulWidget};
use ratatui::{
    style::Modifier,
    text::{Line, Span},
};
use regex::Regex;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use unicode_width::UnicodeWidthStr;

use crate::{
    DisplayContext, MatchRange, Selector, SkimItem, SkimOptions,
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
    selector: Option<Rc<dyn Selector>>,
    pre_select_target: usize, // How many items we want to pre-select
    no_clear_if_empty: bool,
    interactive: bool,         // Whether we're in interactive mode
    showing_stale_items: bool, // True when displaying old items due to no_clear_if_empty
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
            selector: None,
            pre_select_target: 0,
            no_clear_if_empty: false,
            interactive: false,
            showing_stale_items: false,
        }
    }
}

impl ItemList {
    fn cursor(&self) -> usize {
        trace!("{:?}", self.selection);
        self.current
    }

    /// Returns the count of items for status display
    /// This may differ from items.len() when no_clear_if_empty is active and showing stale items
    pub fn count(&self) -> usize {
        if self.showing_stale_items { 0 } else { self.items.len() }
    }

    pub fn selected(&self) -> Option<Arc<dyn SkimItem>> {
        self.items.get(self.cursor()).map(|x| x.item.clone())
    }

    pub fn append(&mut self, items: &mut Vec<MatchedItem>) {
        self.items.append(items);
        self.showing_stale_items = false;
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
    /// Also expands tabs to spaces according to tabstop setting
    fn apply_hscroll<'a>(&self, line: Line<'a>, shift: usize, container_width: usize, full_width: usize) -> Line<'a> {
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
    pub fn clear(&mut self) {
        self.items.clear();
        self.selection.clear();
        self.current = 0;
        self.offset = 0;
        self.showing_stale_items = false;
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
        use crate::helper::selector::DefaultSkimSelector;
        use crate::util::read_file_lines;

        let skip_to_pattern = options
            .skip_to_pattern
            .as_ref()
            .and_then(|pattern| Regex::new(pattern).ok());

        // Build the selector from options and calculate pre-select target
        let (selector, pre_select_target) = if options.pre_select_n > 0
            || !options.pre_select_pat.is_empty()
            || !options.pre_select_items.is_empty()
            || options.pre_select_file.is_some()
            || options.selector.is_some()
        {
            match options.selector.clone() {
                Some(s) => {
                    // For custom selectors, use a very large target (pre-select all matching)
                    (Some(s), usize::MAX)
                }
                None => {
                    let mut preset_items: Vec<String> = options
                        .pre_select_items
                        .split('\n')
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .collect();

                    if let Some(ref pre_select_file) = options.pre_select_file {
                        if let Ok(file_items) = read_file_lines(pre_select_file) {
                            preset_items.extend(file_items);
                        }
                    }

                    let selector = DefaultSkimSelector::default()
                        .first_n(options.pre_select_n)
                        .regex(&options.pre_select_pat)
                        .preset(preset_items.clone());

                    // Only use a target for --pre-select-n
                    // For pattern/items, the selector always returns the same matches regardless of timing
                    let target = if options.pre_select_n > 0 {
                        options.pre_select_n
                    } else {
                        usize::MAX // No target - keep selecting matching items
                    };

                    (Some(Rc::new(selector) as Rc<dyn Selector>), target)
                }
            }
        } else {
            (None, 0)
        };

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
            selector,
            pre_select_target,
            no_clear_if_empty: options.no_clear_if_empty,
            interactive: options.interactive,
            ..Default::default()
        }
    }

    fn render(&mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) -> SkimRender {
        let this = &mut *self;
        this.height = area.height;
        if this.current < this.offset {
            this.offset = this.current;
        } else if this.offset + area.height as usize <= this.current {
            this.offset = this.current - area.height as usize + 1;
        }
        let items_updated = if let Ok(items) = this.rx.try_recv() {
            debug!("Got {} items to put in list", items.len());

            let items_are_empty_or_blank =
                items.is_empty() || items.iter().all(|item| item.item.text().trim().is_empty());

            if this.interactive && this.no_clear_if_empty && items_are_empty_or_blank && !this.items.is_empty() {
                debug!(
                    "no_clear_if_empty: keeping {} old items for display (new items are empty/blank)",
                    this.items.len()
                );
                // Set flag to show 0 count in status, but keep old items for display
                this.showing_stale_items = true;
                true
            } else {
                this.items = items;
                this.showing_stale_items = false;

                // Apply pre-selection BEFORE sorting, while indices are still meaningful
                // Only pre-select if we haven't reached our target yet
                if this.multi_select && this.selector.is_some() && this.selection.len() < this.pre_select_target {
                    debug!(
                        "Applying pre-selection to {} items (currently {} selected, target {})",
                        this.items.len(),
                        this.selection.len(),
                        this.pre_select_target
                    );
                    for (index, item) in this.items.iter().enumerate() {
                        // Stop if we've reached our target
                        if this.selection.len() >= this.pre_select_target {
                            break;
                        }

                        let item_text = item.item.text();
                        let should_select = this.selector.as_ref().unwrap().should_select(index, item.item.as_ref());
                        debug!("Item[{}]: '{}' -> {}", index, item_text, should_select);
                        if should_select {
                            this.selection.insert(item.clone());
                        }
                    }
                    debug!(
                        "Pre-selected {} items: {:?}",
                        this.selection.len(),
                        this.selection.iter().map(|i| i.item.text()).collect::<Vec<_>>()
                    );
                }

                // Sort AFTER pre-selection so we select the correct items by index
                this.items.sort_by_key(|item| item.rank);

                true
            }
        } else {
            false
        };

        if this.items.is_empty() {
            return SkimRender { items_updated };
        }

        let theme = &this.theme;

        let list = List::new(
            this.items
                .iter()
                .enumerate()
                .skip(this.offset)
                .take(area.height as usize)
                .map(|(idx, item)| {
                    let is_current = idx == this.current;
                    let is_selected = this.selection.contains(item);

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
                        this.calc_hscroll(&item_text, container_width, match_start_char, match_end_char);

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
                    display_line = this.apply_hscroll(display_line, shift, container_width, full_width);

                    // Prepend cursor indicators
                    let mut spans: Vec<Span> = vec![
                        if is_current {
                            Span::styled(">", theme.selected().add_modifier(Modifier::BOLD))
                        } else {
                            Span::raw(" ")
                        },
                        if this.multi_select && is_selected {
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
        .direction(this.direction);

        StatefulWidget::render(
            list,
            area,
            buf,
            &mut ListState::default().with_selected(Some(this.current.saturating_sub(this.offset))),
        );
        SkimRender { items_updated }
    }
}
