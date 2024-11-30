use std::sync::Arc;
use std::time::Duration;

use regex::Regex;
use tuikit::attr::{Attr, Effect};
use tuikit::canvas::Canvas;
use tuikit::draw::{Draw, DrawResult};
use tuikit::widget::Widget;

use crate::event::Event;
use crate::theme::ColorTheme;
use crate::util::clear_canvas;

use super::InfoDisplay;

const SPINNER_DURATION: u32 = 200;
// const SPINNERS: [char; 8] = ['-', '\\', '|', '/', '-', '\\', '|', '/'];
const SPINNERS_INLINE: [char; 2] = ['-', '<'];
const SPINNERS_UNICODE: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

lazy_static! {
    static ref RE_FIELDS: Regex = Regex::new(r"\\?(\{-?[0-9.,q]*?})").unwrap();
    static ref RE_PREVIEW_OFFSET: Regex = Regex::new(r"^\+([0-9]+|\{-?[0-9]+\})(-[0-9]+|-/[1-9][0-9]*)?$").unwrap();
}

#[derive(Clone)]
pub(crate) struct Status {
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

#[allow(unused_assignments)]
impl Draw for Status {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        // example:
        //    /--num_matched/num_read        /-- current_item_index
        // [| 869580/869580                  0.]
        //  `-spinner                         `-- still matching

        // example(inline):
        //        /--num_matched/num_read    /-- current_item_index
        // [>   - 549334/549334              0.]
        //      `-spinner                     `-- still matching

        canvas.clear()?;
        let (screen_width, _) = canvas.size()?;
        clear_canvas(canvas)?;
        if self.info == InfoDisplay::Hidden {
            return Ok(());
        }

        let info_attr = self.theme.info();
        let info_attr_bold = Attr {
            effect: Effect::BOLD,
            ..self.theme.info()
        };

        let a_while_since_read = self.time_since_read > Duration::from_millis(50);
        let a_while_since_match = self.time_since_match > Duration::from_millis(50);

        let mut col = 0;
        let spinner_set: &[char] = match self.info {
            InfoDisplay::Default => &SPINNERS_UNICODE,
            InfoDisplay::Inline => &SPINNERS_INLINE,
            InfoDisplay::Hidden => panic!("This should never happen"),
        };

        if self.info == InfoDisplay::Inline {
            col += canvas.put_char_with_attr(0, col, ' ', info_attr)?;
        }

        // draw the spinner
        if self.reading && a_while_since_read {
            let mills = (self.time_since_read.as_secs() * 1000) as u32 + self.time_since_read.subsec_millis();
            let index = (mills / SPINNER_DURATION) % (spinner_set.len() as u32);
            let ch = spinner_set[index as usize];
            col += canvas.put_char_with_attr(0, col, ch, self.theme.spinner())?;
        } else {
            match self.info {
                InfoDisplay::Inline => col += canvas.put_char_with_attr(0, col, '<', self.theme.prompt())?,
                InfoDisplay::Default => col += canvas.put_char_with_attr(0, col, ' ', self.theme.prompt())?,
                InfoDisplay::Hidden => panic!("This should never happen"),
            }
        }

        // display matched/total number
        col += canvas.print_with_attr(0, col, format!(" {}/{}", self.matched, self.total).as_ref(), info_attr)?;

        // display the matcher mode
        if !self.matcher_mode.is_empty() {
            col += canvas.print_with_attr(0, col, format!("/{}", &self.matcher_mode).as_ref(), info_attr)?;
        }

        // display the percentage of the number of processed items
        if self.matcher_running && a_while_since_match {
            col += canvas.print_with_attr(
                0,
                col,
                format!(" ({}%) ", self.processed * 100 / self.total).as_ref(),
                info_attr,
            )?;
        }

        // selected number
        if self.multi_selection && self.selected > 0 {
            col += canvas.print_with_attr(0, col, format!(" [{}]", self.selected).as_ref(), info_attr_bold)?;
        }

        // item cursor
        let line_num_str = format!(
            " {}/{}{}",
            self.current_item_idx,
            self.hscroll_offset,
            if self.matcher_running { '.' } else { ' ' }
        );
        canvas.print_with_attr(0, screen_width - line_num_str.len(), &line_num_str, info_attr_bold)?;

        Ok(())
    }
}

impl Widget<Event> for Status {}

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
