use std::{
    cmp::{max, min},
    collections::HashSet,
};

use ratatui::style::Color;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::{
    item::{MatchedItem, RankBuilder},
    DisplayContext, MatchRange, SkimItem,
};

pub(crate) struct ItemList {
    pub items: Vec<MatchedItem>,
    pub cursor: usize,
    pub view_range: (usize, usize),
    pub selection: HashSet<MatchedItem>,
    pub tx: UnboundedSender<Vec<MatchedItem>>,
    rank_builder: RankBuilder,
    rx: UnboundedReceiver<Vec<MatchedItem>>,
}

impl Default for ItemList {
    fn default() -> Self {
        let (tx, rx) = unbounded_channel();
        Self {
            items: Vec::default(),
            cursor: usize::default(),
            view_range: (0, 0),
            selection: HashSet::default(),
            rank_builder: RankBuilder::default(),
            tx,
            rx,
        }
    }
}

impl ItemList {
    pub fn move_cursor_by(&mut self, offset: i32) {
        if -offset >= self.cursor as i32 {
            self.move_cursor_to(0);
        } else {
            self.move_cursor_to((self.cursor as i32 + offset) as usize);
        }
    }

    pub fn move_cursor_to(&mut self, pos: usize) {
        if self.items.is_empty() {
            return;
        }
        let clamped = usize::clamp(pos, 0, self.items.len() - 1) as usize;
        self.cursor = clamped;

        // Move view range
        let (mut start, mut end) = self.view_range;
        let height = end - start;
        if self.cursor < start {
            trace!("Scrolling under start (target: {}, start: {})", self.cursor, start);
            start = self.cursor;
            end = self.cursor + height;
        } else if self.cursor >= end {
            trace!("Scrolling over end(target: {}, end: {})", self.cursor, end);
            end = self.cursor + 1;
            start = end - height;
        }
        self.view_range = (start, end);
    }

    fn toggle_item(&mut self, item: &MatchedItem) {
        if self.selection.contains(item) {
            self.selection.remove(item);
        } else {
            self.selection.insert(item.clone());
        }
    }

    pub fn toggle_at(&mut self, index: usize) {
        let item = self.items[index + self.view_range.0].clone();
        self.toggle_item(&item);
    }
    pub fn toggle(&mut self) {
        self.toggle_at(self.cursor);
    }
    pub fn toggle_all(&mut self) {
        for item in self.items.clone() {
            self.toggle_item(&item);
        }
    }
    pub fn select(&mut self) {
        self.select_row(self.cursor)
    }
    pub fn select_row(&mut self, row: usize) {
        let item = self.items[self.view_range.0 + row].clone();
        self.selection.insert(item);
    }
    pub fn select_all(&mut self) {
        for item in self.items.clone() {
            self.selection.insert(item.clone());
        }
    }
}

impl Widget for &mut ItemList {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        if let Ok(items) = self.rx.try_recv() {
            debug!("Got {} items to put in list", items.len());
            self.items = items;
        }

        if self.items.is_empty() {
            return;
        }

        let (start, _) = self.view_range;
        let end = min(start + area.height as usize, self.items.len());
        self.view_range.1 = end;

        let mut items = self.items[start..end].to_vec();
        items.sort_by_key(|item| {
            return item.rank;
        });

        Paragraph::new(
            items
                .iter()
                .enumerate()
                .map(|(cursor_id, item)| {
                    let cursor = if cursor_id + start == self.cursor {
                        Span::styled(">", Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD))
                    } else {
                        Span::raw(" ")
                    };
                    let selector = if self.selection.contains(&item) {
                        Span::styled(">", Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD))
                    } else {
                        Span::raw(" ")
                    };
                    let number = Span::raw(format!(" {} ", item.item_idx));
                    let relnumber = Span::raw(format!(" {} ", cursor_id));
                    let cursor_val = Span::raw(format!(" {} ", self.cursor));
                    let spans = item
                        .item
                        .display(DisplayContext {
                            score: item.rank[0],
                            matches: match &item.matched_range {
                                Some(MatchRange::ByteRange(start, end)) => crate::Matches::ByteRange(*start, *end),
                                Some(MatchRange::Chars(chars)) => crate::Matches::CharIndices(chars.clone()),
                                None => crate::Matches::None,
                            },
                            container_width: area.width as usize,
                            style: Style::from(Color::Blue),
                        })
                        .spans;
                    Line::from(vec![vec![cursor, selector, number, relnumber, cursor_val], spans].concat())
                })
                .rev()
                .collect::<Vec<Line>>(),
        )
        .render(area, buf);
    }
}
