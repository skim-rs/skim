use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;

use clap::Parser;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use skim::{
    field::FieldRange,
    helper::item::DefaultSkimItem,
    prelude::*,
    theme::ColorTheme,
    tui::{App, Event, Tui, event::Action},
};

/// A test harness for running skim TUI tests with insta snapshots.
///
/// This struct wraps the TUI and App, providing an event-driven interface
/// that mirrors how the real application works. Events are sent via the
/// event channel and processed through the app's event loop.
pub struct TestHarness<'a> {
    /// The TUI instance with TestBackend
    pub tui: Tui<TestBackend>,
    /// The application state
    pub app: App<'a>,
    /// Tokio runtime for async operations (preview commands, etc.)
    pub runtime: tokio::runtime::Runtime,
}

impl<'a> TestHarness<'a> {
    /// Process all pending events from the event queue.
    ///
    /// This is the core method that processes events just like the real event loop
    /// in `lib.rs`. It drains the event_rx channel and calls `app.handle_event()`
    /// for each event, mimicking the actual application behavior.
    ///
    /// For `Event::Reload`, it executes the command and restarts the reader,
    /// just like the main event loop does.
    pub fn tick(&mut self) -> Result<()> {
        // Process all pending events
        while let Ok(event) = self.tui.event_rx.try_recv() {
            self.process_event(event)?;
        }
        Ok(())
    }

    /// Process a single event through the app's event handler.
    ///
    /// This handles special events like Reload that need extra processing
    /// beyond what `app.handle_event()` does.
    fn process_event(&mut self, event: Event) -> Result<()> {
        // Handle reload event specially - this is what the main loop does
        if let Event::Reload(ref new_cmd) = event {
            // Clear items
            self.app.item_pool.clear();
            if !self.app.options.no_clear_if_empty {
                self.app.item_list.clear();
            }
            // Run the command and add items
            self.run_command_internal(new_cmd)?;
            self.app.restart_matcher(true);
        }

        // Let the app handle the event (this may queue more events)
        // Enter the runtime context so that tokio::spawn() calls work
        let _guard = self.runtime.enter();
        self.app.handle_event(&mut self.tui, &event)?;
        Ok(())
    }

    /// Send an event to the event queue.
    ///
    /// This queues an event for processing. Call `tick()` to process queued events.
    pub fn send(&mut self, event: Event) -> Result<()> {
        self.tui.event_tx.send(event)?;
        Ok(())
    }

    /// Send a key event and process it immediately.
    ///
    /// This is the primary way to simulate user input. It:
    /// 1. Sends the key event to the queue
    /// 2. Processes all pending events (including any triggered by the key)
    /// 3. For interactive mode, handles any reload commands
    pub fn key(&mut self, key: KeyEvent) -> Result<()> {
        self.send(Event::Key(key))?;
        self.tick()?;
        // Wait for matcher if items changed
        if self.app.pending_matcher_restart || !self.app.matcher_control.stopped() {
            self.wait_for_matcher()?;
        }
        Ok(())
    }

    /// Send a character key event.
    pub fn char(&mut self, c: char) -> Result<()> {
        self.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
    }

    /// Type a string, sending each character as a key event.
    pub fn type_str(&mut self, s: &str) -> Result<()> {
        for c in s.chars() {
            self.char(c)?;
        }
        Ok(())
    }

    /// Send an action and process it immediately.
    pub fn action(&mut self, action: Action) -> Result<()> {
        self.send(Event::Action(action))?;
        self.tick()?;
        // Wait for matcher if items changed
        if self.app.pending_matcher_restart || !self.app.matcher_control.stopped() {
            self.wait_for_matcher()?;
        }
        Ok(())
    }

    /// Render the current app state to the terminal buffer.
    pub fn render(&mut self) -> Result<()> {
        self.tui.draw(|frame| {
            frame.render_widget(&mut self.app, frame.area());
        })?;
        Ok(())
    }

    /// Get a string representation of the current buffer for snapshot testing.
    pub fn buffer_view(&self) -> String {
        self.tui.backend().to_string()
    }

    /// Prepare for taking a snapshot by waiting for preview and processing heartbeat.
    ///
    /// This ensures the state is up-to-date before taking a snapshot.
    /// Call `render()` and `buffer_view()` afterward to actually take the snapshot.
    pub fn prepare_snap(&mut self) -> Result<()> {
        // Wait for preview if configured - do this BEFORE heartbeat so we don't
        // accidentally consume PreviewReady events
        if self.app.options.preview.is_some() {
            self.wait_for_preview()?;
        }

        // Send heartbeat to update status counters (item counts, spinner, etc.)
        self.send(Event::Heartbeat)?;
        self.tick()?;

        self.render()?;
        Ok(())
    }

    /// Take a snapshot of the current state.
    ///
    /// NOTE: This method should NOT be called from test code directly because
    /// insta will use the wrong file path for the snapshot. Use the snap! macro instead.
    #[doc(hidden)]
    pub fn snap(&mut self) -> Result<()> {
        self.prepare_snap()?;
        let buf = self.buffer_view();
        let cursor_pos = format!("cursor: {}x{}", self.app.cursor_pos.0, self.app.cursor_pos.1);
        insta::assert_snapshot!(buf + &cursor_pos);
        Ok(())
    }

    /// Add items to the item pool and run the matcher.
    pub fn add_items<I, S>(&mut self, items: I) -> Result<()>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        // Parse field ranges from options
        let transform_fields: Vec<FieldRange> = self
            .app
            .options
            .with_nth
            .iter()
            .filter_map(|f| if !f.is_empty() { FieldRange::from_str(f) } else { None })
            .collect();

        let matching_fields: Vec<FieldRange> = self
            .app
            .options
            .nth
            .iter()
            .filter_map(|f| if !f.is_empty() { FieldRange::from_str(f) } else { None })
            .collect();

        let items: Vec<Arc<dyn SkimItem>> = items
            .into_iter()
            .enumerate()
            .map(|(idx, s)| {
                Arc::new(DefaultSkimItem::new(
                    s.into(),
                    self.app.options.ansi,
                    &transform_fields,
                    &matching_fields,
                    &self.app.options.delimiter,
                    idx,
                )) as Arc<dyn SkimItem>
            })
            .collect();
        self.app.handle_items(items);
        self.app.restart_matcher(true);
        self.wait_for_matcher()?;
        Ok(())
    }

    /// Execute a shell command and add its output lines as items.
    /// This is an internal method that doesn't restart the matcher.
    fn run_command_internal(&mut self, cmd: &str) -> Result<()> {
        let output = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdout = output
            .stdout
            .ok_or_else(|| color_eyre::eyre::eyre!("Failed to capture stdout"))?;
        let reader = BufReader::new(stdout);

        // Parse field ranges from options
        let transform_fields: Vec<FieldRange> = self
            .app
            .options
            .with_nth
            .iter()
            .filter_map(|f| if !f.is_empty() { FieldRange::from_str(f) } else { None })
            .collect();

        let matching_fields: Vec<FieldRange> = self
            .app
            .options
            .nth
            .iter()
            .filter_map(|f| if !f.is_empty() { FieldRange::from_str(f) } else { None })
            .collect();

        let items: Vec<Arc<dyn SkimItem>> = reader
            .lines()
            .filter_map(|line| line.ok())
            .enumerate()
            .map(|(idx, s)| {
                Arc::new(DefaultSkimItem::new(
                    s,
                    self.app.options.ansi,
                    &transform_fields,
                    &matching_fields,
                    &self.app.options.delimiter,
                    idx,
                )) as Arc<dyn SkimItem>
            })
            .collect();

        self.app.handle_items(items);
        Ok(())
    }

    /// Execute a shell command and add its output lines as items.
    pub fn run_command(&mut self, cmd: &str) -> Result<()> {
        self.run_command_internal(cmd)?;
        self.app.restart_matcher(true);
        self.wait_for_matcher()?;
        Ok(())
    }

    /// Wait for matcher to complete processing.
    pub fn wait_for_matcher(&mut self) -> Result<()> {
        let timeout = std::time::Duration::from_secs(5);
        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_millis(10);

        // Wait for matcher to complete
        while !self.app.matcher_control.stopped() {
            if start.elapsed() > timeout {
                return Err(color_eyre::eyre::eyre!("Timeout waiting for matcher to stop"));
            }
            std::thread::sleep(poll_interval);
        }

        // Give the background processing thread time to receive items
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Render to consume processed items
        self.render()?;

        // Process heartbeat to update status counters
        self.send(Event::Heartbeat)?;
        self.tick()?;

        // Manually trigger preview if configured and an item is selected
        // Note: on_item_changed won't trigger automatically because the item was
        // already selected during render, so prev_item == new_item
        if self.app.options.preview.is_some() && self.app.item_list.selected().is_some() {
            self.send(Event::RunPreview)?;
            self.wait_for_preview()?;
        }

        Ok(())
    }

    /// Wait for preview to be ready.
    pub fn wait_for_preview(&mut self) -> Result<()> {
        // Process any queued events first (including RunPreview)
        self.tick()?;

        // Now check if there's a pending preview task
        // If not, there's nothing to wait for
        if let Some(ref handle) = self.app.preview.thread_handle {
            if handle.is_finished() {
                return Ok(());
            }
        } else {
            return Ok(());
        }

        // Wait for preview to execute
        // With multi-threaded runtime, spawned tasks run on background threads
        let timeout = std::time::Duration::from_secs(2);
        let start = std::time::Instant::now();

        loop {
            // Sleep to give background tasks time to execute
            std::thread::sleep(std::time::Duration::from_millis(50));

            // Try to process any pending events (including PreviewReady)
            loop {
                match self.tui.event_rx.try_recv() {
                    Ok(event) => {
                        let is_preview_ready = matches!(event, Event::PreviewReady);
                        self.process_event(event)?;
                        // If we got PreviewReady, render and return
                        if is_preview_ready {
                            self.render()?;
                            return Ok(());
                        }
                    }
                    Err(_) => break, // No more events
                }
            }

            if start.elapsed() > timeout {
                // Timeout - render anyway and return
                self.render()?;
                return Ok(());
            }
        }
    }

    /// Process a heartbeat event to update status counters.
    pub fn heartbeat(&mut self) -> Result<()> {
        self.send(Event::Heartbeat)?;
        self.tick()
    }
}

// ============================================================================
// Factory functions
// ============================================================================

/// Initialize a test harness with the given options and dimensions.
pub fn enter_sized<'a>(options: SkimOptions, width: u16, height: u16) -> Result<TestHarness<'a>> {
    let backend = TestBackend::new(width, height);
    let tui = Tui::new_for_test(backend)?;
    let theme = Arc::new(ColorTheme::init_from_options(&options));
    let cmd = options.cmd.clone().unwrap_or_default();
    let app = App::from_options(options, theme, cmd);

    // Create a multi-threaded tokio runtime for async operations (preview commands, etc.)
    // We use multi-threaded so spawned tasks can execute on background threads
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;

    Ok(TestHarness { tui, app, runtime })
}

/// Initialize a test harness with default dimensions (80x24).
pub fn enter<'a>(options: SkimOptions) -> Result<TestHarness<'a>> {
    enter_sized(options, 80, 24)
}

/// Initialize a test harness with default options.
pub fn enter_default<'a>() -> Result<TestHarness<'a>> {
    enter_sized(SkimOptions::default().build(), 80, 24)
}

/// Initialize a test harness with pre-loaded items.
pub fn enter_items<'a, I, S>(items: I, options: SkimOptions) -> Result<TestHarness<'a>>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut harness = enter(options)?;
    harness.add_items(items)?;
    Ok(harness)
}

/// Initialize a test harness with command output as items.
pub fn enter_cmd<'a>(cmd: &str, options: SkimOptions) -> Result<TestHarness<'a>> {
    let mut harness = enter(options)?;
    harness.run_command(cmd)?;
    Ok(harness)
}

/// Initialize a test harness for interactive mode.
///
/// This runs the initial command (with empty query) and sets up the harness.
pub fn enter_interactive<'a>(options: SkimOptions) -> Result<TestHarness<'a>> {
    let mut harness = enter(options)?;

    // Run initial command with current (empty) query
    if let Some(ref cmd_template) = harness.app.options.cmd.clone() {
        let expanded_cmd = harness.app.expand_cmd(&cmd_template, true);
        harness.run_command(&expanded_cmd)?;
    }

    Ok(harness)
}

/// Parse SkimOptions from CLI-style arguments.
pub fn parse_options(args: &[&str]) -> SkimOptions {
    let mut full_args = vec!["sk"];
    full_args.extend(args);
    SkimOptions::try_parse_from(full_args)
        .expect("Failed to parse options")
        .build()
}

// ============================================================================
// Macros
// ============================================================================

#[macro_export]
macro_rules! snap {
    ($harness:ident) => {
        $harness.prepare_snap()?;
        let buf = $harness.buffer_view();
        let cursor_pos = format!(
            "cursor: ({}, {})",
            $harness.app.cursor_pos.1 + 1,
            $harness.app.cursor_pos.0 + 1
        );
        insta::assert_snapshot!(buf + &cursor_pos);
    };
}

/// Macro for writing compact insta snapshot tests.
///
/// # Usage
///
/// ## Input syntax:
/// - `["a", "b", "c"]` - items array
/// - `@cmd "seq 1 100"` - command output as items
/// - `@interactive` - interactive mode with `--cmd`
///
/// ## Basic usage (just takes a snapshot):
/// ```ignore
/// insta_test!(test_name, ["item1", "item2"], &["--opts"]);
/// ```
///
/// ## DSL usage (with commands):
/// ```ignore
/// insta_test!(test_name, ["a", "b", "c"], &["--multi"], {
///     @snap;              // Take snapshot
///     @char 'f';          // Send single character
///     @type "foo";        // Type string
///     @action Down(1);    // Send action
///     @key Enter;         // Send special key
/// });
/// ```
#[macro_export]
macro_rules! insta_test {
    // Simple variant with items array - just snapshot
    ($name:ident, [$($item:expr),* $(,)?], $options:expr) => {
        #[test]
        fn $name() -> color_eyre::Result<()> {
            let options = $crate::common::insta::parse_options($options);
            let mut h = $crate::common::insta::enter_items([$($item),*], options)?;
            $crate::snap!(h);
            Ok(())
        }
    };

    // Simple variant with @cmd - just snapshot
    ($name:ident, @cmd $cmd:expr, $options:expr) => {
        #[test]
        fn $name() -> color_eyre::Result<()> {
            let options = $crate::common::insta::parse_options($options);
            let mut h = $crate::common::insta::enter_cmd($cmd, options)?;
            $crate::snap!(h);
            Ok(())
        }
    };

    // Simple variant with @interactive - just snapshot
    ($name:ident, @interactive, $options:expr) => {
        #[test]
        fn $name() -> color_eyre::Result<()> {
            let options = $crate::common::insta::parse_options($options);
            let mut h = $crate::common::insta::enter_interactive(options)?;
            $crate::snap!(h);
            Ok(())
        }
    };

    // DSL variant with items array
    ($name:ident, [$($item:expr),* $(,)?], $options:expr, { $($content:tt)* }) => {
        #[test]
        fn $name() -> color_eyre::Result<()> {
            let options = $crate::common::insta::parse_options($options);
            let mut h = $crate::common::insta::enter_items([$($item),*], options)?;

            insta_test!(@expand h; $($content)*);

            Ok(())
        }
    };

    // DSL variant with @cmd
    ($name:ident, @cmd $cmd:expr, $options:expr, { $($content:tt)* }) => {
        #[test]
        fn $name() -> color_eyre::Result<()> {
            let options = $crate::common::insta::parse_options($options);
            let mut h = $crate::common::insta::enter_cmd($cmd, options)?;

            insta_test!(@expand h; $($content)*);

            Ok(())
        }
    };

    // DSL variant with @interactive
    ($name:ident, @interactive, $options:expr, { $($content:tt)* }) => {
        #[test]
        fn $name() -> color_eyre::Result<()> {
            let options = $crate::common::insta::parse_options($options);
            let mut h = $crate::common::insta::enter_interactive(options)?;

            insta_test!(@expand h; $($content)*);

            Ok(())
        }
    };

    // Token processing rules
    (@expand $h:ident; ) => {};

    // @snap - take snapshot
    (@expand $h:ident; @snap; $($rest:tt)*) => {
        $crate::snap!($h);
        insta_test!(@expand $h; $($rest)*);
    };

    // @char - send single character
    (@expand $h:ident; @char $c:expr ; $($rest:tt)*) => {
        $h.char($c)?;
        insta_test!(@expand $h; $($rest)*);
    };

    // @type - type a string
    (@expand $h:ident; @type $text:expr ; $($rest:tt)*) => {
        $h.type_str($text)?;
        insta_test!(@expand $h; $($rest)*);
    };

    // @action - send an action (e.g., @action Down(1); or @action BackwardChar;)
    (@expand $h:ident; @action $action:ident ; $($rest:tt)*) => {
        $h.action(skim::tui::event::Action::$action)?;
        insta_test!(@expand $h; $($rest)*);
    };

    // @action with parenthesized args (e.g., @action Down(1);)
    (@expand $h:ident; @action $action:ident ($($args:tt)*) ; $($rest:tt)*) => {
        $h.action(skim::tui::event::Action::$action($($args)*))?;
        insta_test!(@expand $h; $($rest)*);
    };

    // @key - send a special key (Enter, Escape, Tab, etc.)
    (@expand $h:ident; @key $key:ident ; $($rest:tt)*) => {
        $h.key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::$key,
            crossterm::event::KeyModifiers::NONE
        ))?;
        insta_test!(@expand $h; $($rest)*);
    };

    // @ctrl - send a key with Ctrl modifier
    (@expand $h:ident; @ctrl $key:ident ; $($rest:tt)*) => {
        $h.key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::$key,
            crossterm::event::KeyModifiers::CONTROL
        ))?;
        insta_test!(@expand $h; $($rest)*);
    };

    // @ctrl with char
    (@expand $h:ident; @ctrl $key:literal ; $($rest:tt)*) => {
        $h.key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char($key),
            crossterm::event::KeyModifiers::CONTROL
        ))?;
        insta_test!(@expand $h; $($rest)*);
    };

    // @alt - send a key with Alt modifier
    (@expand $h:ident; @alt $key:ident ; $($rest:tt)*) => {
        $h.key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::$key,
            crossterm::event::KeyModifiers::ALT
        ))?;
        insta_test!(@expand $h; $($rest)*);
    };

    // @alt with char
    (@expand $h:ident; @alt $key:literal ; $($rest:tt)*) => {
        $h.key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char($key),
            crossterm::event::KeyModifiers::ALT
        ))?;
        insta_test!(@expand $h; $($rest)*);
    };

    // @shift - send a key with Shift modifier
    (@expand $h:ident; @shift $key:ident ; $($rest:tt)*) => {
        $h.key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::$key,
            crossterm::event::KeyModifiers::SHIFT
        ))?;
        insta_test!(@expand $h; $($rest)*);
    };

    // @shift with char
    (@expand $h:ident; @shift $key:literal ; $($rest:tt)*) => {
        $h.key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char($key),
            crossterm::event::KeyModifiers::SHIFT
        ))?;
        insta_test!(@expand $h; $($rest)*);
    };

    // @dbg - debug print current buffer
    (@expand $h:ident; @dbg; $($rest:tt)*) => {
        $h.render()?;
        println!("DBG buffer:\n{}", $h.buffer_view());
        insta_test!(@expand $h; $($rest)*);
    };

    // @assert - run an assertion closure
    // Pass a closure that takes the harness as parameter
    // Usage: @assert(|h| h.app.should_quit);
    //        @assert(|h| h.app.item_list.selected().unwrap().text() == "1");
    (@expand $h:ident; @assert ( $assertion:expr ) ; $($rest:tt)*) => {
        assert!(($assertion)(&$h));
        insta_test!(@expand $h; $($rest)*);
    };
}
