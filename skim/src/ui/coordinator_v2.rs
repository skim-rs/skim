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
use std::time::Duration;

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
const MATCHER_TIMEOUT_SECS: u64 = 5;
const BATCH_THRESHOLD: usize = 100; // Trigger search after this many new items

/// Events - simple and clear
#[derive(Clone)]
enum UIEvent {
    // User actions - highest priority
    KeyPress(KeyEvent),
    
    // Background updates - lower priority
    ItemsAvailable, // No count, just a notification
    MatchResults(Vec<MatchedItem>, u64), // Results with generation number
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
    items: Vec<MatchedItem>,
    
    // Status
    reading: bool,
    matching: bool,
    
    // Animation
    spinner_frame: usize,
    
    // Track last item count to avoid redundant searches
    last_search_item_count: usize,
    
    // Generation counter to handle race conditions
    search_generation: u64,
    
    // Track if we should exit
    should_exit: bool,
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
    matcher: Matcher,
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
        
        // Create matcher (using same logic as legacy system)
        let rank_builder = Arc::new(RankBuilder::new(options.tiebreak.clone()));
            
        let engine_factory: std::rc::Rc<dyn MatchEngineFactory> = if options.regex {
            std::rc::Rc::new(crate::engine::factory::RegexEngineFactory::builder()
                .rank_builder(rank_builder.clone())
                .build())
        } else {
            // For fuzzy mode, wrap ExactOrFuzzyEngineFactory with AndOrEngineFactory
            let fuzzy_engine = crate::engine::factory::ExactOrFuzzyEngineFactory::builder()
                .exact_mode(options.exact)
                .rank_builder(rank_builder.clone())
                .build();
            std::rc::Rc::new(crate::engine::factory::AndOrEngineFactory::new(fuzzy_engine))
        };
        
        let matcher = Matcher::builder(engine_factory)
            .case(options.case)
            .build();
        
        Ok(Self {
            terminal,
            state: UIState {
                query: String::new(),
                selected: 0,
                items: Vec::new(),
                reading: true,
                matching: false,
                spinner_frame: 0,
                last_search_item_count: 0,
                search_generation: 0,
                should_exit: false,
            },
            options,
            high_priority_rx,
            high_priority_tx,
            normal_rx,
            normal_tx,
            reader,
            matcher,
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
        
        // Trigger initial search to show all items
        if !self.item_pool.is_empty() {
            self.trigger_search();
        }
        
        // Main event loop
        loop {
            // ALWAYS check high priority first (user input)
            while let Ok(event) = self.high_priority_rx.try_recv() {
                if self.handle_event(event)? {
                    self.state.should_exit = true;
                }
            }
            
            if self.state.should_exit {
                break;
            }
            
            // Then check normal priority with timeout
            match self.normal_rx.recv_timeout(Duration::from_millis(10)) {
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
            
            // Draw updates
            self.draw()?;
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
                
                // Get actual count from atomic
                let actual_count = self.item_count.load(Ordering::Relaxed);
                
                // Trigger search if:
                // 1. We're not already matching
                // 2. Item count changed significantly or reading is done
                if !self.state.matching {
                    let diff = actual_count.saturating_sub(self.state.last_search_item_count);
                    if diff >= BATCH_THRESHOLD || !self.state.reading {
                        self.state.last_search_item_count = actual_count;
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
                }
                // Ignore results from old/cancelled searches
            }
            UIEvent::ReaderDone => {
                self.state.reading = false;
                // Trigger final search with all items
                if !self.state.matching {
                    self.trigger_search();
                }
            }
            UIEvent::Tick => {
                self.state.spinner_frame = (self.state.spinner_frame + 1) % SPINNERS.len();
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
                }
            }
            (KeyCode::Down, _) => {
                if self.state.selected + 1 < self.state.items.len() {
                    self.state.selected += 1;
                }
            }
            
            // Query input - INSTANT response
            (KeyCode::Char(c), _) => {
                self.state.query.push(c);
                self.state.last_search_item_count = self.item_count.load(Ordering::Relaxed);
                self.trigger_search();
            }
            (KeyCode::Backspace, _) => {
                self.state.query.pop();
                self.state.last_search_item_count = self.item_count.load(Ordering::Relaxed);
                self.trigger_search();
            }
            
            _ => {}
        }
        
        Ok(false)
    }
    
    
    fn trigger_search(&mut self) {
        // Cancel any existing search by setting the flag
        if let Some(cancel_flag) = self.current_matcher.take() {
            cancel_flag.store(true, Ordering::SeqCst); // Consistent ordering
        }
        
        self.state.matching = true;
        
        // Increment generation to handle race conditions
        self.state.search_generation += 1;
        let current_generation = self.state.search_generation;
        
        // Create cancellation flag
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.current_matcher = Some(cancel_flag.clone());
        
        // Run matcher asynchronously
        let query = self.state.query.clone();
        let pool = self.item_pool.clone();
        let tx = self.normal_tx.clone();
        
        // Use Mutex instead of SpinLock for results
        let results = Arc::new(Mutex::new(Vec::new()));
        let results_clone = results.clone();
        
        // Note: We cannot synchronize pool.reset() with reader's append() without
        // modifying the ItemPool implementation. This is a known limitation.
        // The reset() call allows the matcher to see all items in the pool.
        pool.reset();
        
        let control = self.matcher.run(&query, pool, move |items| {
            if let Ok(mut results) = results_clone.lock() {
                results.clear();
                let new_items = items.lock();
                results.extend(new_items.iter().cloned());
            }
        });
        
        // Monitor matcher in a separate thread
        thread::spawn(move || {
            // Poll for completion or cancellation
            let start = std::time::Instant::now();
            loop {
                // Check if matcher finished naturally
                if control.stopped() {
                    break;
                }
                
                // Check if cancelled
                if cancel_flag.load(Ordering::SeqCst) {
                    control.kill();
                    return; // Don't send results
                }
                
                // Check timeout
                if start.elapsed() >= Duration::from_secs(MATCHER_TIMEOUT_SECS) {
                    control.kill();
                    return; // Don't send results
                }
                
                thread::sleep(Duration::from_millis(10)); // Balanced polling
            }
            
            // Send results only if not cancelled
            if let Ok(final_results) = results.lock() {
                let _ = tx.try_send(UIEvent::MatchResults(final_results.clone(), current_generation));
            }
        });
    }
    
    fn draw(&mut self) -> io::Result<()> {
        self.terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3), // Query
                    Constraint::Min(1),    // Items
                    Constraint::Length(1), // Status
                ])
                .split(f.area());
            
            // Query
            let query_widget = Paragraph::new(self.state.query.as_str())
                .block(Block::default().borders(Borders::ALL).title("Query"));
            f.render_widget(query_widget, chunks[0]);
            
            // Items
            let items: Vec<ListItem> = self.state.items
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let style = if i == self.state.selected {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    };
                    ListItem::new(item.item.text()).style(style)
                })
                .collect();
            
            let items_widget = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Items"));
            f.render_widget(items_widget, chunks[1]);
            
            // Status
            let mut status = String::new();
            if self.state.reading || self.state.matching {
                status.push(SPINNERS[self.state.spinner_frame]);
                status.push(' ');
            }
            let actual_count = self.item_count.load(Ordering::Relaxed);
            status.push_str(&format!("{}/{}", self.state.items.len(), actual_count));
            
            let status_widget = Paragraph::new(status);
            f.render_widget(status_widget, chunks[2]);
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
