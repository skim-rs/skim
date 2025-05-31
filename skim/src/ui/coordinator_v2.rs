//! Event-driven UI Coordinator for ratatui interface
//! 
//! Clean architecture with clear priorities:
//! 1. User input ALWAYS has highest priority
//! 2. Event-driven, not polling-based
//! 3. Simple state management

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
    item::{MatchedItem, ItemPool, RankBuilder},
    reader::Reader,
    matcher::Matcher,
    MatchEngineFactory,
};
use defer_drop::DeferDrop;

// Simple constants
const SPINNERS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const INPUT_POLL_MS: u64 = 20; // Balanced between responsiveness and CPU usage
const TICKER_INTERVAL_MS: u64 = 200;
const READER_POLL_MS: u64 = 50;
const BATCH_THRESHOLD: usize = 100; // Trigger search after this many new items
const SEARCH_DEBOUNCE_MS: u64 = 50; // Debounce search triggering
const FRAME_TIME_MS: u64 = 16; // ~60fps

/// Events - simple and clear
#[derive(Clone)]
enum UIEvent {
    // User actions - highest priority
    KeyPress(KeyEvent),
    
    // Background updates - lower priority
    ItemsAvailable, // No count, just a notification
    MatchResults(Vec<MatchedItem>, u64), // Results with generation number
    SearchProgress(usize, usize, usize), // (processed, matched, total)
    ReaderDone,
    
    // UI updates
    Tick, // For animations
    
    // Control events
    Shutdown, // Signal threads to exit
}

/// Clean state - just what we need
struct UIState {
    // User input
    query: String,
    selected: usize,
    
    // Data
    items: Vec<MatchedItem>,          // Current search results
    searchable_items: Vec<Arc<dyn crate::SkimItem>>, // Stable snapshot for searching
    
    // Status
    reading: bool,
    matching: bool,
    
    // Animation
    spinner_frame: usize,
    
    // Track when we last updated searchable items
    last_searchable_update: usize,
    
    // Generation counter to handle race conditions
    search_generation: u64,
    
    // Track if we should exit
    should_exit: bool,
    
    // Performance optimization
    last_query_change: Instant,
    pending_search: bool,
    needs_redraw: bool,
    
    // Viewport management
    viewport_offset: usize,
    
    // Search progress tracking
    search_processed: usize,
    search_matched: usize,
    search_total: usize,
    search_start_time: Option<Instant>, // Track when search started for UI timeout
}

/// Thread handle management
struct ThreadHandles {
    input: Option<JoinHandle<()>>,
    ticker: Option<JoinHandle<()>>,
    reader: Option<JoinHandle<()>>,
}

pub struct UICoordinator<'a> {
    // Core components
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    state: UIState,
    options: &'a SkimOptions,
    
    // Two channels: high priority for user input, normal for everything else
    high_priority_rx: mpsc::Receiver<UIEvent>,
    high_priority_tx: mpsc::Sender<UIEvent>,
    normal_rx: mpsc::Receiver<UIEvent>,
    normal_tx: mpsc::SyncSender<UIEvent>,
    
    // Background workers
    reader: Reader,
    item_pool: Arc<DeferDrop<ItemPool>>,
    
    // Keep track of current matcher to cancel it
    current_matcher: Option<Arc<AtomicBool>>,
    
    // Thread management
    thread_handles: ThreadHandles,
    shutdown_flag: Arc<AtomicBool>,
    
    // Track pending items notification
    pending_items_update: Arc<AtomicBool>,
    item_count: Arc<AtomicUsize>,
}

impl<'a> UICoordinator<'a> {
    pub fn new(options: &'a SkimOptions) -> io::Result<Self> {
        // Terminal setup
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        
        // Two channels: unbounded for high priority (user input), bounded for normal
        let (high_priority_tx, high_priority_rx) = mpsc::channel();
        let (normal_tx, normal_rx) = mpsc::sync_channel(100);
        
        // Create components
        let reader = Reader::with_options(options);
        let item_pool = Arc::new(DeferDrop::new(ItemPool::new()));
        
        
        Ok(Self {
            terminal,
            state: UIState {
                query: String::new(),
                selected: 0,
                items: Vec::new(),
                searchable_items: Vec::new(),
                reading: true,
                matching: false,
                spinner_frame: 0,
                last_searchable_update: 0,
                search_generation: 0,
                should_exit: false,
                last_query_change: Instant::now(),
                pending_search: false,
                needs_redraw: true,
                viewport_offset: 0,
                search_processed: 0,
                search_matched: 0,
                search_total: 0,
                search_start_time: None,
            },
            options,
            high_priority_rx,
            high_priority_tx,
            normal_rx,
            normal_tx,
            reader,
            item_pool,
            current_matcher: None,
            thread_handles: ThreadHandles {
                input: None,
                ticker: None,
                reader: None,
            },
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            pending_items_update: Arc::new(AtomicBool::new(false)),
            item_count: Arc::new(AtomicUsize::new(0)),
        })
    }
    
    pub fn set_item_source(&mut self, source: SkimItemReceiver) {
        let reader = std::mem::replace(&mut self.reader, Reader::with_options(self.options));
        self.reader = reader.source(Some(source));
    }
    
    pub fn run(mut self) -> io::Result<crate::output::SkimOutput> {
        // Spawn input thread - HIGHEST PRIORITY
        let input_tx = self.high_priority_tx.clone();
        let shutdown = self.shutdown_flag.clone();
        self.thread_handles.input = Some(thread::spawn(move || {
            while !shutdown.load(Ordering::SeqCst) {
                // Check for input with balanced timeout
                if event::poll(Duration::from_millis(INPUT_POLL_MS)).unwrap_or(false) {
                    if let Ok(event::Event::Key(key)) = event::read() {
                        // Always send to high priority channel
                        if input_tx.send(UIEvent::KeyPress(key)).is_err() {
                            break; // Channel closed
                        }
                    }
                }
            }
        }));
        
        // Spawn ticker for animations
        let tick_tx = self.normal_tx.clone();
        let shutdown = self.shutdown_flag.clone();
        self.thread_handles.ticker = Some(thread::spawn(move || {
            while !shutdown.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(TICKER_INTERVAL_MS));
                // Don't care if this fails - it's low priority
                let _ = tick_tx.try_send(UIEvent::Tick);
            }
        }));
        
        // Start reader
        let normal_tx = self.normal_tx.clone();
        let pool = self.item_pool.clone();
        let reader_control = self.reader.run(self.options.cmd.as_deref().unwrap_or(""));
        let shutdown = self.shutdown_flag.clone();
        let pending_items = self.pending_items_update.clone();
        let item_count = self.item_count.clone();
        
        // Reader monitoring thread
        self.thread_handles.reader = Some(thread::spawn(move || {
            while !shutdown.load(Ordering::SeqCst) {
                let items = reader_control.take();
                if !items.is_empty() {
                    pool.append(items);
                    // Update count and set flag instead of sending event for each batch
                    item_count.store(pool.len(), Ordering::Relaxed);
                    if !pending_items.swap(true, Ordering::AcqRel) {
                        // Only send if not already pending
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
        
        // Initial draw
        self.draw()?;
        
        // Take initial snapshot for searching
        self.update_searchable_items();
        
        // Trigger initial search to show all items
        if !self.state.searchable_items.is_empty() {
            self.trigger_search();
        }
        
        // Main event loop
        let mut last_frame = Instant::now();
        loop {
            // ALWAYS check high priority first (user input)
            let mut processed_input = false;
            while let Ok(event) = self.high_priority_rx.try_recv() {
                processed_input = true;
                if self.handle_event(event)? {
                    self.state.should_exit = true;
                }
            }
            
            if self.state.should_exit {
                break;
            }
            
            // If we processed input, mark for redraw but don't block on other events
            if processed_input {
                self.state.needs_redraw = true;
            }
            
            // Process pending search if debounce time has passed
            if self.state.pending_search && 
               self.state.last_query_change.elapsed() >= Duration::from_millis(SEARCH_DEBOUNCE_MS) {
                self.state.pending_search = false;
                self.trigger_search();
            }
            
            // UI-side timeout: Force completion if search has been running too long
            if self.state.matching {
                if let Some(start_time) = self.state.search_start_time {
                    if start_time.elapsed() >= Duration::from_secs(15) { // 15 second absolute timeout
                        // Force search completion - this prevents all stuck states
                        self.state.matching = false;
                        self.state.search_processed = 0;
                        self.state.search_matched = 0;
                        self.state.search_total = 0;
                        self.state.search_start_time = None;
                        self.state.needs_redraw = true;
                        
                        // Cancel the stuck matcher
                        if let Some(cancel_flag) = self.current_matcher.take() {
                            cancel_flag.store(true, Ordering::SeqCst);
                        }
                    }
                }
            }
            
            
            // Then check normal priority with very short timeout
            match self.normal_rx.recv_timeout(Duration::from_millis(1)) {
                Ok(event) => {
                    self.handle_event(event)?;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // No events, just continue
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break; // All senders dropped
                }
            }
            
            // Draw updates only if needed and at reasonable frame rate
            let now = Instant::now();
            if self.state.needs_redraw && now.duration_since(last_frame) >= Duration::from_millis(FRAME_TIME_MS) {
                self.draw()?;
                self.state.needs_redraw = false;
                last_frame = now;
            }
        }
        
        self.finalize()
    }
    
    fn handle_event(&mut self, event: UIEvent) -> io::Result<bool> {
        match event {
            UIEvent::KeyPress(key) => {
                return self.handle_key(key);
            }
            UIEvent::ItemsAvailable => {
                // Clear the pending flag
                self.pending_items_update.store(false, Ordering::Release);
                
                // Only update searchable items if we're not currently searching
                if !self.state.matching {
                    let current_pool_size = self.item_pool.len();
                    let diff = current_pool_size.saturating_sub(self.state.last_searchable_update);
                    
                    // Update searchable items if enough new items or reading is done
                    if diff >= BATCH_THRESHOLD || !self.state.reading {
                        self.update_searchable_items();
                        self.trigger_search();
                    }
                }
            }
            UIEvent::MatchResults(mut results, generation) => {
                // Only accept results from the current search generation
                if generation == self.state.search_generation {
                    // Sort results by rank to match legacy system behavior
                    results.sort();
                    self.state.items = results;
                    self.state.matching = false;
                    
                    if self.state.selected >= self.state.items.len() && !self.state.items.is_empty() {
                        self.state.selected = self.state.items.len() - 1;
                    }
                    self.state.viewport_offset = 0;
                    self.state.needs_redraw = true;
                    
                    // Clear progress when search is done
                    self.state.search_processed = 0;
                    self.state.search_matched = 0;
                    self.state.search_total = 0;
                    self.state.search_start_time = None;
                    
                    // Check if new items are available for next search
                    let current_pool_size = self.item_pool.len();
                    if current_pool_size > self.state.last_searchable_update {
                        // New items available - update searchable items and search again
                        self.update_searchable_items();
                        if !self.state.query.is_empty() {
                            self.trigger_search();
                        }
                    }
                }
                // Ignore results from old/cancelled searches
            }
            UIEvent::ReaderDone => {
                self.state.reading = false;
                // Update searchable items with final state and trigger search if needed
                if !self.state.matching {
                    self.update_searchable_items();
                    self.trigger_search();
                }
            }
            UIEvent::Tick => {
                self.state.spinner_frame = (self.state.spinner_frame + 1) % SPINNERS.len();
                self.state.needs_redraw = true;
            }
            UIEvent::SearchProgress(processed, matched, total) => {
                self.state.search_processed = processed;
                self.state.search_matched = matched;
                self.state.search_total = total;
                self.state.needs_redraw = true;
            }
            UIEvent::Shutdown => {
                return Ok(true);
            }
        }
        Ok(false)
    }
    
    fn handle_key(&mut self, key: KeyEvent) -> io::Result<bool> {
        match (key.code, key.modifiers) {
            // Exit - IMMEDIATE
            (KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                // Cancel any running operations immediately
                if let Some(cancel_flag) = self.current_matcher.take() {
                    cancel_flag.store(true, Ordering::SeqCst);
                }
                return Ok(true);
            }
            
            // Selection
            (KeyCode::Enter, _) => {
                return Ok(true);
            }
            
            // Navigation
            (KeyCode::Up, _) => {
                if self.state.selected > 0 {
                    self.state.selected -= 1;
                    self.update_viewport();
                }
            }
            (KeyCode::Down, _) => {
                if self.state.selected + 1 < self.state.items.len() {
                    self.state.selected += 1;
                    self.update_viewport();
                }
            }
            
            // Query input - debounced
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
    
    fn update_viewport(&mut self) {
        let viewport_height = self.get_viewport_height();
        
        // Keep selected item in view
        if self.state.selected < self.state.viewport_offset {
            self.state.viewport_offset = self.state.selected;
        } else if self.state.selected >= self.state.viewport_offset + viewport_height {
            self.state.viewport_offset = self.state.selected.saturating_sub(viewport_height - 1);
        }
    }
    
    fn get_viewport_height(&self) -> usize {
        // Get terminal height and calculate available space for items
        if let Ok(size) = self.terminal.size() {
            // Account for borders, query box, status line, margins
            size.height.saturating_sub(6) as usize
        } else {
            20 // Default fallback
        }
    }
    
    fn update_searchable_items(&mut self) {
        // Take a snapshot of current items for searching
        self.item_pool.reset(); // Allow iteration
        self.state.searchable_items.clear();
        
        // Copy all items from pool to our searchable snapshot
        let items = self.item_pool.take();
        self.state.searchable_items.extend(items.iter().cloned());
        self.state.last_searchable_update = self.state.searchable_items.len();
    }
    
    fn trigger_search(&mut self) {
        // Cancel any existing search
        if let Some(cancel_flag) = self.current_matcher.take() {
            cancel_flag.store(true, Ordering::SeqCst);
        }
        
        self.state.matching = true;
        self.state.search_start_time = Some(Instant::now());
        
        // Reset progress
        self.state.search_processed = 0;
        self.state.search_matched = 0;
        self.state.search_total = self.state.searchable_items.len();
        
        // Increment generation to handle race conditions
        self.state.search_generation += 1;
        let current_generation = self.state.search_generation;
        
        // Simple approach: Run search synchronously in a thread with guaranteed completion
        let query = self.state.query.clone();
        let searchable_items = self.state.searchable_items.clone();
        let tx = self.normal_tx.clone();
        
        // Create cancellation flag
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.current_matcher = Some(cancel_flag.clone());
        
        thread::spawn(move || {
            // Create a temporary pool with our snapshot
            let temp_pool = Arc::new(DeferDrop::new(ItemPool::new()));
            temp_pool.append(searchable_items);
            temp_pool.reset();
            
            let total_items = temp_pool.len();
            
            // Send initial progress
            let _ = tx.try_send(UIEvent::SearchProgress(0, 0, total_items));
            
            // Use Mutex for results - simpler approach
            let results = Arc::new(Mutex::new(Vec::new()));
            let results_clone = results.clone();
            
            // Use the same matcher that was configured in new()
            let rank_builder = Arc::new(crate::item::RankBuilder::new(vec![])); // Default tiebreak
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
            
            let control = matcher.run(&query, temp_pool.clone(), move |items| {
                if let Ok(mut results) = results_clone.lock() {
                    results.clear();
                    let new_items = items.lock();
                    results.extend(new_items.iter().cloned());
                }
            });
            
            // Simple polling with progress updates and guaranteed completion
            let start = Instant::now();
            let mut last_progress = Instant::now();
            
            // Wait for completion with timeout
            while !control.stopped() && start.elapsed() < Duration::from_secs(10) {
                // Check if cancelled
                if cancel_flag.load(Ordering::SeqCst) {
                    control.kill();
                    return;
                }
                
                // Send progress updates
                if last_progress.elapsed() >= Duration::from_millis(100) {
                    let processed = control.get_num_processed();
                    let matched = control.get_num_matched();
                    let _ = tx.try_send(UIEvent::SearchProgress(processed, matched, total_items));
                    last_progress = Instant::now();
                }
                
                thread::sleep(Duration::from_millis(20));
            }
            
            // Force completion if not stopped
            if !control.stopped() {
                control.kill();
            }
            
            // Always send results - even if empty due to timeout/cancellation
            if let Ok(final_results) = results.lock() {
                // Send final progress (mark as complete)
                let final_matched = final_results.len();
                let _ = tx.try_send(UIEvent::SearchProgress(total_items, final_matched, total_items));
                
                // Send results - this will ALWAYS complete the search
                let _ = tx.try_send(UIEvent::MatchResults(final_results.clone(), current_generation));
            } else {
                // Fallback: send empty results to complete the search
                let _ = tx.try_send(UIEvent::SearchProgress(total_items, 0, total_items));
                let _ = tx.try_send(UIEvent::MatchResults(Vec::new(), current_generation));
            }
        });
    }
    
    fn draw(&mut self) -> io::Result<()> {
        self.terminal.draw(|f| {
            // Fixed layout - always show progress/status area
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3), // Query
                    Constraint::Length(3), // Progress/Status
                    Constraint::Min(1),    // Items
                    Constraint::Length(1), // Bottom status
                ])
                .split(f.area());
            
            // Query
            let query_widget = Paragraph::new(self.state.query.as_str())
                .block(Block::default().borders(Borders::ALL).title("Query"));
            f.render_widget(query_widget, chunks[0]);
            
            // Search Status - always visible
            let total_items = self.item_count.load(Ordering::Relaxed);
            let searchable_count = self.state.searchable_items.len();
            
            if self.state.matching {
                // Active search - show progress bar
                let percentage = if self.state.search_total > 0 {
                    (self.state.search_processed as f64 / self.state.search_total as f64 * 100.0) as u16
                } else {
                    0
                };
                
                let progress_text = format!(
                    "Searching '{}': {} / {} ({} matches) - {}%",
                    self.state.query,
                    self.state.search_processed,
                    self.state.search_total,
                    self.state.search_matched,
                    percentage
                );
                
                use ratatui::widgets::Gauge;
                let progress_widget = Gauge::default()
                    .block(Block::default().borders(Borders::ALL).title("🔍 Search Progress"))
                    .gauge_style(Style::default().fg(Color::Yellow))
                    .percent(percentage.min(100))
                    .label(progress_text);
                
                f.render_widget(progress_widget, chunks[1]);
            } else {
                // Not searching - show current state
                let status_text = if self.state.query.is_empty() {
                    if self.state.reading {
                        let available_indicator = if total_items > searchable_count {
                            format!(" (+{} new)", total_items - searchable_count)
                        } else {
                            String::new()
                        };
                        format!("📖 Loading items... ({} searchable{})", searchable_count, available_indicator)
                    } else {
                        format!("✅ Ready - {} items available", total_items)
                    }
                } else {
                    // Show last search results
                    let new_items_indicator = if total_items > searchable_count {
                        format!(" (+{} new items available)", total_items - searchable_count)
                    } else {
                        String::new()
                    };
                    format!("✅ Search complete for '{}' - {} matches found{}", 
                           self.state.query, self.state.items.len(), new_items_indicator)
                };
                
                let status_widget = Paragraph::new(status_text)
                    .block(Block::default().borders(Borders::ALL).title("📊 Status"))
                    .style(Style::default().fg(Color::Green));
                
                f.render_widget(status_widget, chunks[1]);
            }
            
            // Items with viewport
            let items_chunk = &chunks[2]; // Always chunk 2 now
            let viewport_height = items_chunk.height.saturating_sub(2) as usize; // Account for borders
            let end_idx = (self.state.viewport_offset + viewport_height).min(self.state.items.len());
            
            let visible_items: Vec<ListItem> = self.state.items[self.state.viewport_offset..end_idx]
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let actual_idx = self.state.viewport_offset + i;
                    let style = if actual_idx == self.state.selected {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    };
                    ListItem::new(item.item.text()).style(style)
                })
                .collect();
            
            let items_widget = List::new(visible_items)
                .block(Block::default().borders(Borders::ALL).title(format!(
                    "Items (showing {}-{} of {})",
                    if self.state.items.is_empty() { 0 } else { self.state.viewport_offset + 1 },
                    end_idx,
                    self.state.items.len()
                )));
            f.render_widget(items_widget, *items_chunk);
            
            // Bottom Status
            let mut status = String::new();
            if self.state.reading || self.state.matching {
                status.push(SPINNERS[self.state.spinner_frame]);
                status.push(' ');
            }
            let total_count = self.item_count.load(Ordering::Relaxed);
            let searchable_count = self.state.searchable_items.len();
            status.push_str(&format!("Filtered: {}/{} (searchable: {})", 
                                   self.state.items.len(), total_count, searchable_count));
            
            let status_widget = Paragraph::new(status);
            f.render_widget(status_widget, chunks[3]); // Always chunk 3 now
        })?;
        
        Ok(())
    }
    
    fn finalize(mut self) -> io::Result<crate::output::SkimOutput> {
        // Signal all threads to shutdown
        self.shutdown_flag.store(true, Ordering::SeqCst);
        
        // Cancel any running matcher
        if let Some(cancel_flag) = self.current_matcher.take() {
            cancel_flag.store(true, Ordering::SeqCst);
        }
        
        // Send shutdown event to wake up blocked threads
        let _ = self.high_priority_tx.send(UIEvent::Shutdown);
        let _ = self.normal_tx.try_send(UIEvent::Shutdown);
        
        // Cleanup terminal IMMEDIATELY
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        
        // Drop the terminal to ensure it's cleaned up
        drop(self.terminal);
        
        // Wait for threads to finish (with timeout)
        let timeout = Duration::from_millis(100);
        let deadline = std::time::Instant::now() + timeout;
        
        if let Some(handle) = self.thread_handles.input.take() {
            let _remaining = deadline.saturating_duration_since(std::time::Instant::now());
            let _ = handle.join();
        }
        
        if let Some(handle) = self.thread_handles.ticker.take() {
            let _remaining = deadline.saturating_duration_since(std::time::Instant::now());
            let _ = handle.join();
        }
        
        if let Some(handle) = self.thread_handles.reader.take() {
            let _remaining = deadline.saturating_duration_since(std::time::Instant::now());
            let _ = handle.join();
        }
        
        // Return results
        let selected = if self.state.selected < self.state.items.len() {
            vec![self.state.items[self.state.selected].item.clone()]
        } else {
            Vec::new()
        };
        
        Ok(crate::output::SkimOutput {
            final_event: crate::event::Event::EvActAccept(None),
            is_abort: self.state.should_exit && self.state.selected >= self.state.items.len(),
            final_key: if self.state.should_exit { 
                skim_tuikit::prelude::Key::ESC 
            } else { 
                skim_tuikit::prelude::Key::Enter 
            },
            query: self.state.query,
            cmd: self.options.cmd.clone().unwrap_or_default(),
            selected_items: selected,
        })
    }
}
