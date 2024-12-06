use std::{
    cmp::{max, min},
    collections::HashSet,
    ops::Deref,
    sync::Arc,
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
    spinlock::SpinLock,
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
        self.move_cursor_to(i32::clamp(self.cursor as i32 + offset, 0, self.items.len() as i32) as usize)
    }

    pub fn move_cursor_to(&mut self, pos: usize) {
        self.cursor = pos;
        let (mut start, mut end) = self.view_range;
        if self.cursor < start {
            start -= start - self.cursor;
            end -= start - self.cursor;
        } else if self.cursor >= end {
            start += self.cursor - end + 1;
            end += self.cursor - end + 1;
        }
        self.view_range = (start, end);
    }

    pub fn toggle_at(&mut self, index: usize) {
        let item = self.items[index + self.view_range.0].clone();
        if self.selection.contains(&item) {
            self.selection.remove(&item);
        } else {
            self.selection.insert(item);
        }
    }
    pub fn toggle(&mut self) {
        self.toggle_at(self.cursor);
    }
    pub fn select(&mut self) {
        let item = self.items[self.cursor + self.view_range.0].clone();
        self.selection.insert(item);
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
            self.view_range.1 = self.view_range.0 + self.items.len();
        }

        let (start, mut end) = self.view_range;
        if end > start + area.height as usize {
            debug!("Resizing item list range");
            end = start + area.height as usize;
            self.view_range = (start, end);
        }

        let mut items = self.items[start..min(max(1, self.items.len()) - 1, end)].to_vec();
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
                    // let number = Span::raw(format!(" {} ", x.item_idx));
                    // let relnumber = Span::raw(format!(" {} ", cursor_id));
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
                    Line::from(vec![vec![cursor, selector], spans].concat())
                })
                .collect::<Vec<Line>>(),
        )
        .render(area, buf);
    }
}
