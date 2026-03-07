use std::{rc::Rc, sync::Arc};

use indexmap::IndexSet;
use ratatui::widgets::{Block, Borders, Clear, List, ListDirection, ListItem, Widget};
use regex::Regex;

use crate::{
    Selector, SkimItem, SkimOptions,
    item::MatchedItem,
    spinlock::SpinLock,
    theme::ColorTheme,
    tui::BorderType,
    tui::item_renderer::ItemRenderer,
    tui::options::TuiLayout,
    tui::widget::{SkimRender, SkimWidget},
};

/// How to apply processed items to the display list
#[derive(Default, Clone, Copy)]
pub(crate) enum MergeStrategy {
    /// Replace the entire item list (full re-match or first result)
    #[default]
    Replace,
    /// Merge into existing list using sorted merge by rank
    SortedMerge,
    /// Append to existing list without sorting (for --no-sort)
    Append,
}

/// Processed items ready for rendering
pub(crate) struct ProcessedItems {
    pub(crate) items: Vec<MatchedItem>,
    pub(crate) merge: MergeStrategy,
}

impl Default for ProcessedItems {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            merge: MergeStrategy::Replace,
        }
    }
}

/// Widget for displaying and managing the list of filtered items
pub struct ItemList {
    pub(crate) items: Vec<MatchedItem>,
    pub(crate) selection: IndexSet<MatchedItem>,
    pub(crate) processed_items: Arc<SpinLock<Option<ProcessedItems>>>,
    pub(crate) direction: ListDirection,
    pub(crate) offset: usize,
    /// How many leading sub-lines of items[offset] have been scrolled off the top.
    /// Only meaningful when multiline is active; always 0 otherwise.
    sub_offset: usize,
    pub(crate) current: usize,
    pub(crate) height: u16,
    pub(crate) theme: std::sync::Arc<crate::theme::ColorTheme>,
    pub(crate) multi_select: bool,
    reserved: usize,
    pub(crate) no_hscroll: bool,
    pub(crate) ellipsis: String,
    pub(crate) keep_right: bool,
    pub(crate) skip_to_pattern: Option<Regex>,
    pub(crate) tabstop: usize,
    selector: Option<Rc<dyn Selector>>,
    pre_select_target: usize, // How many items we want to pre-select
    no_clear_if_empty: bool,
    interactive: bool,              // Whether we're in interactive mode
    showing_stale_items: bool,      // True when displaying old items due to no_clear_if_empty
    pub(crate) manual_hscroll: i32, // Manual horizontal scroll offset for ScrollLeft/ScrollRight
    pub(crate) selector_icon: String,
    pub(crate) multi_select_icon: String,
    cycle: bool,
    pub(crate) wrap: bool,
    /// When Some, split item text on this separator and show each part on its own line
    pub(crate) multiline: Option<String>,
    /// Border type, if borders are enabled
    pub border: Option<BorderType>,
    /// When true, prepend each item's match score to its display text
    pub(crate) print_score: bool,
    /// When true, highlight the entire current line (not just the matched text)
    pub(crate) highlight_line: bool,
}

impl Default for ItemList {
    fn default() -> Self {
        let processed_items = Arc::new(SpinLock::new(None));

        Self {
            processed_items,
            direction: ListDirection::BottomToTop,
            items: Default::default(),
            selection: Default::default(),
            offset: Default::default(),
            sub_offset: 0,
            current: Default::default(),
            height: Default::default(),
            theme: Arc::new(ColorTheme::default()),
            multi_select: false,
            reserved: 0,
            no_hscroll: false,
            ellipsis: String::from(".."),
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
            multiline: None,
            border: None,
            print_score: false,
            highlight_line: false,
        }
    }
}

impl ItemList {
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
        self.sub_offset = 0;
        self.showing_stale_items = false;
    }
    /// Scrolls the list by `rows` terminal rows, counting each item's sub-lines.
    /// Positive = toward higher indices (up the screen in default layout).
    pub fn scroll_by_rows(&mut self, rows: i32) {
        if self.reserved >= self.items.len() || rows == 0 {
            return;
        }
        let total = self.items.len();
        let reserved = self.reserved;

        if rows > 0 {
            let mut remaining = rows as usize;
            let mut idx = self.current;
            while remaining > 0 && idx + 1 < total {
                idx += 1;
                let row_count = self.item_row_count(idx);
                remaining = remaining.saturating_sub(row_count);
            }
            self.current = idx;
        } else {
            let mut remaining = (-rows) as usize;
            let mut idx = self.current;
            while remaining > 0 && idx > reserved {
                let row_count = self.item_row_count(idx);
                remaining = remaining.saturating_sub(row_count);
                idx -= 1;
            }
            self.current = idx.max(reserved);
        }
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
            self.offset = self.reserved;
            self.sub_offset = 0;
        }
    }
    /// Jump to the last item in the list
    pub fn jump_to_last(&mut self) {
        if !self.items.is_empty() {
            self.current = self.items.len().saturating_sub(1);
            self.sub_offset = 0;
        }
    }

    /// Number of terminal rows item at `index` occupies.
    ///
    /// When `--multiline` is active this is the number of sub-lines produced by
    /// splitting on the separator; otherwise every item is exactly 1 row.
    fn item_row_count(&self, index: usize) -> usize {
        if let Some(sep) = self.multiline.as_deref()
            && let Some(item) = self.items.get(index)
        {
            item.item.text().split(sep).count().max(1)
        } else {
            1
        }
    }

    /// How many terminal rows are consumed by items `[from, from + count)`.
    fn rows_for_range(&self, from: usize, count: usize) -> usize {
        (from..from + count).map(|i| self.item_row_count(i)).sum()
    }

    /// How many rows are consumed by items `[offset..=current]`, with `sub_offset`
    /// leading sub-lines of `items[offset]` already scrolled off.
    fn rows_visible(&self, offset: usize, sub_offset: usize, current: usize) -> usize {
        if offset > current {
            return 0;
        }
        let top_rows = self.item_row_count(offset).saturating_sub(sub_offset);
        if offset == current {
            return top_rows;
        }
        top_rows + self.rows_for_range(offset + 1, current - offset)
    }

    /// Advance `(offset, sub_offset)` one row at a time until `current` fits
    /// within `available_rows`.  Returns the new `(offset, sub_offset)`.
    fn advance_to_fit(&self, current: usize, available_rows: usize) -> (usize, usize) {
        let mut offset = self.offset;
        let mut sub_offset = self.sub_offset;
        while offset < current && self.rows_visible(offset, sub_offset, current) > available_rows {
            let top_rows = self.item_row_count(offset);
            if sub_offset + 1 < top_rows {
                // Still more sub-lines in the top item: scroll one sub-line off.
                sub_offset += 1;
            } else {
                // Entire top item scrolled off: move to next item.
                offset += 1;
                sub_offset = 0;
            }
        }
        (offset, sub_offset)
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

        let processed_items = Arc::new(SpinLock::new(None));

        let interactive = options.interactive;
        let no_clear_if_empty = options.no_clear_if_empty;
        let multi_select = options.multi;

        // Spawn background processing thread with the appropriate configuration
        Self {
            processed_items,
            reserved: 0, // header_lines are now displayed in the Header widget, not ItemList
            direction: match options.layout {
                TuiLayout::Default => ratatui::widgets::ListDirection::BottomToTop,
                TuiLayout::Reverse | TuiLayout::ReverseList => ratatui::widgets::ListDirection::TopToBottom,
            },
            current: 0,
            theme,
            multi_select,
            no_hscroll: options.no_hscroll,
            ellipsis: options.ellipsis.clone(),
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
            sub_offset: 0,
            height: Default::default(),
            selector_icon: options.selector_icon.clone(),
            multi_select_icon: options.multi_select_icon.clone(),
            cycle: options.cycle,
            wrap: options.wrap_items,
            multiline: options
                .multiline
                .clone()
                .map(|opt_m| opt_m.unwrap_or(String::from("\\n"))),
            border: options.border,
            print_score: options.flags.contains(&crate::options::FeatureFlag::ShowScore),
            highlight_line: options.highlight_line,
        }
    }

    fn render(&mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) -> SkimRender {
        let this = &mut *self;

        // Calculate inner area if borders are enabled
        let inner_area = if this.border.is_some() {
            ratatui::layout::Rect {
                x: area.x + 1,
                y: area.y + 1,
                width: area.width.saturating_sub(2),
                height: area.height.saturating_sub(2),
            }
        } else {
            area
        };

        this.height = inner_area.height;
        let available_rows = inner_area.height as usize;

        // Clamp current to valid range after any item list replacement.
        if !this.items.is_empty() {
            this.current = this.current.min(this.items.len() - 1).max(this.reserved);
        } else {
            this.current = 0;
            this.offset = 0;
        }

        if this.current < this.offset {
            // Cursor moved above the top item: snap to it with no sub-line offset.
            this.offset = this.current;
            this.sub_offset = 0;
        } else if this.rows_visible(this.offset, this.sub_offset, this.current) > available_rows {
            // Current item is below the visible window: advance one row at a time.
            (this.offset, this.sub_offset) = this.advance_to_fit(this.current, available_rows);
        }
        let initial_current = this.selected();

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
                match processed.merge {
                    MergeStrategy::Replace => {
                        this.items = processed.items;
                        this.sub_offset = 0;
                    }
                    MergeStrategy::SortedMerge => {
                        let existing = std::mem::take(&mut this.items);
                        this.items = MatchedItem::sorted_merge(existing, processed.items);
                        this.sub_offset = 0;
                    }
                    MergeStrategy::Append => {
                        this.items.extend(processed.items);
                    }
                }
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

        let icon_width = this.selector_icon.chars().count() + this.multi_select_icon.chars().count();
        let container_width = (inner_area.width as usize).saturating_sub(icon_width);
        let sub_offset = this.sub_offset;

        let renderer = ItemRenderer::new_for(this, container_width);

        let mut flat_rows: Vec<ListItem<'static>> = Vec::with_capacity(available_rows + 1);
        let mut rows_used = 0usize;

        for (idx, item) in this.items.iter().enumerate().skip(this.offset) {
            if rows_used >= available_rows {
                break;
            }
            let is_current = idx == this.current;
            let is_selected = this.selection.contains(item);
            let skip_subs = if idx == this.offset { sub_offset } else { 0 };
            rows_used += renderer.render_item(
                item,
                is_current,
                is_selected,
                skip_subs,
                available_rows,
                rows_used,
                &mut flat_rows,
            );
        }

        let list = List::new(flat_rows).direction(this.direction).style(this.theme.normal);

        Widget::render(Clear, area, buf);

        // Render border if enabled
        if let Some(border_type) = this.border {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(border_type.into())
                .border_style(this.theme.border);
            Widget::render(block, area, buf);
        }

        // We manage offset and selection styling ourselves, so render as a plain
        // Widget — this bypasses ratatui's get_items_bounds which can index out of
        // bounds on our pre-sliced flat_rows when the selected row is near the edge.
        Widget::render(list, inner_area, buf);
        let run_preview = if let Some(curr) = self.selected()
            && let Some(prev) = initial_current
        {
            curr.text() != prev.text()
        } else {
            self.selected().is_some() != initial_current.is_some()
        };
        SkimRender {
            items_updated,
            run_preview,
        }
    }
}

fn toggle_item(sel: &mut IndexSet<MatchedItem>, item: &MatchedItem) {
    if sel.contains(item) {
        sel.shift_remove(item);
    } else {
        sel.insert(item.clone());
    }
}
