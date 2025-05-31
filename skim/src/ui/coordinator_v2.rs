//! Event-driven UI Coordinator for ratatui interface
//!
//! Architecture principles:
//! - User input always has highest priority via separate channel
//! - Event-driven design prevents unnecessary CPU polling
//! - State isolated to prevent race conditions
//! - Background threads handle I/O without blocking UI

use std::io;
use std::sync::{Arc, Mutex, mpsc};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::{
    SkimOptions, SkimItemReceiver,
    item::{MatchedItem, ItemPool},
    reader::Reader,
};
use defer_drop::DeferDrop;

// Timing constants balancing responsiveness vs CPU usage
const INPUT_POLL_MS: u64 = 20;        // Fast input response
const READER_POLL_MS: u64 = 50;       // Reader thread polling
const BATCH_THRESHOLD: usize = 100;   // Trigger search after N new items

/// Events flowing through the UI system
/// 
/// High priority: KeyPress (goes to separate channel)
/// Normal priority: All others (batched/throttled as needed)
#[derive(Clone)]
enum UIEvent {
    KeyPress(KeyEvent),
    ItemsAvailable,                    // New items ready for search
    MatchResults(Vec<MatchedItem>),    // Search completed
    SearchProgress(usize, usize),      // (processed, total) for progress bar
    ReaderDone,                        // Input stream finished
    Shutdown,                          // Graceful shutdown signal
}

/// Core UI state - kept minimal to reduce complexity
struct UIState {
    // User interaction
    query: String,
    viewport: Viewport,
    
    // Search results
    items: Vec<MatchedItem>,
    // Arc prevents copying entire Vec when sending to search thread
    searchable_items: Arc<Vec<Arc<dyn crate::SkimItem>>>,
    
    // Status flags
    reading: bool,                     // Still reading input
    matching: bool,                    // Search in progress
    should_exit: bool,
    
    // Performance optimizations
    last_searchable_update: usize,    // Prevents redundant snapshots
    last_query_change: Instant,       // For debouncing
    pending_search: bool,              // Debounced search pending
    
    // UI state
    needs_redraw: bool,
    
    // Progress tracking
    search_processed: usize,
    search_total: usize,
}

/// Coordination flags for inter-thread communication
#[derive(Clone)]
struct CoordinationState {
    shutdown: Arc<AtomicBool>,
    pending_items: Arc<AtomicBool>,    // Prevents event flooding
    item_count: Arc<AtomicUsize>,      // Atomic counter for UI display
}

impl CoordinationState {
    fn new() -> Self {
        Self {
            shutdown: Arc::new(AtomicBool::new(false)),
            pending_items: Arc::new(AtomicBool::new(false)),
            item_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

/// Viewport management for scrolling through long item lists
struct Viewport {
    selected: usize,
    offset: usize,
}

impl Viewport {
    fn new() -> Self {
        Self {
            selected: 0,
            offset: 0,
        }
    }
    
    /// Update selection and adjust viewport if needed
    fn update_selection(&mut self, new_selected: usize, item_count: usize, viewport_height: usize) {
        if item_count == 0 {
            self.selected = 0;
            self.offset = 0;
            return;
        }
        
        self.selected = new_selected.min(item_count - 1);
        
        // Keep selected item in view
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + viewport_height {
            self.offset = self.selected.saturating_sub(viewport_height - 1);
        }
    }
    
    /// Reset viewport for new search results
    fn reset(&mut self) {
        self.selected = 0;
        self.offset = 0;
    }
    
    fn selected(&self) -> usize {
        self.selected
    }
    
    fn offset(&self) -> usize {
        self.offset
    }
}


/// Main UI coordinator managing all UI state and background threads
/// 
/// Uses dual-channel architecture:
/// - High priority channel for user input (never blocks UI)
/// - Normal priority channel for background updates (can be throttled)
pub struct UICoordinator<'a> {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    state: UIState,
    options: &'a SkimOptions,
    
    // Dual channel architecture for priority-based event handling
    high_priority_rx: mpsc::Receiver<UIEvent>,
    high_priority_tx: mpsc::SyncSender<UIEvent>,
    normal_rx: mpsc::Receiver<UIEvent>,
    normal_tx: mpsc::SyncSender<UIEvent>,
    
    // Background workers
    reader: Reader,
    item_pool: Arc<DeferDrop<ItemPool>>,
    
    // Search cancellation - allows immediate abort on new search/exit
    current_matcher: Option<Arc<AtomicBool>>,
    
    // Thread lifecycle management - ensures clean shutdown without resource leaks
    input_thread: Option<JoinHandle<()>>,
    reader_thread: Option<JoinHandle<()>>,
    search_thread: Option<JoinHandle<()>>,   // Critical: search threads must be joined
    
    /// Inter-thread coordination
    coordination: CoordinationState,
}

impl<'a> UICoordinator<'a> {
    pub fn new(options: &'a SkimOptions) -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        
        let (high_priority_tx, high_priority_rx) = mpsc::sync_channel(1000);
        let (normal_tx, normal_rx) = mpsc::sync_channel(100);
        
        let reader = Reader::with_options(options);
        let item_pool = Arc::new(DeferDrop::new(ItemPool::new()));
        
        
        
        Ok(Self {
            terminal,
            state: UIState {
                query: String::new(),
                viewport: Viewport::new(),
                items: Vec::new(),
                searchable_items: Arc::new(Vec::new()),
                reading: true,
                matching: false,
                last_searchable_update: 0,
                should_exit: false,
                last_query_change: Instant::now(),
                pending_search: false,
                needs_redraw: true,
                search_processed: 0,
                search_total: 0,
            },
            options,
            high_priority_rx,
            high_priority_tx,
            normal_rx,
            normal_tx,
            reader,
            item_pool,
            current_matcher: None,
            input_thread: None,
            reader_thread: None,
            search_thread: None,
            coordination: CoordinationState::new(),
        })
    }
    
    pub fn set_item_source(&mut self, source: SkimItemReceiver) {
        let reader = std::mem::replace(&mut self.reader, Reader::with_options(self.options));
        self.reader = reader.source(Some(source));
    }
    
    pub fn run(&mut self) -> io::Result<crate::output::SkimOutput> {
        // Start background threads for input and reading
        self.spawn_input_thread();
        self.spawn_reader_thread();
        
        // Initial UI setup
        self.draw()?;
        self.update_searchable_items();
        
        // Show all items initially if we have data
        if !self.state.searchable_items.is_empty() {
            self.trigger_search();
        }
        
        // Main event loop - prioritizes user input over background events
        self.run_event_loop()?;
        
        let selected = if self.state.viewport.selected() < self.state.items.len() {
            vec![self.state.items[self.state.viewport.selected()].item.clone()]
        } else {
            Vec::new()
        };
        
        Ok(crate::output::SkimOutput {
            final_event: crate::event::Event::EvActAccept(None),
            is_abort: self.state.should_exit && self.state.viewport.selected() >= self.state.items.len(),
            final_key: if self.state.should_exit { 
                skim_tuikit::prelude::Key::ESC 
            } else { 
                skim_tuikit::prelude::Key::Enter 
            },
            query: self.state.query.clone(),
            cmd: self.options.cmd.clone().unwrap_or_default(),
            selected_items: selected,
        })
    }
    
    fn handle_event(&mut self, event: UIEvent) -> io::Result<bool> {
        match event {
            UIEvent::KeyPress(key) => {
                return self.handle_key(key);
            }
            UIEvent::ItemsAvailable => {
                self.coordination.pending_items.store(false, Ordering::SeqCst);
                
                if !self.state.matching {
                    let current_pool_size = self.item_pool.len();
                    let diff = current_pool_size.saturating_sub(self.state.last_searchable_update);
                    
                    if diff >= BATCH_THRESHOLD || !self.state.reading {
                        self.update_searchable_items();
                        self.trigger_search();
                    }
                }
            }
            UIEvent::MatchResults(mut results) => {
                results.sort();
                self.state.items = results;
                self.state.matching = false;
                self.state.search_processed = 0;
                self.state.search_total = 0;
                
                self.state.viewport.reset();
                if !self.state.items.is_empty() && self.state.viewport.selected() >= self.state.items.len() {
                    let viewport_height = self.get_viewport_height();
                    self.state.viewport.update_selection(self.state.items.len() - 1, self.state.items.len(), viewport_height);
                }
                self.state.needs_redraw = true;
                
                let current_pool_size = self.item_pool.len();
                if current_pool_size > self.state.last_searchable_update {
                    self.update_searchable_items();
                    if !self.state.query.is_empty() {
                        self.state.pending_search = true;
                        self.state.last_query_change = Instant::now();
                    }
                }
            }
            UIEvent::SearchProgress(processed, total) => {
                self.state.search_processed = processed;
                self.state.search_total = total;
                self.state.needs_redraw = true;
            }
            UIEvent::ReaderDone => {
                self.state.reading = false;
                if !self.state.matching {
                    self.update_searchable_items();
                    self.trigger_search();
                }
            }
            UIEvent::Shutdown => {
                return Ok(true);
            }
        }
        Ok(false)
    }
    
    fn handle_key(&mut self, key: KeyEvent) -> io::Result<bool> {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                if let Some(cancel_flag) = self.current_matcher.take() {
                    cancel_flag.store(true, Ordering::SeqCst);
                }
                return Ok(true);
            }
            
            (KeyCode::Enter, _) => {
                return Ok(true);
            }
            
            (KeyCode::Up, _) => {
                if self.state.viewport.selected() > 0 {
                    let new_selected = self.state.viewport.selected() - 1;
                    let viewport_height = self.get_viewport_height();
                    self.state.viewport.update_selection(new_selected, self.state.items.len(), viewport_height);
                }
            }
            (KeyCode::Down, _) => {
                if self.state.viewport.selected() + 1 < self.state.items.len() {
                    let new_selected = self.state.viewport.selected() + 1;
                    let viewport_height = self.get_viewport_height();
                    self.state.viewport.update_selection(new_selected, self.state.items.len(), viewport_height);
                }
            }
            
            (KeyCode::Char(c), _) => {
                self.state.query.push(c);
                self.state.last_query_change = Instant::now();
                self.state.pending_search = true;
            }
            (KeyCode::Backspace, _) => {
                self.state.query.pop();
                self.state.last_query_change = Instant::now();
                self.state.pending_search = true;
            }
            
            _ => {}
        }
        
        Ok(false)
    }
    
    fn get_viewport_height(&self) -> usize {
        if let Ok(size) = self.terminal.size() {
            size.height.saturating_sub(6) as usize
        } else {
            20
        }
    }
    
    /// Create atomic snapshot of current items for searching
    /// This prevents race conditions where items change during search
    fn update_searchable_items(&mut self) {
        self.item_pool.reset();
        let items = self.item_pool.take();
        
        let snapshot_len = items.len();
        
        // Arc wrapper prevents cloning entire Vec when sending to search thread
        let new_items: Vec<Arc<dyn crate::SkimItem>> = items.iter().cloned().collect();
        self.state.searchable_items = Arc::new(new_items);
        self.state.last_searchable_update = snapshot_len;
        
        // Update atomic counter for consistent UI display
        self.coordination.item_count.store(self.item_pool.len(), Ordering::SeqCst);
    }
    
    /// Launch search in background thread with cancellation support
    /// Previous search is cancelled and thread joined to prevent resource leaks
    fn trigger_search(&mut self) {
        // Cancel any existing search
        if let Some(cancel_flag) = self.current_matcher.take() {
            cancel_flag.store(true, Ordering::SeqCst);
        }
        
        // Join previous search thread to prevent resource leak
        if let Some(search_handle) = self.search_thread.take() {
            let _ = search_handle.join();
        }
        
        self.state.matching = true;
        self.state.search_processed = 0;
        self.state.search_total = self.state.searchable_items.len();
        
        let query = self.state.query.clone();
        let searchable_items = self.state.searchable_items.clone();
        let tx = self.normal_tx.clone();
        
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.current_matcher = Some(cancel_flag.clone());
        
        // Spawn search thread with timeout and progress reporting
        let search_handle = thread::spawn(move || {
            // Create temporary pool for this search
            let temp_pool = Arc::new(DeferDrop::new(ItemPool::new()));
            temp_pool.append((*searchable_items).clone());
            temp_pool.reset();
            
            let total_items = temp_pool.len();
            let results = Arc::new(Mutex::new(Vec::new()));
            let results_clone = results.clone();
            
            // Configure matcher for fuzzy search with case insensitive matching
            let rank_builder = Arc::new(crate::item::RankBuilder::new(vec![]));
            let engine_factory: std::rc::Rc<dyn crate::MatchEngineFactory> = {
                let fuzzy_engine = crate::engine::factory::ExactOrFuzzyEngineFactory::builder()
                    .exact_mode(false)
                    .rank_builder(rank_builder.clone())
                    .build();
                std::rc::Rc::new(crate::engine::factory::AndOrEngineFactory::new(fuzzy_engine))
            };
            
            let matcher = crate::matcher::Matcher::builder(engine_factory)
                .case(crate::CaseMatching::Ignore)
                .build();
            
            // Run search with callback to collect results
            let control = matcher.run(&query, temp_pool.clone(), move |items| {
                if let Ok(mut results) = results_clone.lock() {
                    results.clear();
                    let new_items = items.lock();
                    results.extend(new_items.iter().cloned());
                }
            });
            
            // Poll for completion with progress updates
            let start = Instant::now();
            let mut last_progress = Instant::now();
            
            while !control.stopped() && start.elapsed() < Duration::from_secs(10) {
                if cancel_flag.load(Ordering::SeqCst) {
                    control.kill();
                    return;
                }
                
                // Send progress updates every 100ms
                if last_progress.elapsed() >= Duration::from_millis(100) {
                    let processed = control.get_num_processed();
                    let _ = tx.try_send(UIEvent::SearchProgress(processed, total_items));
                    last_progress = Instant::now();
                }
                
                thread::sleep(Duration::from_millis(20));
            }
            
            // Ensure search completes
            if !control.stopped() {
                control.kill();
            }
            
            // Send final results
            if let Ok(final_results) = results.lock() {
                let _ = tx.try_send(UIEvent::MatchResults(final_results.clone()));
            }
        });
        
        self.search_thread = Some(search_handle);
    }
    
    /// Spawn high-priority input thread that never blocks UI
    /// Uses separate thread to prevent input lag during heavy operations
    fn spawn_input_thread(&mut self) {
        let input_tx = self.high_priority_tx.clone();
        let shutdown = self.coordination.shutdown.clone();
        self.input_thread = Some(thread::spawn(move || {
            while !shutdown.load(Ordering::SeqCst) {
                if event::poll(Duration::from_millis(INPUT_POLL_MS)).unwrap_or(false) {
                    if let Ok(event::Event::Key(key)) = event::read() {
                        if input_tx.try_send(UIEvent::KeyPress(key)).is_err() {
                            break; // Channel closed or full
                        }
                    }
                }
            }
        }));
    }
    
    /// Spawn reader thread to monitor input stream
    /// Batches items and notifies UI when ready for search
    fn spawn_reader_thread(&mut self) {
        let normal_tx = self.normal_tx.clone();
        let pool = self.item_pool.clone();
        let reader_control = self.reader.run(self.options.cmd.as_deref().unwrap_or(""));
        let shutdown = self.coordination.shutdown.clone();
        let pending_items = self.coordination.pending_items.clone();
        let item_count = self.coordination.item_count.clone();
        
        self.reader_thread = Some(thread::spawn(move || {
            while !shutdown.load(Ordering::SeqCst) {
                let items = reader_control.take();
                if !items.is_empty() {
                    pool.append(items);
                    item_count.store(pool.len(), Ordering::SeqCst);
                    // Prevent event flooding - only send if not already pending
                    if !pending_items.swap(true, Ordering::SeqCst) {
                        let _ = normal_tx.try_send(UIEvent::ItemsAvailable);
                    }
                }
                if reader_control.is_done() {
                    let _ = normal_tx.try_send(UIEvent::ReaderDone);
                    break;
                }
                thread::sleep(Duration::from_millis(READER_POLL_MS));
            }
        }));
    }
    
    fn draw(&mut self) -> io::Result<()> {
        self.terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(1),
                    Constraint::Length(1),
                ])
                .split(f.area());
            
            let query_widget = Paragraph::new(self.state.query.as_str())
                .block(Block::default().borders(Borders::ALL).title("Query"));
            f.render_widget(query_widget, chunks[0]);
            
            let _total_items = self.coordination.item_count.load(Ordering::SeqCst);
            let searchable_count = self.state.searchable_items.len();
            
            let status_text = if self.state.matching {
                if self.state.search_total > 0 {
                    let progress = (self.state.search_processed as f64 / self.state.search_total as f64 * 100.0) as u16;
                    format!("🔍 Searching '{}' - {}% ({}/{})", self.state.query, progress, self.state.search_processed, self.state.search_total)
                } else {
                    format!("🔍 Searching '{}'...", self.state.query)
                }
            } else if self.state.reading {
                format!("📖 Loading... ({} items)", searchable_count)
            } else {
                format!("✅ {} matches found", self.state.items.len())
            };
            
            let status_color = if self.state.matching { Color::Yellow } else { Color::Green };
            let status_widget = Paragraph::new(status_text)
                .block(Block::default().borders(Borders::ALL).title("Status"))
                .style(Style::default().fg(status_color));
            
            f.render_widget(status_widget, chunks[1]);
            
            let items_chunk = &chunks[2];
            let viewport_height = items_chunk.height.saturating_sub(2) as usize;
            let viewport_offset = self.state.viewport.offset();
            let end_idx = (viewport_offset + viewport_height).min(self.state.items.len());
            
            let visible_items: Vec<ListItem> = self.state.items[viewport_offset..end_idx]
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let actual_idx = viewport_offset + i;
                    let style = if actual_idx == self.state.viewport.selected() {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    };
                    ListItem::new(item.item.text()).style(style)
                })
                .collect();
            
            let items_widget = List::new(visible_items)
                .block(Block::default().borders(Borders::ALL).title("Items"));
            f.render_widget(items_widget, *items_chunk);
            
            let total_count = self.coordination.item_count.load(Ordering::SeqCst);
            let status = format!("{}/{} items", self.state.items.len(), total_count);
            
            let status_widget = Paragraph::new(status);
            f.render_widget(status_widget, chunks[3]);
        })?;
        
        Ok(())
    }
    
}

impl<'a> Drop for UICoordinator<'a> {
    fn drop(&mut self) {
        self.coordination.shutdown.store(true, Ordering::SeqCst);
        
        if let Some(cancel_flag) = self.current_matcher.take() {
            cancel_flag.store(true, Ordering::SeqCst);
        }
        
        let _ = self.high_priority_tx.try_send(UIEvent::Shutdown);
        let _ = self.normal_tx.try_send(UIEvent::Shutdown);
        
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        
        if let Some(handle) = self.input_thread.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.reader_thread.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.search_thread.take() {
            let _ = handle.join();
        }
    }
}

impl<'a> UICoordinator<'a> {
    /// Main event processing loop with priority-based handling
    fn run_event_loop(&mut self) -> io::Result<()> {
        let mut last_frame = Instant::now();
        loop {
            // Process high priority events (user input)
            let processed_input = self.process_high_priority_events()?;
            
            if self.state.should_exit {
                break;
            }
            
            if processed_input {
                self.state.needs_redraw = true;
            }
            
            // Handle debounced search triggering
            self.process_pending_search();
            
            // Process background events (lower priority)
            self.process_background_events()?;
            
            // Rate-limited drawing for smooth 60fps
            self.update_display(&mut last_frame)?;
        }
        Ok(())
    }
    
    /// Process user input events (highest priority)
    fn process_high_priority_events(&mut self) -> io::Result<bool> {
        let mut processed_input = false;
        while let Ok(event) = self.high_priority_rx.try_recv() {
            processed_input = true;
            if self.handle_event(event)? {
                self.state.should_exit = true;
            }
        }
        Ok(processed_input)
    }
    
    /// Trigger search if debounce time has elapsed
    fn process_pending_search(&mut self) {
        if self.state.pending_search && 
           self.state.last_query_change.elapsed() >= Duration::from_millis(50) { // Debounce search
            self.state.pending_search = false;
            self.trigger_search();
        }
    }
    
    /// Process background events (items available, search results, etc.)
    fn process_background_events(&mut self) -> io::Result<()> {
        match self.normal_rx.recv_timeout(Duration::from_millis(10)) {
            Ok(event) => {
                self.handle_event(event)?;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.state.should_exit = true; // All senders dropped
            }
        }
        Ok(())
    }
    
    /// Update display at controlled frame rate
    fn update_display(&mut self, last_frame: &mut Instant) -> io::Result<()> {
        let now = Instant::now();
        if self.state.needs_redraw && now.duration_since(*last_frame) >= Duration::from_millis(16) { // ~60fps
            self.draw()?;
            self.state.needs_redraw = false;
            *last_frame = now;
        }
        Ok(())
    }
}
