///! header of the items
use crate::ansi::{ANSIParser, AnsiString};
use crate::event::UpdateScreen;
use crate::event::{Event, EventHandler};
use crate::item::ItemPool;
use crate::theme::ColorTheme;
use crate::theme::DEFAULT_THEME;
use crate::util::{clear_canvas, print_item, str_lines, LinePrinter};
use crate::{DisplayContext, SkimOptions};

use defer_drop::DeferDrop;

use std::cmp::max;
use std::sync::{Arc, Weak};
use tuikit::prelude::*;

pub struct Header {
    header: Vec<AnsiString>,
    tabstop: usize,
    reverse: bool,
    theme: Arc<ColorTheme>,

    // for reserved header items
    item_pool: Weak<DeferDrop<ItemPool>>,
}

impl Header {
    pub fn empty() -> Self {
        Self {
            header: vec![],
            tabstop: 8,
            reverse: false,
            theme: Arc::new(*DEFAULT_THEME),
            item_pool: Weak::new(),
        }
    }

    pub fn upgrade_pool(&self) -> Arc<DeferDrop<ItemPool>> {
        if let Some(upgraded) = Weak::upgrade(&self.item_pool) {
            upgraded
        } else {
            Arc::new(DeferDrop::new(ItemPool::new()))
        }
    }

    pub fn item_pool(mut self, item_pool: &Arc<DeferDrop<ItemPool>>) -> Self {
        self.item_pool = Arc::downgrade(&item_pool);
        self
    }

    pub fn theme(mut self, theme: Arc<ColorTheme>) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_options(mut self, options: &SkimOptions) -> Self {
        if let Some(tabstop_str) = options.tabstop {
            let tabstop = tabstop_str.parse::<usize>().unwrap_or(8);
            self.tabstop = max(1, tabstop);
        }

        if options.layout.starts_with("reverse") {
            self.reverse = true;
        }

        match options.header {
            None => {}
            Some("") => {}
            Some(header) => {
                let mut parser = ANSIParser::default();
                self.header = str_lines(header).into_iter().map(|l| parser.parse_ansi(l)).collect();
            }
        }
        self
    }

    fn lines_of_header(&self) -> usize {
        if let Some(upgraded) = Weak::upgrade(&self.item_pool) {
            self.header.len() + upgraded.reserved().len()
        } else {
            self.header.len()
        }
    }

    fn adjust_row(&self, index: usize, screen_height: usize) -> usize {
        if self.reverse {
            index
        } else {
            screen_height - index - 1
        }
    }
}

impl Draw for Header {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        let (screen_width, screen_height) = canvas.size()?;
        if screen_width < 3 {
            return Err("screen width is too small".into());
        }

        if screen_height < self.lines_of_header() {
            return Err("screen height is too small".into());
        }

        canvas.clear()?;
        clear_canvas(canvas)?;

        self.header.iter().enumerate().for_each(|(idx, header)| {
            // print fixed header(specified by --header)
            let mut printer = LinePrinter::builder()
                .row(self.adjust_row(idx, screen_height))
                .col(2)
                .tabstop(self.tabstop)
                .container_width(screen_width - 2)
                .shift(0)
                .text_width(screen_width - 2)
                .build();

            header.iter().for_each(|(ch, _attr)| {
                printer.print_char(canvas, ch, self.theme.header(), false);
            });
        });

        let lines_used = self.header.len();

        // print "reserved" header lines (--header-lines)
        self.upgrade_pool()
            .reserved()
            .iter()
            .filter_map(Weak::upgrade)
            .enumerate()
            .for_each(|(idx, item)| {
                let mut printer = LinePrinter::builder()
                    .row(self.adjust_row(idx + lines_used, screen_height))
                    .col(2)
                    .tabstop(self.tabstop)
                    .container_width(screen_width - 2)
                    .shift(0)
                    .text_width(screen_width - 2)
                    .build();

                let context = DisplayContext {
                    text: &item.text(),
                    score: 0,
                    matches: None,
                    container_width: screen_width - 2,
                    highlight_attr: self.theme.header(),
                };

                print_item(canvas, &mut printer, item.display(context), self.theme.header());
            });

        Ok(())
    }
}

impl Widget<Event> for Header {
    fn size_hint(&self) -> (Option<usize>, Option<usize>) {
        (None, Some(self.lines_of_header()))
    }
}

impl EventHandler for Header {
    fn handle(&mut self, _event: &Event) -> UpdateScreen {
        UpdateScreen::DONT_REDRAW
    }
}
