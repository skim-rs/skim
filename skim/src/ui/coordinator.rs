//! UI Coordinator for the ratatui-based interface

use std::io;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::{SkimOptions, SkimItemReceiver, MatchResult, MatchRange, Rank};
use crate::item::MatchedItem;

pub struct UICoordinator {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    item_source: Option<SkimItemReceiver>,
    items: Vec<MatchedItem>,
    list_state: ListState,
    query: String,
    should_quit: bool,
}

impl UICoordinator {
    pub fn new(_options: &SkimOptions) -> Result<Self, io::Error> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(UICoordinator {
            terminal,
            item_source: None,
            items: Vec::new(),
            list_state: ListState::default(),
            query: String::new(),
            should_quit: false,
        })
    }

    pub fn set_item_source(&mut self, source: SkimItemReceiver) {
        self.item_source = Some(source);
    }

    pub fn run(&mut self) -> Result<(), io::Error> {
        loop {
            self.draw()?;
            
            if self.should_quit {
                break;
            }

            if event::poll(std::time::Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            self.should_quit = true;
                        }
                        KeyCode::Enter => {
                            self.should_quit = true;
                        }
                        KeyCode::Up => {
                            self.previous_item();
                        }
                        KeyCode::Down => {
                            self.next_item();
                        }
                        KeyCode::Char(c) => {
                            self.query.push(c);
                        }
                        KeyCode::Backspace => {
                            self.query.pop();
                        }
                        _ => {}
                    }
                }
            }

            // Process incoming items
            if let Some(ref item_source) = self.item_source {
                while let Ok(item) = item_source.try_recv() {
                    // For now, just wrap the item in a MatchedItem without actual matching
                    let matched_item = MatchedItem {
                        item,
                        rank: [0, 0, 0, 0, 0], // Default rank
                        matched_range: None,
                        item_idx: self.items.len() as u32,
                    };
                    self.items.push(matched_item);
                }
            }
        }

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;

        Ok(())
    }

    fn draw(&mut self) -> Result<(), io::Error> {
        self.terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Query input
                    Constraint::Min(0),    // Items list
                ])
                .split(f.area());

            // Query input
            let query_widget = Paragraph::new(format!("> {}", self.query))
                .block(Block::default().borders(Borders::ALL).title("Query"));
            f.render_widget(query_widget, chunks[0]);

            // Items list
            let items: Vec<ListItem> = self
                .items
                .iter()
                .map(|item| ListItem::new(item.item.output().to_string()))
                .collect();

            let items_widget = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Items"))
                .highlight_style(Style::default().bg(Color::LightBlue));

            f.render_stateful_widget(items_widget, chunks[1], &mut self.list_state);
        })?;

        Ok(())
    }

    fn next_item(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn previous_item(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn ui_state(&self) -> UIState {
        UIState {
            selection_state: SelectionState {
                items: self.items.clone(),
                selected_index: self.list_state.selected(),
            },
            query_state: QueryState {
                content: self.query.clone(),
            }
        }
    }
}

// Placeholder structures to match the expected interface
pub struct UIState {
    pub selection_state: SelectionState,
    pub query_state: QueryState,
}

pub struct QueryState {
    pub content: String,
}

pub struct SelectionState {
    items: Vec<MatchedItem>,
    selected_index: Option<usize>,
}

impl SelectionState {
    pub fn get_selected_items(&self) -> Vec<MatchedItem> {
        if let Some(index) = self.selected_index {
            if index < self.items.len() {
                vec![self.items[index].clone()]
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }
}