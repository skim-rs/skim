use std::sync::Arc;
use std::time::{Duration, Instant};

use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Styled};
use ratatui::text::{Line, Span, Text, ToText};
use ratatui::widgets::{Paragraph, Widget};
use regex::Regex;

use crate::theme::ColorTheme;

use crate::SkimOptions;
use crate::model::options::InfoDisplay;

const SPINNER_DURATION: u32 = 200;
// const SPINNERS: [char; 8] = ['-', '\\', '|', '/', '-', '\\', '|', '/'];
const SPINNERS_INLINE: [char; 2] = ['-', '<'];
const SPINNERS_UNICODE: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

// Constants for indicator timing thresholds
const ACTIVE_COLLECTION_THRESHOLD: Duration = Duration::from_secs(2);
const SUSTAINED_MATCHING_THRESHOLD: Duration = Duration::from_millis(300);

lazy_static! {
    static ref RE_FIELDS: Regex = Regex::new(r"\\?(\{-?[0-9.,q]*?})").unwrap();
    static ref RE_PREVIEW_OFFSET: Regex = Regex::new(r"^\+([0-9]+|\{-?[0-9]+\})(-[0-9]+|-/[1-9][0-9]*)?$").unwrap();
}

#[derive(Clone)]
pub struct StatusLine {
    pub total: usize,
    pub matched: usize,
    pub processed: usize,
    pub matcher_running: bool,
    pub multi_selection: bool,
    pub selected: usize,
    pub current_item_idx: usize,
    pub hscroll_offset: i64,
    pub reading: bool,
    pub time_since_read: Duration,
    pub time_since_match: Duration,
    pub matcher_mode: String,
    pub theme: Arc<ColorTheme>,
    pub info: InfoDisplay,
    pub start: Instant,
    // show spinner flag controlled by App (debounced there)
    pub show_spinner: bool,
}

impl Default for StatusLine {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            total: 0,
            matched: 0,
            processed: 0,
            matcher_running: false,
            multi_selection: false,
            selected: 0,
            current_item_idx: 0,
            hscroll_offset: 0,
            reading: false,
            time_since_read: Duration::from_millis(0),
            time_since_match: Duration::from_millis(0),
            matcher_mode: String::new(),
            theme: Arc::new(ColorTheme::default()),
            info: InfoDisplay::Default,
            start: now,
            show_spinner: false,
        }
    }
}

impl StatusLine {
    pub fn with_options(options: &SkimOptions, theme: Arc<ColorTheme>) -> Self {
        Self {
            theme,
            info: options.info.clone(),
            ..Default::default()
        }
    }
}

impl Widget for &StatusLine {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let info_attr = self.theme.info();
        let info_attr_bold = self.theme.info().add_modifier(Modifier::BOLD);

        // Show indicators during active collection phase or sustained matcher activity
        // Use reader timer to detect if we're still in active collection (within 2 seconds of last read)
        let show_progress_indicators = self.time_since_read <= ACTIVE_COLLECTION_THRESHOLD
            || (self.matcher_running && self.time_since_match >= SUSTAINED_MATCHING_THRESHOLD);

        // Compute spinner animation timing once for performance
        let spinner_elapsed_ms = self.start.elapsed().as_millis();

        let spinner_set: &[char] = match self.info {
            InfoDisplay::Default => &SPINNERS_UNICODE,
            InfoDisplay::Inline => &SPINNERS_INLINE,
            InfoDisplay::Hidden => panic!("This should never happen"),
        };

        let layout = Layout::horizontal([Constraint::Max(1), Constraint::Min(3), Constraint::Fill(1)]);
        let [spinner_a, matched_a, cursor_a] = layout.areas(area);

        // draw the spinner - use same logic as other indicators
        if show_progress_indicators {
            // use pre-computed elapsed time for stable animation
            let index = ((spinner_elapsed_ms / (SPINNER_DURATION as u128)) % (spinner_set.len() as u128)) as usize;
            let ch = spinner_set[index];
            Paragraph::new(ch.to_string()).style(self.theme.spinner()).render(spinner_a, buf);
        } else if self.info == InfoDisplay::Inline {
          let ch = spinner_set.last().unwrap();
          Paragraph::new(ch.to_string()).style(self.theme.spinner()).render(spinner_a, buf);
        }

        // build matched/total and extra info (mode, percentage, selection)
        let mut parts: Vec<Span> = Vec::new();
        parts.push(Span::styled(format!(" {}/{}", self.matched, self.total), info_attr));
        if !self.matcher_mode.is_empty() {
            parts.push(Span::styled(format!("/{}", &self.matcher_mode), info_attr));
        }
        if show_progress_indicators && self.total > 0 {
            let pct = self.processed.saturating_mul(100) / self.total;
            parts.push(Span::styled(format!(" ({}%)", pct), info_attr));
        }
        if self.multi_selection && self.selected > 0 {
            parts.push(Span::styled(format!(" [{}]", self.selected), info_attr_bold));
        }
        // create a Line from spans and convert to Text for Paragraph
        let line = Line::from(parts);
        Paragraph::new(Text::from(vec![line])).render(matched_a, buf);

        // item cursor (current index / hscroll, show '.' when matcher running or recently active)
        let line_num_str = format!(
            "{}/{}{}",
            self.current_item_idx,
            self.hscroll_offset,
            if show_progress_indicators { '.' } else { ' ' }
        );
        Paragraph::new(line_num_str.to_text().set_style(info_attr_bold))
            .alignment(ratatui::layout::Alignment::Right)
            .render(cursor_a, buf);
    }
}
