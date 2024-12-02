use std::sync::Arc;
use std::time::Duration;

use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Styled};
use ratatui::text::ToText;
use ratatui::widgets::{Paragraph, Widget};
use regex::Regex;

use crate::theme::ColorTheme;

use crate::model::options::InfoDisplay;

const SPINNER_DURATION: u32 = 200;
// const SPINNERS: [char; 8] = ['-', '\\', '|', '/', '-', '\\', '|', '/'];
const SPINNERS_INLINE: [char; 2] = ['-', '<'];
const SPINNERS_UNICODE: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

lazy_static! {
    static ref RE_FIELDS: Regex = Regex::new(r"\\?(\{-?[0-9.,q]*?})").unwrap();
    static ref RE_PREVIEW_OFFSET: Regex = Regex::new(r"^\+([0-9]+|\{-?[0-9]+\})(-[0-9]+|-/[1-9][0-9]*)?$").unwrap();
}

#[derive(Clone, Default)]
pub(crate) struct StatusLine {
    pub(crate) total: usize,
    pub(crate) matched: usize,
    pub(crate) processed: usize,
    pub(crate) matcher_running: bool,
    pub(crate) multi_selection: bool,
    pub(crate) selected: usize,
    pub(crate) current_item_idx: usize,
    pub(crate) hscroll_offset: i64,
    pub(crate) reading: bool,
    pub(crate) time_since_read: Duration,
    pub(crate) time_since_match: Duration,
    pub(crate) matcher_mode: String,
    pub(crate) theme: Arc<ColorTheme>,
    pub(crate) info: InfoDisplay,
}

impl Widget for &StatusLine {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        if self.info == InfoDisplay::Hidden || self.info == InfoDisplay::Inline {
            return;
        }
        let info_attr = self.theme.info();
        let info_attr_bold = self.theme.info().add_modifier(Modifier::BOLD);

        let a_while_since_read = self.time_since_read > Duration::from_millis(50);
        let a_while_since_match = self.time_since_match > Duration::from_millis(50);

        let spinner_set: &[char] = match self.info {
            InfoDisplay::Default => &SPINNERS_UNICODE,
            InfoDisplay::Inline => &SPINNERS_INLINE,
            InfoDisplay::Hidden => panic!("This should never happen"),
        };

        let layout = Layout::horizontal([
            Constraint::Max(1),
            Constraint::Min(3),
            Constraint::Fill(1),
            Constraint::Min(3),
        ]);
        let [spinner_a, matched_a, _spacer, cursor_a] = layout.areas(area);

        // draw the spinner
        if self.reading && a_while_since_read {
            let mills = (self.time_since_read.as_secs() * 1000) as u32 + self.time_since_read.subsec_millis();
            let index = (mills / SPINNER_DURATION) % (spinner_set.len() as u32);
            let ch = spinner_set[index as usize];
            Paragraph::new(String::from_utf8(vec![ch as u8]).unwrap()).render(spinner_a, buf);
        }

        // display matched/total number
        Paragraph::new(
            format!(" {}/{}", self.matched, self.total)
                .to_text()
                .set_style(info_attr),
        )
        .render(matched_a, buf);

        // // display the matcher mode TODO
        // if !self.matcher_mode.is_empty() {
        //     col += canvas.print_with_attr(0, col, format!("/{}", &self.matcher_mode).as_ref(), info_attr)?;
        // }

        // // display the percentage of the number of processed items TODO
        // if self.matcher_running && a_while_since_match {
        //     col += canvas.print_with_attr(
        //         0,
        //         col,
        //         format!(" ({}%) ", self.processed * 100 / self.total).as_ref(),
        //         info_attr,
        //     )?;
        // }

        // // selected number TODO
        // if self.multi_selection && self.selected > 0 {
        //     col += canvas.print_with_attr(0, col, format!(" [{}]", self.selected).as_ref(), info_attr_bold)?;
        // }

        // item cursor
        let line_num_str = format!(
            " {}/{}{}",
            self.current_item_idx,
            self.hscroll_offset,
            if self.matcher_running { '.' } else { ' ' }
        );
        Paragraph::new(line_num_str.to_text().set_style(info_attr_bold)).render(cursor_a, buf);
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Copy)]
pub(crate) enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(PartialEq, Eq, Clone, Debug, Copy)]
pub(crate) enum ClearStrategy {
    DontClear,
    Clear,
    ClearIfNotNull,
}
