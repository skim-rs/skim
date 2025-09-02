use std::{collections::HashSet, hash::{DefaultHasher, Hash, Hasher}, sync::Arc};

use ratatui::{
    style::{Color, Stylize as _},
    widgets::{List, ListDirection, ListState, StatefulWidget},
};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    DisplayContext, MatchRange, SkimItem,
    item::{MatchedItem, RankBuilder},
};

pub struct ItemList {
    pub(crate) items: Vec<MatchedItem>,
    pub(crate) selection: HashSet<MatchedItem>,
    pub(crate) tx: UnboundedSender<Vec<MatchedItem>>,
    rank_builder: RankBuilder,
    rx: UnboundedReceiver<Vec<MatchedItem>>,
    pub(crate) direction: ListDirection,
    pub(crate) offset: usize,
    pub(crate) current: usize,
    pub(crate) height: u16,
    pub(crate) theme: std::sync::Arc<crate::theme::ColorTheme>,
}

impl Default for ItemList {
    fn default() -> Self {
        let (tx, rx) = unbounded_channel();
        Self {
            tx,
            rx,
            direction: ListDirection::BottomToTop,
            items: Default::default(),
            selection: Default::default(),
            rank_builder: Default::default(),
            offset: Default::default(),
            current: Default::default(),
            height: Default::default(),
            theme: std::sync::Arc::new(crate::theme::ColorTheme::default()),
        }
    }
}

impl ItemList {
    fn cursor(&self) -> usize {
        self.current
    }
    pub fn selected(&self) -> Option<Arc<dyn SkimItem>> {
        if self.items.is_empty() {
            None
        } else {
            Some(self.items[self.cursor()].item.clone())
        }
    }

    /// Render the item list using the theme colors.
    pub fn render_with_theme(&mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        self.height = area.height;
        if self.current < self.offset {
            self.offset = self.current;
        } else if self.offset + area.height as usize <= self.current {
            self.offset = self.current - area.height as usize + 1;
        }
        if let Ok(items) = self.rx.try_recv() {
            debug!("Got {} items to put in list", items.len());
            self.items = items;
        }

        if self.items.is_empty() {
            return;
        }

        let theme = &self.theme;


        let list = List::new(
            self.items
                .iter()
                .skip(self.offset)
                .take(area.height as usize)
                .map(|item| {
                    let mut spans: Vec<Span> = vec![if self.selection.contains(item) {
                        Span::styled(">", theme.selected().add_modifier(Modifier::BOLD))
                    } else {
                        Span::raw(" ")
                    }];
                    spans.append(&mut item
                        .item
                        .display(DisplayContext {
                            score: item.rank[0],
                            matches: match &item.matched_range {
                                Some(MatchRange::ByteRange(start, end)) => crate::Matches::ByteRange(*start, *end),
                                Some(MatchRange::Chars(chars)) => crate::Matches::CharIndices(chars.clone()),
                                None => crate::Matches::None,
                            },
                            container_width: area.width as usize,
                            style: theme.normal(),
                        })
                        .spans);
                    Line::from(spans)
                })
                .collect::<Vec<Line>>(),
        )
        .highlight_symbol(">")
        .highlight_style(theme.current())
        .direction(self.direction);

        StatefulWidget::render(
            list,
            area,
            buf,
            &mut ListState::default().with_selected(Some(self.current.saturating_sub(self.offset))),
        );
    }
    pub fn toggle_item(&mut self, item: &MatchedItem) {
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
    pub fn scroll_by(&mut self, offset: i32) {
        if offset > 0 {
            self.scroll_down_by(offset.unsigned_abs() as u16);
        } else {
            self.scroll_up_by(offset.unsigned_abs() as u16);
        }
    }
    pub fn scroll_up_by(&mut self, offset: u16) {
        self.current = self.current.saturating_sub(offset as usize);
    }
    pub fn scroll_down_by(&mut self, offset: u16) {
        self.current = self.current.saturating_add(offset as usize);
    }
    pub fn select_previous(&mut self) {
        self.current = self.current.saturating_sub(1);
    }
    pub fn select_next(&mut self) {
        self.current = self.current.saturating_add(1);
    }
}

impl Widget for &mut ItemList {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        self.height = area.height;
        if self.current < self.offset {
            self.offset = self.current;
        } else if self.offset + area.height as usize <= self.current {
            self.offset = self.current - area.height as usize + 1;
        }
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
                .skip(self.offset)
                .take(area.height as usize)
                .map(|item| {
                    let selector = if self.selection.contains(item) {
                        Span::styled(">", self.theme.selected().add_modifier(Modifier::BOLD))
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
                            style: self.theme.normal(),
                        })
                        .spans;
                    spans.insert(0, selector);
                    spans.insert(0, idx);
                    let offset = Span::raw(format!(":{}:", self.offset));
                    let current = Span::raw(format!("{}", self.current.saturating_sub(self.offset)));
                    spans.insert(0, offset);
                    spans.insert(0, current);
                    Line::from(spans)
                })
                .collect::<Vec<Line>>(),
        )
        .highlight_symbol(">")
        .highlight_style(self.theme.current())
        .direction(self.direction);

        StatefulWidget::render(
            list,
            area,
            buf,
            &mut ListState::default().with_selected(Some(self.current.saturating_sub(self.offset))),
        );
    }
}
