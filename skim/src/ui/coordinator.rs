//! UI Coordinator for the ratatui-based interface
//!
//! This module implements an event-driven UI coordinator that integrates with
//! skim's existing Reader and Matcher threads. It follows these principles:
//! 
//! 1. UI thread is display-only - never blocks on background operations
//! 2. Input is polled with 1ms timeout for immediate responsiveness
//! 3. No custom background threads - uses existing skim infrastructure
//! 4. Immediate visual feedback - query updates shown without waiting

use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
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
    reader::{Reader, ReaderControl},
    matcher::{Matcher, MatcherControl},
    engine::factory::{AndOrEngineFactory, ExactOrFuzzyEngineFactory, RegexEngineFactory},
    MatchEngineFactory,
};
use defer_drop::DeferDrop;
use crate::spinlock::SpinLock;
use std::rc::Rc;

// Spinner animation constants
const SPINNERS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const SPINNER_DURATION_MS: u64 = 200;

// Timing constants for UI responsiveness
const LOADING_INDICATOR_DELAY_MS: u64 = 50;  // Show spinner/progress after this delay
const QUERY_DEBOUNCE_READING_MS: u64 = 150;  // Query debounce during reading
const QUERY_DEBOUNCE_IDLE_MS: u64 = 10;      // Query debounce when idle
const MATCHER_RESTART_READING_MS: u64 = 300; // Matcher restart throttle during reading  
const MATCHER_RESTART_IDLE_MS: u64 = 10;     // Matcher restart throttle when idle
const READER_CHECK_INTERVAL_MS: u64 = 200;   // How often to check for new items during reading
const MATCHER_CHECK_INTERVAL_MS: u64 = 150;  // How often to check matcher results during reading

// Sleep durations for CPU efficiency
const SLEEP_READING_MS: u64 = 1;              // Sleep during reading
const SLEEP_IDLE_NANOS: u64 = 500_000;       // Sleep when idle (0.5ms)

// UI refresh rates
const UI_REFRESH_RATE_FPS: u64 = 60;          // Target FPS for normal UI
const UI_REFRESH_INTERVAL_MS: u64 = 1000 / UI_REFRESH_RATE_FPS; // 16ms

/// Lightweight UI state for immediate visual updates
struct UIState {
    // Visual state - updated immediately
    display_query: String,
    selected_index: usize,
    matched_items: Vec<MatchedItem>,
    total_items: usize,
    
    // Status flags
    is_running: bool,
    is_reading: bool,
    needs_redraw: bool,
    
    // Timing for throttled operations
    last_draw: Instant,
    last_query_sent: String,
    last_matched_count: usize,
    last_matcher_restart: Instant,
    query_debounce_timer: Instant,
    
    // Matcher progress tracking (for loading indicator)
    processed_items: usize,
    matcher_running: bool,
    read_start_time: Instant,
    match_start_time: Instant,
}

impl UIState {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            display_query: String::new(),
            selected_index: 0,
            matched_items: Vec::new(),
            total_items: 0,
            is_running: true,
            is_reading: true,
            needs_redraw: true, // Initial draw needed
            last_draw: now,
            last_query_sent: String::new(),
            last_matched_count: 0,
            last_matcher_restart: now,
            query_debounce_timer: now,
            processed_items: 0,
            matcher_running: false,
            read_start_time: now,
            match_start_time: now,
        }
    }
}

pub struct UICoordinator<'a> {
    // === UI Components ===
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    state: UIState,
    draw_interval: Duration,
    
    // === Configuration ===
    options: &'a SkimOptions,
    use_regex: bool,
    
    // === Reader Integration ===
    reader: Reader,
    reader_control: Option<ReaderControl>,
    item_source: Option<SkimItemReceiver>,
    item_pool: Arc<DeferDrop<ItemPool>>,
    
    // === Matcher Integration ===
    matcher: Matcher,
    regex_matcher: Matcher,
    matcher_control: Option<MatcherControl>,
    matched_items: Arc<SpinLock<Vec<MatchedItem>>>,
    matcher_generation: Arc<SpinLock<u64>>,
}

impl<'a> UICoordinator<'a> {
    pub fn new(options: &'a SkimOptions) -> io::Result<Self> {
        // Terminal setup
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        
        // Create shared item pool
        let item_pool = Arc::new(DeferDrop::new(ItemPool::new()));
        
        // Create reader
        let reader = Reader::with_options(&options);
        
        // Create matchers with proper configuration to match legacy system
        use crate::item::RankBuilder;
        let rank_builder = Arc::new(RankBuilder::new(options.tiebreak.clone()));
        
        // Use the same engine factory configuration as legacy system
        let engine_factory: Rc<dyn MatchEngineFactory> = Rc::new(AndOrEngineFactory::new(
            ExactOrFuzzyEngineFactory::builder()
                .exact_mode(options.exact)
                .fuzzy_algorithm(options.algorithm)
                .rank_builder(rank_builder.clone())
                .build(),
        ));
        
        let matcher = Matcher::builder(engine_factory.clone())
            .case(options.case)
            .build();
            
        let regex_matcher = Matcher::builder(Rc::new(RegexEngineFactory::builder()
                .rank_builder(rank_builder.clone())
                .build()))
            .build();
        
        Ok(Self {
            terminal,
            state: UIState::new(),
            options,
            reader,
            reader_control: None,
            matcher,
            regex_matcher,
            matcher_control: None,
            item_pool,
            item_source: None,
            matched_items: Arc::new(SpinLock::new(Vec::new())),
            matcher_generation: Arc::new(SpinLock::new(0)),
            use_regex: false,
            draw_interval: Duration::from_millis(UI_REFRESH_INTERVAL_MS),
        })
    }
    
    pub fn set_item_source(&mut self, source: SkimItemReceiver) {
        self.item_source = Some(source);
    }
    
    pub fn run(mut self) -> io::Result<(Vec<MatchedItem>, String)> {
        // Start reader with item source or default command
        self.reader = if let Some(source) = self.item_source.take() {
            // If we have an item source, use it 
            self.reader.source(Some(source))
        } else {
            // Otherwise just use the reader as is
            self.reader
        };
        
        let reader_control = self.reader.run(&self.options.cmd.clone().unwrap_or_default());
        self.reader_control = Some(reader_control);
        
        // Start initial matcher with empty query
        self.restart_matcher();
        
        // Main event loop - event-driven with prioritized input handling
        'outer: loop {
            // Quick exit check at top of loop
            if !self.state.is_running {
                break;
            }
            
            // 1. Prioritize input events - always check for keys first
            while event::poll(Duration::ZERO)? {
                match event::read()? {
                    Event::Key(key) => {
                        if self.handle_key_event(key)? {
                            break 'outer;
                        }
                        // Force immediate redraw for key events
                        if self.state.needs_redraw {
                            self.draw()?;
                            self.state.last_draw = Instant::now();
                            self.state.needs_redraw = false;
                        }
                    }
                    Event::Resize(_, _) => {
                        self.state.needs_redraw = true;
                    }
                    _ => {}
                }
            }
            
            // Early exit check after input processing
            if !self.state.is_running {
                break;
            }
            
            // 2. Check reader status - but only every few iterations when reading
            let mut needs_restart = false;
            if self.state.is_running {
                if let Some(ref reader_control) = self.reader_control {
                    let was_reading = self.state.is_reading;
                    self.state.is_reading = !reader_control.is_done();
                    
                    // Track read start time for spinner
                    if !was_reading && self.state.is_reading {
                        self.state.read_start_time = Instant::now();
                    }
                    
                    // Only process new items periodically during reading to avoid blocking input
                    if !self.state.is_reading || self.state.last_draw.elapsed() >= Duration::from_millis(READER_CHECK_INTERVAL_MS) {
                        let new_items = reader_control.take();
                        let has_new_items = !new_items.is_empty();
                        if has_new_items {
                            let _ = self.item_pool.append(new_items);
                            self.state.total_items = self.item_pool.len();
                            
                            // Throttle matcher restarts more aggressively during loading
                            let min_restart_interval = self.get_matcher_restart_interval();
                            
                            if self.state.last_matcher_restart.elapsed() >= min_restart_interval {
                                needs_restart = true;
                                self.state.last_matcher_restart = Instant::now();
                            }
                        }
                    } else {
                        // Just update total items count without taking items (much faster)
                        self.state.total_items = self.item_pool.len();
                    }
                }
            }
            
            if needs_restart && self.state.is_running {
                self.restart_matcher();
            }
            
            // 3. Check for matcher updates (non-blocking) - only copy if changed
            // During heavy reading, check much less frequently to prioritize input responsiveness
            let should_check_matcher = self.state.is_running && (!self.state.is_reading || 
                self.state.last_draw.elapsed() >= Duration::from_millis(MATCHER_CHECK_INTERVAL_MS));
                
            if should_check_matcher && self.matcher_control.is_some() {
                let items = self.matched_items.lock();
                let current_count = items.len();
                
                // Update matcher progress for loading indicator
                if let Some(ref ctrl) = self.matcher_control {
                    let new_processed = ctrl.get_num_processed();
                    let new_matched = ctrl.get_num_matched();
                    
                    if new_processed != self.state.processed_items || 
                       new_matched != self.state.last_matched_count {
                        self.state.processed_items = new_processed;
                        self.state.needs_redraw = true; // Progress changed, need redraw
                    }
                    
                    self.state.matcher_running = !ctrl.stopped();
                }
                
                // Only copy if the number of items changed (significant optimization)
                if current_count != self.state.last_matched_count {
                    self.state.matched_items = items.clone();
                    self.state.last_matched_count = current_count;
                    self.state.needs_redraw = true; // Items changed, need redraw
                    
                    // Reset selection if needed
                    if self.state.selected_index >= self.state.matched_items.len() 
                        && !self.state.matched_items.is_empty() {
                        self.state.selected_index = self.state.matched_items.len() - 1;
                    }
                }
                drop(items);
            }
            
            // 4. Send query update to matcher if changed (with debouncing for responsiveness)
            if self.state.is_running && self.state.display_query != self.state.last_query_sent {
                // Debounce query changes - wait for user to stop typing before matching
                let debounce_delay = self.get_query_debounce_delay();
                
                if self.state.query_debounce_timer.elapsed() >= debounce_delay {
                    self.state.last_query_sent = self.state.display_query.clone();
                    self.state.needs_redraw = true; // Query changed, need redraw
                    self.restart_matcher();
                } else {
                    // User is still typing - kill current matcher to save CPU
                    if let Some(ctrl) = self.matcher_control.take() {
                        ctrl.kill();
                    }
                }
            }
            
            // 5. Draw only when needed and enough time has passed
            // Use different intervals: fast for user input, slower during heavy loading
            let draw_interval = if self.state.is_reading || self.state.matcher_running {
                Duration::from_millis(SPINNER_DURATION_MS) // Update for spinner animation
            } else {
                self.draw_interval // Normal speed when not loading
            };
            
            let should_draw_for_animation = (self.state.is_reading || self.state.matcher_running) && 
                self.state.last_draw.elapsed() >= Duration::from_millis(SPINNER_DURATION_MS);
            
            if (self.state.needs_redraw && self.state.last_draw.elapsed() >= draw_interval) || should_draw_for_animation {
                self.draw()?;
                self.state.last_draw = Instant::now();
                self.state.needs_redraw = false;
            }
            
            // Small delay to prevent 100% CPU usage, but only if no input is pending
            // Also break immediately if quit was requested
            if !self.state.is_running {
                break;
            }
            if !event::poll(Duration::ZERO)? {
                let sleep_duration = if self.state.is_reading {
                    Duration::from_millis(SLEEP_READING_MS)
                } else {
                    Duration::from_nanos(SLEEP_IDLE_NANOS)
                };
                std::thread::sleep(sleep_duration);
            }
        }
        
        // Clean up
        self.cleanup()?;
        
        // Return selected items and query
        let selected = if self.state.selected_index < self.state.matched_items.len() {
            vec![self.state.matched_items[self.state.selected_index].clone()]
        } else {
            Vec::new()
        };
        
        Ok((selected, self.state.display_query))
    }
    
    fn handle_key_event(&mut self, key: event::KeyEvent) -> io::Result<bool> {
        match (key.code, key.modifiers) {
            // Quit keys - immediate exit without cleanup in main loop
            (KeyCode::Esc, _) => {
                self.state.is_running = false;
                // Signal stop to matcher without waiting (much faster)
                if let Some(ctrl) = self.matcher_control.take() {
                    // Use a custom fast kill that doesn't wait for thread join
                    self.fast_kill_matcher(ctrl);
                }
                return Ok(true);
            }
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                self.state.is_running = false;
                // Signal stop to matcher without waiting (much faster)
                if let Some(ctrl) = self.matcher_control.take() {
                    // Use a custom fast kill that doesn't wait for thread join
                    self.fast_kill_matcher(ctrl);
                }
                return Ok(true);
            }
            
            // Selection
            (KeyCode::Enter, _) => {
                return Ok(true);
            }
            
            // Navigation
            (KeyCode::Up, _) => {
                if self.state.selected_index > 0 {
                    self.state.selected_index -= 1;
                    self.state.needs_redraw = true;
                }
            }
            (KeyCode::Down, _) => {
                if self.state.selected_index + 1 < self.state.matched_items.len() {
                    self.state.selected_index += 1;
                    self.state.needs_redraw = true;
                }
            }
            
            // Query editing - IMMEDIATE visual update but debounced matching
            (KeyCode::Char(c), _) => {
                self.state.display_query.push(c);
                self.state.needs_redraw = true;
                self.state.query_debounce_timer = Instant::now(); // Reset debounce timer
            }
            (KeyCode::Backspace, _) => {
                self.state.display_query.pop();
                self.state.needs_redraw = true;
                self.state.query_debounce_timer = Instant::now(); // Reset debounce timer
            }
            
            _ => {}
        }
        
        Ok(false)
    }
    
    fn draw(&mut self) -> io::Result<()> {
        // Pre-compute status line to avoid borrowing issues
        let status_line = self.format_status_line();
        
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
            
            // Query input - shows immediately typed characters
            let query_block = Block::default()
                .borders(Borders::ALL)
                .title("Query");
            let query = Paragraph::new(self.state.display_query.as_str())
                .block(query_block);
            f.render_widget(query, chunks[0]);
            
            // Items list with virtual scrolling for performance
            let items_area = chunks[1];
            let visible_height = items_area.height.saturating_sub(2) as usize; // Subtract borders
            
            // Calculate visible range around selected index
            let total_items = self.state.matched_items.len();
            let start_index = if total_items <= visible_height {
                0
            } else if self.state.selected_index < visible_height / 2 {
                0
            } else if self.state.selected_index + visible_height / 2 >= total_items {
                total_items.saturating_sub(visible_height)
            } else {
                self.state.selected_index.saturating_sub(visible_height / 2)
            };
            
            let end_index = (start_index + visible_height).min(total_items);
            
            // Only create ListItems for visible items (major performance improvement)
            let items: Vec<ListItem> = self.state.matched_items[start_index..end_index]
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let global_index = start_index + i;
                    // Avoid string allocation by using text() directly
                    let content = item.item.text();
                    let style = if global_index == self.state.selected_index {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    };
                    ListItem::new(content).style(style)
                })
                .collect();
            
            let items_block = Block::default()
                .borders(Borders::ALL)
                .title("Items");
            let items_list = List::new(items).block(items_block);
            f.render_widget(items_list, chunks[1]);
            
            // Status line with loading indicator (like legacy skim)
            let status_widget = Paragraph::new(status_line.clone());
            f.render_widget(status_widget, chunks[2]);
        })?;
        
        Ok(())
    }
    
    fn format_status_line(&self) -> String {
        let mut status = String::new();
        
        // 1. Spinner during reading or matching (like legacy)
        let reading_for_a_while = self.should_show_loading_indicator(self.state.read_start_time);
        let matching_for_a_while = self.should_show_loading_indicator(self.state.match_start_time);
        
        if (self.state.is_reading && reading_for_a_while) || 
           (self.state.matcher_running && matching_for_a_while) {
            // Animated spinner
            let time_since_match = Instant::now().duration_since(self.state.match_start_time);
            let mills = time_since_match.as_millis() as u64;
            let index = (mills / SPINNER_DURATION_MS) % (SPINNERS.len() as u64);
            let spinner = SPINNERS[index as usize];
            status.push(spinner);
        } else {
            status.push(' ');
        }
        
        // 2. Matched/total counts (like legacy: " 123/456")
        status.push_str(&format!(" {}/{}", self.state.matched_items.len(), self.state.total_items));
        
        // 3. Processing percentage during long matching operations (like legacy)
        if self.state.matcher_running && matching_for_a_while && self.state.total_items > 0 {
            let percentage = (self.state.processed_items * 100) / self.state.total_items;
            status.push_str(&format!(" ({}%)", percentage));
        }
        
        // 4. Current item index/horizontal scroll offset (like legacy: " 1/0")
        if !self.state.matched_items.is_empty() {
            // In legacy: current_item_idx/hscroll_offset  
            // For now, we don't have horizontal scrolling, so use 0
            status.push_str(&format!(" {}/0", self.state.selected_index + 1));
        }
        
        // 5. Matcher running indicator (dot after cursor like legacy)
        if self.state.matcher_running {
            status.push('.');
        }
        
        status
    }
    
    /// Check if enough time has passed to show loading indicators
    fn should_show_loading_indicator(&self, start_time: Instant) -> bool {
        start_time.elapsed().as_millis() > LOADING_INDICATOR_DELAY_MS as u128
    }
    
    /// Get appropriate debounce delay based on current state
    fn get_query_debounce_delay(&self) -> Duration {
        if self.state.is_reading {
            Duration::from_millis(QUERY_DEBOUNCE_READING_MS)
        } else {
            Duration::from_millis(QUERY_DEBOUNCE_IDLE_MS)
        }
    }
    
    /// Get appropriate matcher restart interval based on current state
    fn get_matcher_restart_interval(&self) -> Duration {
        if self.state.is_reading {
            Duration::from_millis(MATCHER_RESTART_READING_MS)
        } else {
            Duration::from_millis(MATCHER_RESTART_IDLE_MS)
        }
    }
    
    fn restart_matcher(&mut self) {
        
        // Kill existing matcher if any
        if let Some(ctrl) = self.matcher_control.take() {
            ctrl.kill();
        }
        
        // Always try to move new items from reader to item pool
        if let Some(ref reader_control) = self.reader_control {
            let new_items = reader_control.take();
            if !new_items.is_empty() {
                let _ = self.item_pool.append(new_items);
            }
        }
        
        // DON'T clear matched items here - let them persist until matcher callback runs
        // This prevents the UI from showing empty results during matcher processing
        
        // Reset the item pool's taken counter so matcher can access all items
        self.item_pool.reset();
        
        // Choose the right matcher
        let matcher = if self.use_regex {
            &self.regex_matcher
        } else {
            &self.matcher
        };
        
        // Run the matcher with current query
        let matched_items = self.matched_items.clone();
        let matcher_generation = self.matcher_generation.clone();
        
        // Increment generation to invalidate old callbacks
        let current_generation = {
            let mut generation = matcher_generation.lock();
            *generation += 1;
            *generation
        };
        
        let control = matcher.run(
            &self.state.display_query,
            self.item_pool.clone(),
            move |items| {
                // Only update if this is still the current generation
                let current_gen = *matcher_generation.lock();
                if current_gen == current_generation {
                    let mut matched = matched_items.lock();
                    matched.clear();
                    let new_items = items.lock();
                    matched.extend(new_items.iter().cloned());
                }
            }
        );
        
        self.matcher_control = Some(control);
        
        // Update matcher timing for loading indicator
        self.state.match_start_time = Instant::now();
        self.state.matcher_running = true;
    }
    
    fn fast_kill_matcher(&self, ctrl: MatcherControl) {
        // Signal the matcher to stop but don't wait for it
        // We can't access the private stopped field, so we'll use a different approach
        
        // First, check if it's already stopped
        if ctrl.stopped() {
            return;
        }
        
        // For fast exit, just forget about the controller entirely
        // This prevents the Drop trait from calling the blocking kill() method
        // The matcher thread will eventually detect that no one is listening and exit
        std::mem::forget(ctrl);
    }
    
    fn cleanup(&mut self) -> io::Result<()> {
        // Fast cleanup - don't wait for components to finish gracefully
        // Just kill them and restore terminal immediately
        
        // Kill components without waiting (already done in quit handler)
        if let Some(ctrl) = self.matcher_control.take() {
            ctrl.kill();
        }
        if let Some(reader_control) = self.reader_control.take() {
            reader_control.kill();
        }
        
        // Restore terminal as quickly as possible
        let _ = disable_raw_mode(); // Ignore errors for speed
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        ); // Ignore errors for speed
        let _ = self.terminal.show_cursor(); // Ignore errors for speed
        
        Ok(())
    }
}