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

use crate::{
    SkimOptions, SkimItemReceiver, SkimItem, CaseMatching,
    event::{Event as SkimEvent, EventReceiver, EventSender},
    item::{ItemPool, MatchedItem, RankBuilder},
    matcher::{Matcher, MatcherControl},
    engine::factory::{AndOrEngineFactory, ExactOrFuzzyEngineFactory},
    reader::{Reader, ReaderControl},
    MatchEngineFactory,
};
use std::sync::{
    Arc,
    mpsc::{self, Receiver, Sender},
};
use defer_drop::DeferDrop;
use std::rc::Rc;
use std::time::{Duration, Instant};
use skim_tuikit::prelude::Key;

pub struct UICoordinator {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    
    // Threading components
    event_tx: EventSender,
    event_rx: EventReceiver,
    item_pool: Arc<DeferDrop<ItemPool>>,
    matcher: Matcher,
    matcher_control: Option<MatcherControl>,
    
    // Reader system (properly handles input sources)
    reader: Reader,
    reader_control: Option<ReaderControl>,
    
    // Query and filtered results
    current_query: String,
    matched_items: Vec<MatchedItem>,
    
    // UI state
    list_state: ListState,
    should_quit: bool,
    
    // Timing
    last_matcher_restart: Instant,
}

impl UICoordinator {
    pub fn new(options: &SkimOptions) -> Result<Self, io::Error> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        // Setup event communication
        let (event_tx, event_rx) = mpsc::channel();
        
        // Setup item pool for threaded processing
        let item_pool = Arc::new(DeferDrop::new(ItemPool::new()));
        
        // Setup matcher with proper engine
        let rank_builder = Arc::new(RankBuilder::new(options.tiebreak.clone()));
        let fuzzy_engine_factory: Rc<dyn MatchEngineFactory> = Rc::new(AndOrEngineFactory::new(
            ExactOrFuzzyEngineFactory::builder()
                .exact_mode(options.exact)
                .rank_builder(rank_builder)
                .build(),
        ));
        let matcher = Matcher::builder(fuzzy_engine_factory)
            .case(options.case)
            .build();

        // Setup reader to handle input sources
        let reader = Reader::with_options(options);

        Ok(UICoordinator {
            terminal,
            
            // Threading components
            event_tx,
            event_rx,
            item_pool,
            matcher,
            matcher_control: None,
            
            // Reader system
            reader,
            reader_control: None,
            
            // Query and results
            current_query: String::new(),
            matched_items: Vec::new(),
            
            // UI state
            list_state: ListState::default(),
            should_quit: false,
            
            // Timing
            last_matcher_restart: Instant::now(),
        })
    }

    pub fn set_item_source(&mut self, source: SkimItemReceiver) {
        // Update reader with the new source - Reader::source() consumes self and returns Self
        self.reader = Reader::with_options(&SkimOptions::default()).source(Some(source));
    }

    pub fn run(&mut self) -> Result<(), io::Error> {
        // Start reader with default command (like the legacy system)
        let default_command = match std::env::var("SKIM_DEFAULT_COMMAND").as_ref().map(String::as_ref) {
            Ok("") | Err(_) => "find .".to_owned(),
            Ok(val) => val.to_owned(),
        };
        self.reader_control = Some(self.reader.run(&default_command));
        
        // Start heartbeat to process incoming items
        self.send_heartbeat();
        
        loop {
            self.draw()?;
            
            if self.should_quit {
                break;
            }

            // Handle crossterm events with timeout
            if event::poll(Duration::from_millis(10))? {
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
                            self.current_query.push(c);
                            self.on_query_change();
                        }
                        KeyCode::Backspace => {
                            self.current_query.pop();
                            self.on_query_change();
                        }
                        _ => {}
                    }
                }
            }

            // Process threaded events (heartbeats, matcher results)
            self.process_events();
            
            // Process incoming items from source
            self.process_incoming_items();
        }

        // Cleanup
        if let Some(ctrl) = self.reader_control.take() {
            ctrl.kill();
        }
        if let Some(ctrl) = self.matcher_control.take() {
            ctrl.kill();
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

    fn process_events(&mut self) {
        // Process all available events without blocking
        while let Ok((key, event)) = self.event_rx.try_recv() {
            match event {
                SkimEvent::EvHeartBeat => {
                    self.handle_heartbeat();
                }
                _ => {
                    // Handle other events as needed
                }
            }
        }
    }

    fn process_incoming_items(&mut self) {
        // Process incoming items from reader and add them to item pool
        if let Some(ref reader_control) = self.reader_control {
            let new_items = reader_control.take();
            
            if !new_items.is_empty() {
                self.item_pool.append(new_items);
                // Restart matcher if we have new items and it's not currently running
                if self.matcher_control.is_none() {
                    self.restart_matcher();
                }
            }
        }
    }

    fn handle_heartbeat(&mut self) {
        // Check if matcher has finished and collect results
        let matcher_stopped = self
            .matcher_control
            .as_ref()
            .map(|ctrl| ctrl.stopped())
            .unwrap_or(false);

        if matcher_stopped {
            if let Some(ctrl) = self.matcher_control.take() {
                let items_lock = ctrl.into_items();
                let mut items = items_lock.lock();
                let matched = std::mem::take(&mut *items);
                
                // Update matched items and sort by rank (like legacy Selection system)
                self.matched_items = matched;
                self.matched_items.sort();
                
                // Reset selection to first item
                if !self.matched_items.is_empty() {
                    self.list_state.select(Some(0));
                } else {
                    self.list_state.select(None);
                }
            }
        }

        // Check if we should restart matcher (like legacy system logic)
        let items_consumed = self.item_pool.num_not_taken() == 0;
        let reader_stopped = self.reader_control.as_ref().map(|c| c.is_done()).unwrap_or(true);
        let processed = reader_stopped && items_consumed;
        
        // Run matcher if matcher had been stopped and reader had new items
        if !processed && self.matcher_control.is_none() {
            self.restart_matcher();
        }

        // Send next heartbeat if matcher is still running or there are items not been processed
        if self.matcher_control.is_some() || !processed {
            self.send_heartbeat_delayed();
        }
    }

    fn on_query_change(&mut self) {
        // Kill existing matcher (like legacy system)
        if let Some(ctrl) = self.matcher_control.take() {
            ctrl.kill();
        }
        
        // CRITICAL: Clear previous results and reset item pool (like legacy system)
        self.matched_items.clear();
        self.item_pool.reset();
        
        // Restart matcher with new query
        self.restart_matcher();
    }

    fn restart_matcher(&mut self) {
        self.last_matcher_restart = Instant::now();
        
        // Kill existing matcher if it exists
        if let Some(ctrl) = self.matcher_control.take() {
            ctrl.kill();
        }

        // CRITICAL: Move items from reader to item pool (like legacy system)
        let processed = self.reader_control.as_ref().map(|c| c.is_done()).unwrap_or(true);
        if !processed {
            // Take out new items and put them into item pool
            if let Some(ref reader_control) = self.reader_control {
                let new_items = reader_control.take();
                if !new_items.is_empty() {
                    self.item_pool.append(new_items);
                }
            }
        }

        // Send heartbeat to trigger processing
        self.send_heartbeat();

        // Start new matcher with current query
        let tx = self.event_tx.clone();
        let matcher_control = self.matcher.run(&self.current_query, self.item_pool.clone(), move |_| {
            // Send heartbeat when matcher has results
            let _ = tx.send((Key::Null, SkimEvent::EvHeartBeat));
        });

        self.matcher_control = Some(matcher_control);
    }

    fn send_heartbeat(&self) {
        let _ = self.event_tx.send((Key::Null, SkimEvent::EvHeartBeat));
    }

    fn send_heartbeat_delayed(&self) {
        // Use a simple approach: send heartbeat after a delay
        // In a real implementation, you'd use a timer thread
        let tx = self.event_tx.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            let _ = tx.send((Key::Null, SkimEvent::EvHeartBeat));
        });
    }

    fn draw(&mut self) -> Result<(), io::Error> {
        self.terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Query input
                    Constraint::Min(0),    // Items list
                    Constraint::Length(1), // Status line
                ])
                .split(f.area());

            // Query input
            let query_widget = Paragraph::new(format!("> {}", self.current_query))
                .block(Block::default().borders(Borders::ALL).title("Query"));
            f.render_widget(query_widget, chunks[0]);

            // Items list using threaded results
            let items: Vec<ListItem> = self
                .matched_items
                .iter()
                .map(|matched_item| ListItem::new(matched_item.item.output().to_string()))
                .collect();

            let items_widget = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Items"))
                .highlight_style(Style::default().bg(Color::LightBlue));

            f.render_stateful_widget(items_widget, chunks[1], &mut self.list_state);

            // Status line
            let total = self.item_pool.len();
            let matched = self.matched_items.len();
            let processing = self.matcher_control.is_some();
            let reading = self.reader_control.as_ref().map(|c| !c.is_done()).unwrap_or(false);
            let status_text = format!(
                " {}/{} {}{}", 
                matched, 
                total,
                if reading { "(reading...)" } else { "" },
                if processing { "(processing...)" } else { "" }
            );
            let status_widget = Paragraph::new(status_text);
            f.render_widget(status_widget, chunks[2]);
        })?;

        Ok(())
    }

    fn next_item(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.matched_items.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        if !self.matched_items.is_empty() {
            self.list_state.select(Some(i));
        }
    }

    fn previous_item(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.matched_items.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        if !self.matched_items.is_empty() {
            self.list_state.select(Some(i));
        }
    }

    pub fn ui_state(&self) -> UIState {
        UIState {
            selection_state: SelectionState {
                items: self.matched_items.clone(),
                selected_index: self.list_state.selected(),
            },
            query_state: QueryState {
                content: self.current_query.clone(),
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