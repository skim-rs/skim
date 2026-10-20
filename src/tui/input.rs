use std::fmt::Write as _;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Instant;

use ansi_to_tui::IntoText;
use ratatui::prelude::*;
use ratatui::widgets::Widget;
use unicode_display_width::width as display_width;

use crate::SkimOptions;
use crate::helper::item::strip_ansi;
use crate::theme::ColorTheme;
use crate::tui::BorderType;
use crate::tui::options::TuiLayout;
use crate::tui::statusline::{Info, InfoDisplay, spinner_char};
use crate::tui::util::style_line;
use crate::tui::widget::{SkimRender, SkimWidget};

/// Status information to display in the input widget's title
#[derive(Clone, Default)]
pub struct StatusInfo {
    /// Total number of items
    pub total: usize,
    /// Number of matched items
    pub matched: usize,
    /// Number of processed items
    pub processed: usize,
    /// Whether the spinner should be shown (controlled by App with debouncing)
    pub show_spinner: bool,
    /// Current matcher mode (e.g., "RE" for regex)
    pub matcher_mode: String,
    /// Whether multi-selection mode is enabled
    pub multi_selection: bool,
    /// Number of selected items
    pub selected: usize,
    /// Index of the current item
    pub current_item_idx: usize,
    /// Horizontal scroll offset
    pub hscroll_offset: i64,
    /// Start time for calculating spinner animation
    pub start: Option<Instant>,
    /// Inline prefix/separator (when the spinner is hidden)
    pub inline_separator: String,
}

impl StatusInfo {
    /// Build the left-aligned title string (spinner, matched/total, mode, progress, selection)
    /// Used for Default info display mode (separate line)
    pub fn left_title(&self) -> String {
        let mut parts = String::new();

        // Spinner
        if self.show_spinner
            && let Some(start) = self.start
        {
            parts.push(spinner_char(start));
            parts.push(' ');
        } else {
            parts.push_str("  ");
        }

        // Matched/total
        let _ = write!(parts, "{}/{}", self.matched, self.total);

        // Matcher mode
        if !self.matcher_mode.is_empty() {
            let _ = write!(parts, "/{}", self.matcher_mode);
        }

        // Progress percentage
        if self.show_spinner && self.total > 0 && self.processed != self.total {
            let pct = self.processed.saturating_mul(100) / self.total;
            let _ = write!(parts, " ({pct}%)");
        }

        // Selection count
        if self.multi_selection && self.selected > 0 {
            let _ = write!(parts, " [{}]", self.selected);
        }

        parts
    }

    /// Get the inline separator character: spinner when active, '<' otherwise
    /// Used for Inline info display mode
    pub fn inline_separator_or_spinner(&self) -> String {
        if self.show_spinner
            && let Some(start) = self.start
        {
            format!(
                "{}{}",
                spinner_char(start),
                " ".repeat(display_width(&self.inline_separator).try_into().unwrap())
            )
        } else {
            self.inline_separator.clone()
        }
    }

    /// Build the inline status string (matched/total, mode, progress, selection)
    /// Used for Inline info display mode - does NOT include spinner prefix
    pub fn inline_status(&self) -> String {
        let mut parts = String::new();

        // Matched/total
        let _ = write!(parts, "{}/{}", self.matched, self.total);

        // Matcher mode
        if !self.matcher_mode.is_empty() {
            let _ = write!(parts, "/{}", self.matcher_mode);
        }

        // Progress percentage
        if self.show_spinner && self.total > 0 {
            let pct = self.processed.saturating_mul(100) / self.total;
            let _ = write!(parts, " ({pct}%)");
        }

        // Selection count
        if self.multi_selection && self.selected > 0 {
            let _ = write!(parts, " [{}]", self.selected);
        }

        parts
    }

    /// Build the right-aligned title string (current index / hscroll)
    pub fn right_title(&self) -> String {
        format!("{}/{}", self.current_item_idx, self.hscroll_offset)
    }
}

pub struct Input {
    pub prompt: String,
    /// see `alternate_value`
    alternate_prompt: String,
    pub value: String,
    /// cmd query when in normal mode, query when in interactive mode
    alternate_value: String,
    pub cursor_pos: u16,
    pub alternate_cursor_pos: u16,
    pub theme: Arc<ColorTheme>,
    /// Border type
    pub border: BorderType,
    /// Status information to display as the input's title
    pub status_info: Option<StatusInfo>,
    /// How to display the info/status (default, inline, or hidden)
    pub info: Info,
    /// Whether layout is reversed (status goes below input instead of above)
    pub reverse: bool,
    /// Repeating horizontal separator, independent of the inline info prefix.
    pub separator: Option<String>,
}

impl Default for Input {
    fn default() -> Self {
        Self::_default()
    }
}

impl Input {
    pub fn insert(&mut self, c: char) {
        self.value.insert(self.cursor_pos.into(), c);
        // unwrap: len_utf8 < 4
        self.move_cursor(c.len_utf8().try_into().unwrap());
    }
    pub fn insert_str(&mut self, s: &str) {
        self.value.insert_str(self.cursor_pos as usize, s);
        // `cursor_pos` is a byte offset (see `move_cursor_to`), so advance by the
        // inserted byte length, not the char count.
        self.move_cursor(s.len().try_into().expect("Failed to fit inserted str len into an i32"));
    }
    fn nchars(&self) -> usize {
        self.value.chars().count()
    }
    pub fn delete(&mut self, offset: i32) -> Option<char> {
        if self.value.is_empty() {
            return None;
        }
        let new_pos = i32::from(self.cursor_pos) + offset;
        if new_pos < 0 || usize::try_from(new_pos).map_or(true, |p| p >= self.value.len()) {
            return None;
        }
        let pos = self.value.floor_char_boundary(new_pos.unsigned_abs() as usize);
        let ch = self.value.remove(pos);
        // Only move cursor if deleting backwards
        if offset < 0 {
            self.move_cursor(-1);
        }
        Some(ch)
    }
    pub fn move_cursor(&mut self, offset: i32) {
        if offset == 0 {
            return;
        }
        if offset < 0 {
            let new_pos = (i32::from(self.cursor_pos) + offset).max(0).unsigned_abs() as usize;
            self.move_cursor_to(u16::try_from(self.value.floor_char_boundary(new_pos)).unwrap_or(u16::MAX));
        } else {
            let new_pos = (i32::from(self.cursor_pos) + offset).unsigned_abs() as usize;
            self.move_cursor_to(u16::try_from(self.value.ceil_char_boundary(new_pos)).unwrap_or(u16::MAX));
        }
    }
    pub fn move_cursor_to(&mut self, pos: u16) {
        if self.value.is_char_boundary(pos as usize) {
            self.cursor_pos = u16::clamp(pos, 0, u16::try_from(self.value.len()).unwrap_or(u16::MAX));
        } else {
            warn!("Invalid cursor pos");
        }
    }
    pub fn move_to_end(&mut self) {
        self.move_cursor_to(
            self.value
                .len()
                .try_into()
                .expect("Failed to fit input len into an u16"),
        );
    }

    /// Check if a character is a word character (alphanumeric only)
    fn is_word_char(ch: char) -> bool {
        ch.is_alphanumeric()
    }

    /// Find the position of the end of the next word (alphanumeric boundaries for deletion)
    fn find_next_word_end(&self, start_pos: usize) -> usize {
        let mut pos = start_pos;

        // Skip any non-word characters
        while pos < self.nchars() {
            let ch = self.value.chars().nth(pos).unwrap();
            if Self::is_word_char(ch) {
                break;
            }
            pos += 1;
        }

        // Skip to the end of the word
        while pos < self.nchars() {
            let ch = self.value.chars().nth(pos).unwrap();
            if !Self::is_word_char(ch) {
                break;
            }
            pos += 1;
        }

        pos
    }

    /// Find the end of compound word (whitespace boundaries for cursor movement)
    fn find_compound_word_end(&self, start_pos: usize) -> usize {
        let mut pos = start_pos;

        // Skip any whitespace
        while pos < self.nchars() {
            let ch = self.value.chars().nth(pos).unwrap();
            if !ch.is_whitespace() {
                break;
            }
            pos += 1;
        }

        // Skip to the end of the non-whitespace sequence (includes punctuation)
        while pos < self.nchars() {
            let ch = self.value.chars().nth(pos).unwrap();
            if ch.is_whitespace() {
                break;
            }
            pos += 1;
        }

        pos
    }

    /// Find the position of the start of the previous word (alphanumeric word boundaries)
    fn find_prev_word_start(&self, start_pos: usize) -> usize {
        if start_pos == 0 {
            return 0;
        }

        let mut pos = start_pos;

        // Move back at least one position
        pos = pos.saturating_sub(1);

        // Skip any non-word characters
        while pos > 0 && !Self::is_word_char(self.value.chars().nth(pos).unwrap()) {
            pos -= 1;
        }

        // Skip to the beginning of the word
        while pos > 0 && Self::is_word_char(self.value.chars().nth(pos - 1).unwrap()) {
            pos -= 1;
        }

        pos
    }

    /// Find the position to delete backward to (stops at non-word characters)
    fn find_delete_backward_pos(&self, start_pos: usize) -> usize {
        if start_pos == 0 {
            return 0;
        }

        let mut pos = start_pos;

        // Skip any non-word characters (whitespace, punctuation, etc.)
        while pos > 0 {
            let ch = self.value.chars().nth(pos - 1).unwrap();
            if Self::is_word_char(ch) {
                break;
            }
            pos -= 1;
        }

        // Skip back through word characters
        while pos > 0 {
            let ch = self.value.chars().nth(pos - 1).unwrap();
            if !Self::is_word_char(ch) {
                break;
            }
            pos -= 1;
        }

        pos
    }

    pub fn delete_backward_word(&mut self) -> String {
        if self.cursor_pos == 0 {
            return String::new();
        }
        // Delete back by alphanumeric word boundaries (for Alt+Backspace)
        let start_pos = self.find_delete_backward_pos(self.cursor_pos as usize);
        let deleted = self.value[start_pos..self.cursor_pos as usize].to_string();
        self.value = format!(
            "{}{}",
            &self.value[..start_pos],
            &self.value[self.cursor_pos as usize..]
        );
        self.cursor_pos = u16::try_from(start_pos).unwrap_or(u16::MAX);
        deleted
    }

    pub fn delete_backward_to_whitespace(&mut self) -> String {
        if self.cursor_pos == 0 {
            return String::new();
        }
        // Unix word rubout: delete back to whitespace (for Ctrl+W)
        let mut pos = self.cursor_pos as usize;

        // Skip any trailing whitespace
        while pos > 0 && self.value.chars().nth(pos - 1).unwrap_or_default().is_whitespace() {
            pos -= 1;
        }

        // Delete back to next whitespace or start
        while pos > 0 && !self.value.chars().nth(pos - 1).unwrap_or_default().is_whitespace() {
            pos -= 1;
        }

        let deleted = self.value[pos..self.cursor_pos as usize].to_string();
        self.value = format!("{}{}", &self.value[..pos], &self.value[self.cursor_pos as usize..]);
        self.cursor_pos = u16::try_from(pos).unwrap_or(u16::MAX);
        deleted
    }

    pub fn delete_forward_word(&mut self) -> String {
        if self.cursor_pos as usize >= self.value.len() {
            return String::new();
        }
        let end_pos = self.find_next_word_end(self.cursor_pos as usize);
        let deleted = self.value[self.cursor_pos as usize..end_pos].to_string();
        self.value = format!("{}{}", &self.value[..self.cursor_pos as usize], &self.value[end_pos..]);
        deleted
    }
    pub fn move_cursor_forward_word(&mut self) {
        let new_pos = self.find_compound_word_end(self.cursor_pos as usize);
        self.cursor_pos = u16::try_from(new_pos).unwrap_or(u16::MAX);
    }

    pub fn move_cursor_backward_word(&mut self) {
        let new_pos = self.find_prev_word_start(self.cursor_pos as usize);
        self.cursor_pos = u16::try_from(new_pos).unwrap_or(u16::MAX);
    }
    pub fn delete_to_beginning(&mut self) -> String {
        let deleted = self.value[..self.cursor_pos as usize].to_string();
        self.value = self.value[self.cursor_pos as usize..].to_string();
        self.cursor_pos = 0;
        deleted
    }
    pub fn cursor_pos(&self) -> u16 {
        (display_width(&self.value[..(self.cursor_pos as usize)]) + display_width(&strip_ansi(&self.prompt).0))
            .try_into()
            .expect("Failed to fit cursor char into an u16")
    }
    pub fn switch_mode(&mut self) {
        std::mem::swap(&mut self.prompt, &mut self.alternate_prompt);
        std::mem::swap(&mut self.value, &mut self.alternate_value);
        std::mem::swap(&mut self.cursor_pos, &mut self.alternate_cursor_pos);
    }

    fn append_inline_info(&self, line: &mut Line<'_>, inner: Rect) -> Option<Rect> {
        let status = self.status_info.as_ref()?;
        let separator = status.inline_separator_or_spinner();
        let inline_status = status.inline_status();
        let right_status = status.right_title();
        let used_width = line.width()
            + Line::from(separator.as_str()).width()
            + Line::from(inline_status.as_str()).width()
            + Line::from(right_status.as_str()).width();
        let padding_width = usize::from(inner.width).saturating_sub(used_width);

        if self.info.display == InfoDisplay::InlineRight {
            line.push_span(Span::raw(" ".repeat(padding_width.saturating_sub(2))));
        }
        line.push_span(Span::styled(separator, self.theme.info));
        line.push_span(Span::styled(inline_status, self.theme.info));
        let separator_area = if self.info.display == InfoDisplay::Inline {
            let start = line.width().saturating_add(1);
            line.push_span(Span::raw(" ".repeat(padding_width)));
            Some(
                Rect::new(
                    inner.x.saturating_add(u16::try_from(start).unwrap_or(u16::MAX)),
                    inner.y,
                    u16::try_from(padding_width.saturating_sub(2)).unwrap_or(u16::MAX),
                    1,
                )
                .intersection(inner),
            )
        } else {
            line.push_span(Span::raw(" ".repeat(2)));
            None
        };
        line.push_span(Span::styled(right_status, self.theme.info));
        separator_area
    }

    fn render_separator(&self, mut area: Rect, buf: &mut Buffer) {
        let Some(pattern) = &self.separator else { return };
        if area.is_empty() {
            return;
        }
        if matches!(
            self.info.display,
            InfoDisplay::Default | InfoDisplay::Left | InfoDisplay::Right
        ) {
            let width = usize::from(area.width);
            let (start, end) = self.status_info.as_ref().map_or((0, width), |status| {
                let left = Line::from(status.left_title()).width();
                let right = Line::from(status.right_title()).width();
                match self.info.display {
                    InfoDisplay::Left => (left + right + 3, width),
                    InfoDisplay::Right => (0, width.saturating_sub(left + right + 3)),
                    _ => (left + 1, width.saturating_sub(right + 1)),
                }
            });
            let start = u16::try_from(start.min(width)).unwrap();
            let end = u16::try_from(end.min(width)).unwrap();
            if end > start {
                for x in [start.checked_sub(1), (end < area.width).then_some(end)]
                    .into_iter()
                    .flatten()
                {
                    buf[(area.x + x, area.y)]
                        .set_symbol(" ")
                        .set_style(Style::reset().patch(self.theme.normal));
                }
            }
            area.x += start;
            area.width = end.saturating_sub(start);
        }
        let width = Line::from(pattern.as_str()).width();
        if width > 0 && !area.is_empty() {
            Line::styled(
                pattern.repeat(usize::from(area.width).div_ceil(width)),
                Style::reset().patch(self.theme.normal).patch(self.theme.separator),
            )
            .render(area, buf);
        }
    }
}

impl SkimWidget for Input {
    fn from_options(options: &SkimOptions, theme: Arc<ColorTheme>) -> Self {
        let mut res = Self {
            theme,
            border: options.border,
            info: options.info.clone(),
            reverse: options.layout == TuiLayout::Reverse,
            separator: options.separator.clone(),
            prompt: String::new(),
            alternate_prompt: String::new(),
            value: String::new(),
            alternate_value: String::new(),
            cursor_pos: 0,
            alternate_cursor_pos: 0,
            status_info: None,
        };
        if options.interactive {
            res.prompt.clone_from(&options.cmd_prompt);
            res.alternate_prompt.clone_from(&options.prompt);
            res.value = options.cmd_query.clone().unwrap_or_default();
            res.alternate_value = options.query.clone().unwrap_or_default();
        } else {
            res.prompt.clone_from(&options.prompt);
            res.alternate_prompt.clone_from(&options.cmd_prompt);
            res.value = options.query.clone().unwrap_or_default();
            res.alternate_value = options.cmd_query.clone().unwrap_or_default();
        }
        res.cursor_pos = u16::try_from(res.value.len()).unwrap_or(u16::MAX);
        res.alternate_cursor_pos = u16::try_from(res.alternate_value.len()).unwrap_or(u16::MAX);
        res
    }

    fn render(&mut self, mut area: Rect, buf: &mut Buffer) -> SkimRender {
        use ratatui::layout::Alignment;
        use ratatui::text::{Line, Span};
        use ratatui::widgets::{Block, Borders, Paragraph, TitlePosition};

        let inset = u16::from(self.border.is_some());
        let status_area = Rect::new(
            area.x.saturating_add(inset),
            if self.reverse {
                area.bottom().saturating_sub(1)
            } else {
                area.y
            },
            area.width.saturating_sub(2 * inset),
            area.height.min(1),
        );
        let mut separator_area = None;
        if self.separator.is_some()
            && matches!(self.info.display, InfoDisplay::InlineRight | InfoDisplay::Hidden)
            && !area.is_empty()
        {
            separator_area = Some(status_area);
            if self.border.is_none() {
                if !self.reverse {
                    area.y += 1;
                }
                area.height -= 1;
            }
        }

        let mut line = self.prompt.into_text().map_or_else(
            |_| Line::from(self.prompt.as_str()),
            |t| t.lines.into_iter().next().unwrap_or_default(),
        );
        style_line(&mut line, self.theme.prompt);
        line.push_span(Span::styled(&self.value, self.theme.query));

        let mut block = Block::default().title_position(if self.reverse {
            TitlePosition::Bottom
        } else {
            TitlePosition::Top
        });

        // Add borders if enabled
        if let Some(border_type) = self.border.into_ratatui() {
            block = block
                .borders(Borders::ALL)
                .border_type(border_type)
                .border_style(self.theme.border);
        }

        // Handle different info display modes
        match self.info.display {
            InfoDisplay::Inline | InfoDisplay::InlineRight => {
                // Inline mode: render status on the same line as input
                // Format: prompt + value + " " + separator_char + " " + status + padding + right_status
                // separator_char is spinner when active, '<' otherwise
                if let Some(inline_area) = self.append_inline_info(&mut line, block.inner(area)) {
                    separator_area = Some(inline_area);
                }
            }
            InfoDisplay::Default | InfoDisplay::Left | InfoDisplay::Right => {
                // Default mode: render status as block title (separate line)
                // In normal layout: status above input (title_top)
                // In reverse layout: status below input (title_bottom)
                //
                // Left and Right modes pack both titles together at the corresponding edge.
                if let Some(ref status) = self.status_info {
                    let left_title = status.left_title();
                    let right_title = status.right_title();

                    if matches!(self.info.display, InfoDisplay::Left | InfoDisplay::Right) {
                        let alignment = if self.info.display == InfoDisplay::Left {
                            Alignment::Left
                        } else {
                            Alignment::Right
                        };
                        let title = Line::from(format!("{left_title}  {right_title}"))
                            .style(self.theme.info)
                            .alignment(alignment);
                        block = block.title(title);
                    } else {
                        let info_line = Line::from(left_title).style(self.theme.info).alignment(Alignment::Left);
                        let index_line = Line::from(right_title)
                            .style(self.theme.info)
                            .alignment(Alignment::Right);
                        block = block.title(info_line).title(index_line);
                    }
                }

                separator_area = Some(status_area);
            }
            InfoDisplay::Hidden => {}
        }

        Paragraph::new(line)
            .block(block)
            .style(self.theme.normal)
            .render(area, buf);
        if let Some(separator_area) = separator_area {
            self.render_separator(separator_area, buf);
        }

        SkimRender::default()
    }
}

impl Deref for Input {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl DerefMut for Input {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
