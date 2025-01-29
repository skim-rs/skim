use std::{collections::HashSet, sync::Arc};

use ratatui::{
    style::{Color, Stylize as _},
    widgets::{List, ListDirection, ListState, StatefulWidget},
};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::{
    item::{MatchedItem, RankBuilder},
    DisplayContext, MatchRange, SkimItem,
};

pub(crate) struct ItemList {
    pub(crate) items: Vec<MatchedItem>,
    pub(crate) selection: HashSet<MatchedItem>,
    pub(crate) tx: UnboundedSender<Vec<MatchedItem>>,
    rank_builder: RankBuilder,
    rx: UnboundedReceiver<Vec<MatchedItem>>,
    pub(crate) state: ListState,
    pub(crate) direction: ListDirection,
}

impl Default for ItemList {
    fn default() -> Self {
        let (tx, rx) = unbounded_channel();
        Self {
            items: Vec::default(),
            selection: HashSet::default(),
            rank_builder: RankBuilder::default(),
            tx,
            rx,
            state: ListState::default().with_selected(Some(0)),
            direction: ListDirection::BottomToTop,
        }
    }
}

impl ItemList {
    fn cursor(&self) -> usize {
        self.state.selected().unwrap_or(0)
    }
    pub fn selected(&self) -> Option<Arc<dyn SkimItem>> {
        if self.items.is_empty() {
            return None;
        } else {
            return Some(self.items[self.cursor()].item.clone());
        }
    }

    fn toggle_item(&mut self, item: &MatchedItem) {
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
    pub fn select(&mut self) {
        self.select_row(self.cursor())
    }
    pub fn select_row(&mut self, index: usize) {
        let item = self.items[index].clone();
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
            //self.items.sort_by_key(|item| std::cmp::Reverse(item.rank));
        }

        if self.items.is_empty() {
            return;
        }

        let list = List::new(
            self.items
                .iter()
                .map(|item| {
                    let selector = if self.selection.contains(&item) {
                        Span::styled(">", Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD))
                    } else {
                        Span::raw(" ")
                    };
                    let idx = Span::raw(format!("{}", item.get_index()));
                    let mut spans = item
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
                    spans.insert(0, selector);
                    spans.insert(0, idx);
                    Line::from(spans)
                })
                .collect::<Vec<Line>>(),
        )
        .highlight_symbol(">")
        .highlight_style(Style::new().reversed())
        .direction(self.direction);

        StatefulWidget::render(list, area, buf, &mut self.state)
    }
}
