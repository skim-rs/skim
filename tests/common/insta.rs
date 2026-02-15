use std::io::Cursor;

use clap::Parser;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use skim::{
    Skim, SkimItemReceiver,
    prelude::*,
    tui::{Event, Tui, event::Action},
};

/// A test harness for running skim TUI tests with insta snapshots.
///
/// This struct wraps a [`Skim<TestBackend>`] instance, providing a synchronous,
/// event-driven interface that mirrors how the real application works. Events are
/// sent via the event channel and processed through the app's event loop.
///
/// The harness reuses as much of the production code path as possible:
/// - Items are loaded through the same `Reader` + `SkimItemReader` pipeline as
///   the real application
/// - Reload events use `Skim::handle_reload()`, the same logic as the production
///   event loop
/// - Reader completion is checked via `Skim::check_reader()`, identical to
///   production
pub struct TestHarness {
    /// The Skim instance backed by a TestBackend for snapshot testing.
    pub skim: Skim<TestBackend>,
    /// Tokio runtime for async operations (preview commands, etc.)
    pub runtime: tokio::runtime::Runtime,
    /// The final event that caused the app to quit (for determining exit code)
    pub final_event: Option<Event>,
}

impl TestHarness {
    /// Process all pending events from the event queue.
    ///
    /// This is the core method that processes events just like the real event loop
    /// in `lib.rs`. It drains the event_rx channel and calls `app.handle_event()`
    /// for each event, mimicking the actual application behavior.
    ///
    /// For `Event::Reload`, it delegates to `Skim::handle_reload()` — the same
    /// method used by the production event loop.
    pub fn tick(&mut self) -> Result<()> {
        // Drain-and-process in a loop so events queued during processing
        // are picked up on the next iteration.
        loop {
            let mut events = Vec::new();
            while let Ok(event) = self.skim.tui_mut().event_rx.try_recv() {
                events.push(event);
            }
            if events.is_empty() {
                break;
            }
            for event in events {
                self.process_event(event)?;
            }
        }
        Ok(())
    }

    /// Process a single event through the same logic as the production event loop.
    ///
    /// `Event::Reload` is handled via `Skim::handle_reload()` — the exact same
    /// method used in production. All other events are forwarded to
    /// `app.handle_event()`.
    fn process_event(&mut self, event: Event) -> Result<()> {
        if let Event::Reload(ref new_cmd) = event {
            let new_cmd = new_cmd.clone();
            self.skim.handle_reload(&new_cmd);
        } else {
            // Let the app handle the event (this may queue more events)
            // Enter the runtime context so that tokio::spawn() calls work
            let _guard = self.runtime.enter();
            let (app, tui) = self.skim.app_and_tui();
            app.handle_event(tui, &event)?;
        }

        // Check reader status, just like the production event loop
        self.skim.check_reader();

        // Track if app should quit and what the final event was
        if self.skim.app().should_quit && self.final_event.is_none() {
            self.final_event = Some(event);
        }

        Ok(())
    }

    /// Send an event to the event queue.
    ///
    /// This queues an event for processing. Call `tick()` to process queued events.
    pub fn send(&mut self, event: Event) -> Result<()> {
        self.skim.tui_mut().event_tx.try_send(event)?;
        Ok(())
    }

    /// Send a key event and process it immediately.
    ///
    /// This is the primary way to simulate user input. It:
    /// 1. Sends the key event to the queue
    /// 2. Processes all pending events (including any triggered by the key)
    /// 3. Waits for reader (if running) and matcher to complete
    pub fn key(&mut self, key: KeyEvent) -> Result<()> {
        self.send(Event::Key(key))?;
        self.tick()?;
        self.wait_for_completion()?;
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
        self.wait_for_completion()?;
        Ok(())
    }

    /// Wait for any in-flight reader and matcher to complete.
    ///
    /// If the reader is still running (e.g., after a reload in interactive mode),
    /// waits for it to finish first. Then waits for the matcher if it needs to run.
    ///
    /// If a debounced matcher restart is pending (e.g., because the query changed
    /// within the 50ms debounce window), this forces the restart immediately so
    /// tests don't see stale results.
    fn wait_for_completion(&mut self) -> Result<()> {
        // If a debounced matcher restart is pending, force it now.
        // In production, the Heartbeat event would trigger this, but in tests
        // we don't have a continuous heartbeat timer.
        if self.skim.app().pending_matcher_restart {
            self.skim.app_mut().restart_matcher(true);
        }

        // If the reader is running (not done), wait for it + matcher
        if !self.skim.reader_done() {
            self.wait_for_reader_and_matcher()?;
        } else {
            // Always wait for the matcher to ensure results are consumed.
            // Even if stopped() is already true, wait_for_matcher will
            // render and process heartbeat to consume processed_items.
            self.wait_for_matcher()?;
        }
        Ok(())
    }

    /// Render the current app state to the terminal buffer.
    pub fn render(&mut self) -> Result<()> {
        let (app, tui) = self.skim.app_and_tui();
        tui.draw(|frame| {
            frame.render_widget(&mut *app, frame.area());
        })?;
        Ok(())
    }

    /// Get a string representation of the current buffer for snapshot testing.
    pub fn buffer_view(&self) -> String {
        self.skim.tui_ref().backend().to_string()
    }

    /// Prepare for taking a snapshot by waiting for preview and processing heartbeat.
    ///
    /// This ensures the state is up-to-date before taking a snapshot.
    /// Call `render()` and `buffer_view()` afterward to actually take the snapshot.
    pub fn prepare_snap(&mut self) -> Result<()> {
        // Send heartbeat first to trigger any debounced pending preview runs.
        // The debounce logic in run_preview() sets pending_preview_run=true when
        // a RunPreview is dropped. The Heartbeat handler checks this flag and
        // spawns the preview. We need this to happen BEFORE wait_for_preview()
        // so it can detect and wait for the spawned preview task.
        self.send(Event::Heartbeat)?;
        self.tick()?;

        // Now wait for preview if configured
        if self.skim.app().options.preview.is_some() {
            self.wait_for_preview()?;
        }

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
        let cursor_pos = format!(
            "cursor: {}x{}",
            self.skim.app().cursor_pos.0,
            self.skim.app().cursor_pos.1
        );
        insta::assert_snapshot!(buf + &cursor_pos);
        Ok(())
    }

    /// Wait for the reader to finish producing items and the matcher to complete.
    ///
    /// This polls `Skim::reader_done()` and `Skim::check_reader()` — the same
    /// methods used by the production event loop — until the reader has finished,
    /// then waits for the matcher to process all items.
    pub fn wait_for_reader_and_matcher(&mut self) -> Result<()> {
        let timeout = std::time::Duration::from_secs(5);
        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_millis(10);

        // Wait for reader to finish
        while !self.skim.reader_done() {
            if start.elapsed() > timeout {
                return Err(color_eyre::eyre::eyre!("Timeout waiting for reader to finish"));
            }
            // Check reader status (may restart matcher)
            self.skim.check_reader();
            std::thread::sleep(poll_interval);
        }

        // Final check to restart matcher with remaining items
        self.skim.check_reader();

        self.wait_for_matcher()?;
        Ok(())
    }

    /// Wait for matcher to complete processing.
    pub fn wait_for_matcher(&mut self) -> Result<()> {
        let timeout = std::time::Duration::from_secs(5);
        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_millis(10);

        // Wait for matcher to complete
        while !self.skim.app().matcher_control.stopped() {
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
        let needs_preview = self.skim.app().options.preview.is_some() && self.skim.app().item_list.selected().is_some();
        if needs_preview {
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
        if let Some(ref handle) = self.skim.app().preview.thread_handle {
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

            // Drain events then process, so we can check for PreviewReady
            let mut events = Vec::new();
            while let Ok(event) = self.skim.tui_mut().event_rx.try_recv() {
                events.push(event);
            }
            for event in events {
                let is_preview_ready = matches!(event, Event::PreviewReady);
                self.process_event(event)?;
                // If we got PreviewReady, render and return
                if is_preview_ready {
                    self.render()?;
                    return Ok(());
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

    /// Get the exit code that skim would return based on the final event.
    ///
    /// This mimics the logic in `bin/main.rs`:
    /// - 130 if the app aborted (Ctrl+C, Ctrl+D, Esc, etc.)
    /// - 0 if the app accepted (Enter)
    /// - None if the app hasn't quit yet
    pub fn app_exit_code(&self) -> Option<i32> {
        if !self.skim.app().should_quit {
            return None;
        }

        // Check if the final event was an Accept action
        // This matches the logic in lib.rs:560
        let is_abort = self
            .final_event
            .as_ref()
            .map(|event| !matches!(event, Event::Action(Action::Accept(_))))
            .unwrap_or(true);

        Some(if is_abort { 130 } else { 0 })
    }
}

// ============================================================================
// Factory functions
// ============================================================================

/// Initialize a test harness with the given options, dimensions, and optional item source.
///
/// Uses [`Skim::init`] for the core initialization (App, Reader, theme, etc.)
/// and [`Skim::init_tui_with`] to inject a [`TestBackend`].
/// Then calls [`Skim::start`] to begin the reader and matcher — the same
/// production code path used by the real application.
fn enter_sized_with_source(
    options: SkimOptions,
    width: u16,
    height: u16,
    source: Option<SkimItemReceiver>,
) -> Result<TestHarness> {
    let backend = TestBackend::new(width, height);
    let tui = Tui::new_for_test(backend)?;
    let mut skim = Skim::<TestBackend>::init(options, source)?;
    skim.init_tui_with(tui);

    // Start the reader and matcher — the same call the production binary makes
    skim.start();

    // Create a multi-threaded tokio runtime for async operations (preview commands, etc.)
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;

    let mut harness = TestHarness {
        skim,
        runtime,
        final_event: None,
    };

    // Wait for the reader to finish and matcher to complete
    harness.wait_for_reader_and_matcher()?;

    Ok(harness)
}

/// Initialize a test harness with the given options and dimensions.
pub fn enter_sized(options: SkimOptions, width: u16, height: u16) -> Result<TestHarness> {
    enter_sized_with_source(options, width, height, None)
}

/// Initialize a test harness with default dimensions (80x24).
pub fn enter(options: SkimOptions) -> Result<TestHarness> {
    enter_sized(options, 80, 24)
}

/// Initialize a test harness with default options.
pub fn enter_default() -> Result<TestHarness> {
    enter_sized(SkimOptions::default().build(), 80, 24)
}

/// Initialize a test harness with pre-loaded items.
///
/// Items are fed through the production `Reader` + `SkimItemReader` pipeline
/// by converting them to a newline-separated byte stream and using
/// `SkimItemReader::of_bufread()`.
pub fn enter_items<I, S>(items: I, options: SkimOptions) -> Result<TestHarness>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    // Build a newline-terminated string from items and feed through SkimItemReader.
    // Each item must end with '\n' so the reader produces one item per entry,
    // including empty-string items like "".
    let text: String = items
        .into_iter()
        .map(|s| {
            let mut line = s.as_ref().to_owned();
            line.push('\n');
            line
        })
        .collect();

    let reader_opts = SkimItemReaderOption::from_options(&options);
    let item_reader = SkimItemReader::new(reader_opts);
    let rx = item_reader.of_bufread(Cursor::new(text));

    enter_sized_with_source(options, 80, 24, Some(rx))
}

/// Initialize a test harness with command output as items.
///
/// The command is executed through the production `Reader` + `SkimItemReader`
/// pipeline, identical to how the real application works.
pub fn enter_cmd(cmd: &str, options: SkimOptions) -> Result<TestHarness> {
    let reader_opts = SkimItemReaderOption::from_options(&options);
    let item_reader = SkimItemReader::new(reader_opts);
    let rx = item_reader.of_bufread(std::io::BufReader::new(
        std::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()?
            .stdout
            .ok_or_else(|| color_eyre::eyre::eyre!("Failed to capture stdout"))?,
    ));

    enter_sized_with_source(options, 80, 24, Some(rx))
}

/// Initialize a test harness for interactive mode.
///
/// Uses [`Skim::start`] which already handles interactive mode correctly:
/// it expands the command template with the initial query and starts the
/// reader pipeline.
pub fn enter_interactive(options: SkimOptions) -> Result<TestHarness> {
    // Skim::init() computes initial_cmd for interactive mode,
    // and Skim::start() kicks off the reader with it.
    // No special-casing needed here.
    enter(options)
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
            $harness.skim.app().cursor_pos.1 + 1,
            $harness.skim.app().cursor_pos.0 + 1
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
///     @exited 0;          // Assert command exited with status code 0
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

    // Simple variant with items expression (identifier or expression) - just snapshot
    ($name:ident, $items:expr, $options:expr) => {
        #[test]
        fn $name() -> color_eyre::Result<()> {
            let options = $crate::common::insta::parse_options($options);
            let mut h = $crate::common::insta::enter_items($items, options)?;
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

    // DSL variant with items expression (identifier or expression)
    ($name:ident, $items:expr, $options:expr, { $($content:tt)* }) => {
        #[test]
        fn $name() -> color_eyre::Result<()> {
            let options = $crate::common::insta::parse_options($options);
            let mut h = $crate::common::insta::enter_items($items, options)?;

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
    // Usage: @assert(|h| h.skim.app().should_quit);
    //        @assert(|h| h.skim.app().item_list.selected().unwrap().text() == "1");
    (@expand $h:ident; @assert ( $assertion:expr ) ; $($rest:tt)*) => {
        assert!(($assertion)(&$h));
        insta_test!(@expand $h; $($rest)*);
    };

    // @exited - assert that the app would exit with a specific status code
    // Usage: @exited 0;      // Assert successful exit (Accept)
    //        @exited 130;    // Assert abort (Ctrl+C, Ctrl+D, Esc)
    (@expand $h:ident; @exited $code:expr ; $($rest:tt)*) => {
        assert_eq!(
            $h.app_exit_code(),
            Some($code),
            "Expected app to exit with status code {}, but got {:?}",
            $code,
            $h.app_exit_code()
        );
        insta_test!(@expand $h; $($rest)*);
    };
}
