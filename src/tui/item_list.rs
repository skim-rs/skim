use std::{rc::Rc, sync::Arc};

use indexmap::IndexSet;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListDirection, ListItem, ListState, StatefulWidget, Widget};
use regex::Regex;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    DisplayContext, MatchRange, Selector, SkimItem, SkimOptions,
    item::MatchedItem,
    spinlock::SpinLock,
    theme::ColorTheme,
    tui::options::TuiLayout,
    tui::util::wrap_text,
    tui::widget::{SkimRender, SkimWidget},
};

/// Processed items ready for rendering
struct ProcessedItems {
    items: Vec<MatchedItem>,
}

/// Widget for displaying and managing the list of filtered items
pub struct ItemList {
    pub(crate) items: Vec<MatchedItem>,
    pub(crate) selection: IndexSet<MatchedItem>,
    pub(crate) tx: UnboundedSender<Vec<MatchedItem>>,
    processed_items: Arc<SpinLock<Option<ProcessedItems>>>,
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
    interactive: bool,              // Whether we're in interactive mode
    showing_stale_items: bool,      // True when displaying old items due to no_clear_if_empty
    pub(crate) manual_hscroll: i32, // Manual horizontal scroll offset for ScrollLeft/ScrollRight
    selector_icon: String,
    multi_select_icon: String,
    cycle: bool,
    wrap: bool,
}

impl Default for ItemList {
    fn default() -> Self {
        let (tx, _rx) = unbounded_channel();
        let processed_items = Arc::new(SpinLock::new(None));

        Self {
            tx,
            processed_items,
            direction: ListDirection::BottomToTop,
            items: Default::default(),
            selection: Default::default(),
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
            manual_hscroll: 0,
            selector_icon: String::from(">"),
            multi_select_icon: String::from(">"),
            cycle: false,
            wrap: false,
        }
    }
}

impl ItemList {
    /// Background task that processes incoming items from matcher
    /// Performs expensive operations (sorting) in background to keep render path fast
    fn process_items_task(
        mut rx: UnboundedReceiver<Vec<MatchedItem>>,
        processed_items: Arc<SpinLock<Option<ProcessedItems>>>,
        no_sort: bool,
    ) {
        while let Some(mut items) = rx.blocking_recv() {
            debug!("Background task: Got {} items to process", items.len());

            // Sort items immediately - use stable sort to preserve order for equal ranks
            if !no_sort {
                items.sort_by_key(|item| item.rank);
            }

            // Write processed items to shared state for render thread
            // Move items instead of cloning for efficiency
            let processed = ProcessedItems { items };

            *processed_items.lock() = Some(processed);
        }
        debug!("Background task: rx channel closed, exiting");
    }

    fn cursor(&self) -> usize {
        self.current
    }

    /// Returns the count of items for status display.
    ///
    /// This may differ from items.len() when no_clear_if_empty is active and showing stale items
    pub fn count(&self) -> usize {
        if self.showing_stale_items { 0 } else { self.items.len() }
    }

    /// Returns the currently selected item, if any
    pub fn selected(&self) -> Option<Arc<dyn SkimItem>> {
        self.items.get(self.cursor()).map(|x| x.item.clone())
    }

    /// Appends new matched items to the list
    pub fn append(&mut self, items: &mut Vec<MatchedItem>) {
        self.items.append(items);
        self.showing_stale_items = false;
    }

    /// Calculate the width to skip when using skip_to_pattern
    /// Returns the actual skip width (not accounting for ".." - that's handled in apply_hscroll)
    fn calc_skip_width(&self, text: &str) -> usize {
        if let Some(ref regex) = self.skip_to_pattern
            && let Some(mat) = regex.find(text)
        {
            return text[..mat.start()].width_cjk();
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

        let base_shift = if self.no_hscroll {
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

        // Apply manual horizontal scroll offset
        // manual_hscroll can be positive (scroll right) or negative (scroll left)
        // final_shift = base_shift + manual_hscroll
        let proposed_shift = (base_shift as i32 + self.manual_hscroll).max(0) as usize;

        // Only clamp if the text is actually wider than the container
        // This allows skip_to_pattern to work even for short text
        let shift = if full_width > available_width {
            let max_shift = full_width.saturating_sub(available_width);
            proposed_shift.min(max_shift)
        } else {
            proposed_shift
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

        // Extract the visible portion of the line while preserving styling
        let mut result = Line::default();

        // Add left indicator if needed
        if has_left_overflow {
            result.push_span(Span::raw(".."));
        }

        // Process spans to extract only the visible portion while preserving styles
        let mut current_char_index = 0;
        let mut current_width = 0;
        let shift_char_start = self.char_index_at_width(&line, shift);
        let shift_char_end = self.char_index_at_width(&line, shift + content_width);

        for span in line.spans {
            let span_text = span.content.as_ref();
            let span_chars: Vec<char> = span_text.chars().collect();

            let span_start_char = current_char_index;
            let span_end_char = current_char_index + span_chars.len();

            // Check if this span intersects with our visible range
            if span_end_char > shift_char_start && span_start_char < shift_char_end {
                // Calculate which part of this span is visible
                let visible_start = shift_char_start.saturating_sub(span_start_char);

                let visible_end = if span_end_char > shift_char_end {
                    shift_char_end - span_start_char
                } else {
                    span_chars.len()
                };

                if visible_start < visible_end && visible_start < span_chars.len() {
                    let visible_chars: String = span_chars[visible_start..visible_end.min(span_chars.len())]
                        .iter()
                        .collect();

                    // Expand tabs to spaces and preserve styling
                    let processed_chars = if visible_chars.contains('\t') {
                        self.expand_tabs(&visible_chars, current_width)
                    } else {
                        visible_chars
                    };

                    if !processed_chars.is_empty() {
                        result.push_span(Span::styled(processed_chars, span.style));
                    }
                }
            }

            current_char_index += span_chars.len();
            current_width += span_text.width_cjk();
        }

        // Add right indicator if needed
        if has_right_overflow {
            result.push_span(Span::raw(".."));
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
                    ch.width_cjk().unwrap_or_default()
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
                current_width += ch.to_string().width_cjk();
            }
        }

        result
    }

    /// Toggles the selection state of the item at the given index
    pub fn toggle_at(&mut self, index: usize) {
        if self.items.is_empty() {
            return;
        }
        let item = &self.items[index];
        trace!("Toggled item {} at index {}", item.text(), index);
        toggle_item(&mut self.selection, item);
        trace!(
            "Selection is now {:#?}",
            self.selection.iter().map(|item| item.item.text()).collect::<Vec<_>>()
        );
    }
    /// Toggles the selection state of the currently selected item
    pub fn toggle(&mut self) {
        self.toggle_at(self.cursor());
    }
    /// Toggles the selection state of all items
    pub fn toggle_all(&mut self) {
        for item in &self.items {
            toggle_item(&mut self.selection, item);
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
    /// Selects all items
    pub fn select_all(&mut self) {
        for item in self.items.clone() {
            self.selection.insert(item.clone());
        }
    }
    /// Clears all selections
    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }
    /// Clears all items from the list
    pub fn clear(&mut self) {
        self.items.clear();
        self.selection.clear();
        self.current = 0;
        self.offset = 0;
        self.showing_stale_items = false;
    }
    /// Scrolls the list by the given offset
    pub fn scroll_by(&mut self, offset: i32) {
        if self.reserved >= self.items.len() {
            return;
        }
        let reserved = self.reserved as i32;
        let total = self.items.len() as i32;
        let mut new = self.current as i32 + offset;
        if self.cycle {
            let n = total - reserved;
            new = reserved + (new + n - reserved) % n;
        } else {
            new = new.min(self.items.len() as i32 - 1).max(self.reserved as i32);
        }
        self.current = new.max(0) as usize;
        debug!("Scrolled to {}", self.current);
        debug!("Selection: {:?}", self.selection);
    }
    /// Selects the previous item in the list
    pub fn select_previous(&mut self) {
        self.scroll_by(-1);
    }
    /// Selects the next item in the list
    pub fn select_next(&mut self) {
        self.scroll_by(1);
    }
    /// Jump to the first selectable item (respecting reserved header lines)
    pub fn jump_to_first(&mut self) {
        if self.items.len() > self.reserved {
            self.current = self.reserved;
        }
    }
    /// Jump to the last item in the list
    pub fn jump_to_last(&mut self) {
        if !self.items.is_empty() {
            self.current = self.items.len().saturating_sub(1);
        }
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

                    if let Some(ref pre_select_file) = options.pre_select_file
                        && let Ok(file_items) = read_file_lines(pre_select_file)
                    {
                        preset_items.extend(file_items);
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

        let (tx, rx) = unbounded_channel();
        let processed_items = Arc::new(SpinLock::new(None));

        let interactive = options.interactive;
        let no_clear_if_empty = options.no_clear_if_empty;
        let multi_select = options.multi;

        // Spawn background processing thread with the appropriate configuration
        let processed_items_clone = processed_items.clone();
        let no_sort = options.no_sort;
        std::thread::spawn(move || {
            Self::process_items_task(rx, processed_items_clone, no_sort);
        });

        Self {
            tx,
            processed_items,
            reserved: options.header_lines,
            direction: match options.layout {
                TuiLayout::Default => ratatui::widgets::ListDirection::BottomToTop,
                TuiLayout::Reverse | TuiLayout::ReverseList => ratatui::widgets::ListDirection::TopToBottom,
            },
            current: options.header_lines,
            theme,
            multi_select,
            no_hscroll: options.no_hscroll,
            keep_right: options.keep_right,
            skip_to_pattern,
            tabstop: options.tabstop.max(1),
            selector,
            pre_select_target,
            no_clear_if_empty,
            interactive,
            showing_stale_items: false,
            manual_hscroll: 0,
            items: Default::default(),
            selection: Default::default(),
            offset: Default::default(),
            height: Default::default(),
            selector_icon: options.selector_icon.clone(),
            multi_select_icon: options.multi_select_icon.clone(),
            cycle: options.cycle,
            wrap: options.wrap_items,
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

        // Check for pre-processed items from background thread (non-blocking)
        let items_updated = if let Some(processed) = this.processed_items.lock().take() {
            debug!("Render: Got {} processed items", processed.items.len());

            // Check if items are empty or blank for no_clear_if_empty handling
            let items_are_empty_or_blank =
                processed.items.is_empty() || processed.items.iter().all(|item| item.item.text().trim().is_empty());

            if this.interactive && this.no_clear_if_empty && items_are_empty_or_blank && !this.items.is_empty() {
                debug!(
                    "no_clear_if_empty: keeping {} old items for display (new items are empty/blank)",
                    this.items.len()
                );
                this.showing_stale_items = true;
            } else {
                this.items = processed.items;
                this.showing_stale_items = false;

                // Apply pre-selection only when new items arrive and only if we haven't reached target
                // This runs once per item batch, not on every render
                if this.multi_select
                    && let Some(selector) = &this.selector
                    && this.selection.len() < this.pre_select_target
                {
                    debug!(
                        "Applying pre-selection to {} items (currently {} selected, target {})",
                        this.items.len(),
                        this.selection.len(),
                        this.pre_select_target
                    );
                    for (index, item) in this.items.iter().enumerate() {
                        if this.selection.len() >= this.pre_select_target {
                            break;
                        }
                        let should_select = selector.should_select(index, item.item.as_ref());
                        if should_select {
                            debug!("Pre-selecting item[{}]: '{}'", index, item.item.text());
                            this.selection.insert(item.clone());
                        }
                    }
                    debug!("Pre-selected {} items total", this.selection.len());
                }
            }

            true
        } else {
            false
        };

        if this.items.is_empty() {
            return SkimRender { items_updated };
        }

        let theme = &this.theme;
        let selector_icon = &this.selector_icon;
        let multi_select_icon = &this.multi_select_icon;
        let wrap = &this.wrap;

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
                    let container_width = (area.width as usize)
                        .saturating_sub(selector_icon.chars().count() + multi_select_icon.chars().count());

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
                    // Avoid cloning chars vector - use reference instead
                    let matches = match &item.matched_range {
                        Some(MatchRange::ByteRange(start, end)) => crate::Matches::ByteRange(*start, *end),
                        Some(MatchRange::Chars(chars)) => crate::Matches::CharIndices(chars.clone()),
                        None => crate::Matches::None,
                    };

                    let mut display_line = item.item.display(DisplayContext {
                        score: item.rank[0],
                        matches,
                        container_width,
                        base_style: if is_current { theme.current } else { theme.normal },
                        matched_syle: if is_current { theme.current_match } else { theme.matched },
                    });

                    if !wrap {
                        // Apply horizontal scrolling to the display content
                        display_line = this.apply_hscroll(display_line, shift, container_width, full_width);
                    }

                    // Prepend cursor indicators
                    // Pre-allocate capacity to avoid reallocation
                    let mut spans: Vec<Span> = Vec::with_capacity(2 + display_line.spans.len());
                    spans.push(Span::styled(
                        if is_current {
                            selector_icon.to_owned()
                        } else {
                            str::repeat(" ", selector_icon.chars().count())
                        },
                        theme.cursor,
                    ));
                    spans.push(Span::styled(
                        if this.multi_select && is_selected {
                            multi_select_icon.to_owned()
                        } else {
                            str::repeat(" ", multi_select_icon.chars().count())
                        },
                        theme.selected,
                    ));
                    spans.extend(display_line.spans);

                    if *wrap {
                        wrap_text(ratatui::text::Text::from(Line::from(spans)), area.width.into()).into()
                    } else {
                        Line::from(spans).into()
                    }
                })
                .collect::<Vec<ListItem>>(),
        )
        .direction(this.direction)
        .style(this.theme.normal);

        Widget::render(Clear, area, buf);
        StatefulWidget::render(
            list,
            area,
            buf,
            &mut ListState::default().with_selected(Some(this.current.saturating_sub(this.offset))),
        );
        SkimRender { items_updated }
    }
}

fn toggle_item(sel: &mut IndexSet<MatchedItem>, item: &MatchedItem) {
    if sel.contains(item) {
        sel.shift_remove(item);
    } else {
        sel.insert(item.clone());
    }
}
