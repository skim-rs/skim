//! Module containing skim's entry point
use std::env;
use std::io::{BufWriter, Stderr};
use std::sync::Arc;
use std::time::Duration;

use color_eyre::eyre::Result;
use color_eyre::eyre::{self, OptionExt};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::{runtime::Handle, select, task::block_in_place};

use crate::reader::{Reader, ReaderControl};
use crate::tui::{App, Event, Size, Tui, event::Action};
use crate::{SkimItemReceiver, SkimOptions, SkimOutput};

/// Main entry point for running skim
pub struct Skim<Backend = ratatui::backend::CrosstermBackend<BufWriter<Stderr>>>
where
    Backend: ratatui::backend::Backend,
    Backend::Error: Send + Sync + 'static,
{
    app: App,
    tui: Option<Tui<Backend>>,
    height: Size,
    reader: Reader,
    reader_done: bool,
    initial_cmd: String,
    reader_control: Option<ReaderControl>,
    matcher_interval: Option<tokio::time::Interval>,
    listener: Option<interprocess::local_socket::tokio::Listener>,
    final_event: Event,
    final_key: KeyEvent,
}

impl Skim {
    /// Run skim, collecting items from the source and using options
    ///
    /// # Params
    ///
    /// - options: the "complex" options that control how skim behaves
    /// - source: a stream of items to be passed to skim for filtering.
    ///   If None is given, skim will invoke the command given to fetch the items.
    ///
    /// # Returns
    ///
    /// - None: on internal errors.
    /// - `SkimOutput`: the collected key, event, query, selected items, etc.
    ///
    /// # Panics
    ///
    /// Panics if the tui fails to initilize
    pub fn run_with(mut options: SkimOptions, source: Option<SkimItemReceiver>) -> Result<SkimOutput> {
        trace!("running skim");
        // In filter mode, use the filter string as the query for matching
        if let Some(ref filter_query) = options.filter
            && options.query.is_none()
        {
            options.query = Some(filter_query.clone());
        }
        let mut skim = Self::init(options, source)?;

        skim.start();

        if skim.should_enter() {
            skim.init_tui()?;
            let task = async {
                skim.enter().await?;
                skim.run().await?;
                eyre::Ok(())
            };

            if let Ok(handle) = Handle::try_current() {
                block_in_place(|| handle.block_on(task))?;
            } else {
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(task)?;
            }
        } else {
            // We didn't enter
            skim.final_event = Event::Action(Action::Accept(None));
        }
        let output = skim.output();
        debug!("output: {output:?}");

        Ok(output)
    }

    /// Initialize the TUI with the default crossterm backend, but do not enter it yet
    pub fn init_tui(&mut self) -> Result<()> {
        self.tui = Some(Tui::new_with_height(self.height)?);
        Ok(())
    }
}

impl<Backend: ratatui::backend::Backend + 'static> Skim<Backend>
where
    Backend::Error: Send + Sync + 'static,
{
    /// Initialize skim, without starting anything yet
    pub fn init(options: SkimOptions, source: Option<SkimItemReceiver>) -> Result<Self> {
        let height = Size::try_from(options.height.as_str())?;

        // application state
        // Initialize theme from options
        let theme = Arc::new(crate::theme::ColorTheme::init_from_options(&options));
        let reader = Reader::from_options(&options).source(source);
        const SKIM_DEFAULT_COMMAND: &str = "find .";
        let default_command = String::from(match env::var("SKIM_DEFAULT_COMMAND").as_deref() {
            Err(_) | Ok("") => SKIM_DEFAULT_COMMAND,
            Ok(v) => v,
        });
        let cmd = options.cmd.clone().unwrap_or(default_command);

        let app = App::from_options(options, theme.clone(), cmd.clone());

        //------------------------------------------------------------------------------
        // reader
        // In interactive mode, expand all placeholders ({}, {q}, etc) with initial query (empty or from --query)
        let initial_cmd = if app.options.interactive && app.options.cmd.is_some() {
            let expanded = app.expand_cmd(&cmd, true);
            log::debug!(
                "Interactive mode: initial_cmd = {:?} (from template {:?})",
                expanded,
                cmd
            );
            expanded
        } else {
            cmd.clone()
        };
        Ok(Self {
            app,
            height,
            reader,
            reader_done: false,
            initial_cmd,
            tui: None,
            reader_control: None,
            matcher_interval: None,
            listener: None,
            final_event: Event::Quit,
            final_key: KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        })
    }

    /// Start the reader and matcher, but do not enter the TUI yet
    pub fn start(&mut self) {
        debug!("Starting reader with initial_cmd: {:?}", self.initial_cmd);
        self.reader_control = Some(self.reader.collect(self.app.item_pool.clone(), &self.initial_cmd));
        self.app.restart_matcher(true);
    }

    /// Handle a reload event by killing the current reader, clearing items, and starting a new reader.
    ///
    /// This encapsulates the reload logic from the main event loop so it can
    /// be reused by test harnesses without reimplementing it.
    pub fn handle_reload(&mut self, new_cmd: &str) {
        debug!("reloading with cmd {new_cmd}");
        // Kill the current reader
        if let Some(rc) = self.reader_control.as_mut() {
            rc.kill()
        }
        // Clear items
        self.app.item_pool.clear();
        // Clear displayed items unless no_clear_if_empty is set
        if !self.app.options.no_clear_if_empty {
            self.app.item_list.clear();
        }
        self.app.restart_matcher(true);
        // Start a new reader with the new command
        self.reader_control = Some(self.reader.collect(self.app.item_pool.clone(), new_cmd));
        self.reader_done = false;
    }

    /// Check if the reader has finished and restart the matcher if needed.
    ///
    /// This encapsulates the reader-status check from the main event loop
    /// so it can be reused by test harnesses.
    ///
    /// Returns `true` if the reader has completed.
    pub fn check_reader(&mut self) -> bool {
        if self.reader_control.as_ref().is_some_and(|rc| rc.is_done()) && !self.reader_done {
            self.reader_done = true;
            self.app.restart_matcher(false);
            true
        } else {
            false
        }
    }

    /// Returns `true` if the reader is done (has finished producing items).
    pub fn reader_done(&self) -> bool {
        self.reader_done && self.reader_control.as_ref().is_none_or(|rc| rc.is_done())
    }

    /// Returns `true` if the matcher is stopped
    pub fn matcher_stopped(&self) -> bool {
        self.app.matcher_control.stopped()
    }

    /// Initialize the TUI with a caller-provided instance.
    ///
    /// Use this instead of [`init_tui()`](Skim::init_tui) when you need a
    /// non-default backend (e.g. `TestBackend` for snapshot tests).
    pub fn init_tui_with(&mut self, tui: Tui<Backend>) {
        self.tui = Some(tui);
    }

    /// Returns a shared reference to the application state.
    pub fn app(&self) -> &App {
        &self.app
    }

    /// Returns a mutable reference to the application state.
    pub fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    /// Returns a shared reference to the TUI.
    ///
    /// # Panics
    ///
    /// Panics if the TUI has not been initialized yet.
    pub fn tui_ref(&self) -> &Tui<Backend> {
        self.tui.as_ref().expect("TUI needs to be initialized before access")
    }

    /// Returns a mutable reference to the TUI.
    ///
    /// # Panics
    ///
    /// Panics if the TUI has not been initialized yet.
    pub fn tui_mut(&mut self) -> &mut Tui<Backend> {
        self.tui.as_mut().expect("TUI needs to be initialized before access")
    }

    /// Returns mutable references to both the app and the TUI simultaneously.
    ///
    /// This is useful when you need to call `app.handle_event(tui, ...)` or
    /// `tui.draw(|frame| frame.render_widget(app, ...))`, which require
    /// disjoint mutable borrows of both fields.
    ///
    /// # Panics
    ///
    /// Panics if the TUI has not been initialized yet.
    pub fn app_and_tui(&mut self) -> (&mut App, &mut Tui<Backend>) {
        (
            &mut self.app,
            self.tui.as_mut().expect("TUI needs to be initialized before access"),
        )
    }

    /// Returns a shared reference to the final event that caused skim to quit.
    pub fn final_event(&self) -> &Event {
        &self.final_event
    }

    /// Returns a clone of the TUI event sender.
    ///
    /// Use this to send events (e.g. [`Event::Render`], [`Event::Action`])
    /// to the running skim instance from outside the event loop. The sender
    /// is cheap to clone and can be moved into async blocks or other tasks.
    ///
    /// Must be called after [`init_tui()`](Skim::init_tui).
    pub fn event_sender(&self) -> tokio::sync::mpsc::Sender<Event> {
        self.tui
            .as_ref()
            .expect("TUI needs to be initialized using Skim::init_tui before getting the event sender")
            .event_tx
            .clone()
    }

    /// Enter the TUI
    pub async fn enter(&mut self) -> Result<()> {
        debug!("Entering TUI");
        self.init_listener().await?;
        self.tui
            .as_mut()
            .expect("TUI needs to be initialized using Skim::init_tui before entering")
            .enter()
    }

    /// Checks read-0 select-1, filter, and sync to wait and returns whether or not we should enter
    fn should_enter(&mut self) -> bool {
        let reader_control = self
            .reader_control
            .as_ref()
            .expect("reader_control needs to be initilized using Skim::start");
        let app = &mut self.app;

        // Filter mode: wait for all items to be read and matched, then return without entering TUI
        if app.options.filter.is_some() {
            trace!("filter mode: waiting for all items to be processed");
            loop {
                let matcher_stopped = app.matcher_control.stopped();
                let reader_done = reader_control.is_done();
                if matcher_stopped && reader_done && app.item_pool.num_not_taken() == 0 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(1));
                app.restart_matcher(false);
            }
            app.item_list.items = app.item_list.processed_items.lock().take().unwrap_or_default().items;
            debug!("filter mode: matched {} items", app.item_list.items.len());
            return false;
        }

        // Deal with read-0 / select-1
        let min_items_before_enter = if app.options.exit_0 {
            1
        } else if app.options.select_1 {
            2
        } else if app.options.sync {
            usize::MAX
        } else {
            0
        };
        if min_items_before_enter > 0 || app.options.sync {
            trace!(
                "checking matcher, stopped: {}, processed: {}, matched: {}/{}, pool: {}, query: {}, reader_control_done: {}",
                app.matcher_control.stopped(),
                app.matcher_control.get_num_processed(),
                app.matcher_control.get_num_matched(),
                min_items_before_enter,
                app.item_pool.num_not_taken(),
                app.input.value,
                reader_control.is_done()
            );
            while app.matcher_control.get_num_matched() < min_items_before_enter
                && (!app.matcher_control.stopped() || !reader_control.is_done())
            {
                trace!("still waiting");
                std::thread::sleep(Duration::from_millis(10));
                app.restart_matcher(false);
            }
            trace!(
                "checked matcher, stopped: {}, processed: {}, pool: {}, query: {}, reader_control_done: {}",
                app.matcher_control.stopped(),
                app.matcher_control.get_num_processed(),
                app.item_pool.num_not_taken(),
                app.input.value,
                reader_control.is_done()
            );
            trace!(
                "checking for matched item count before entering: {}/{min_items_before_enter}",
                app.matcher_control.get_num_matched()
            );
            if app.matcher_control.get_num_matched() == min_items_before_enter - 1 {
                app.item_list.items = app.item_list.processed_items.lock().take().unwrap_or_default().items;
                debug!("early exit, result: {:?}", app.results());
                return false;
            };
        }
        true
    }

    /// Initialize the IPC socket listener
    /// This needs to be called from an async context despite being sync
    async fn init_listener(&mut self) -> Result<()> {
        if let Some(socket_name) = &self.app.options.listen {
            self.listener = Some(
                interprocess::local_socket::ListenerOptions::new()
                    .name(interprocess::local_socket::ToNsName::to_ns_name::<
                        interprocess::local_socket::GenericNamespaced,
                    >(socket_name.to_owned())?)
                    .create_tokio()?,
            )
        }
        Ok(())
    }

    /// Capture `self` and extract the output
    /// This will perform cleanup
    pub fn output(mut self) -> SkimOutput {
        if let Some(mut rc) = self.reader_control.take() {
            rc.kill()
        }

        // Extract final_key and is_abort from final_event
        let is_abort = !matches!(&self.final_event, Event::Action(Action::Accept(_)));

        SkimOutput {
            cmd: if self.app.options.interactive {
                // In interactive mode, cmd is what the user typed
                self.app.input.to_string()
            } else if self.app.options.cmd_query.is_some() {
                // If cmd_query was provided, use that for output
                self.app.options.cmd_query.clone().unwrap()
            } else {
                // Otherwise use the execution command
                self.initial_cmd
            },
            final_event: self.final_event,
            final_key: self.final_key,
            query: self.app.input.to_string(),
            is_abort,
            selected_items: self.app.results(),
            current: self.app.item_list.selected(),
            header: self.app.header.header.clone(),
        }
    }

    /// Returns true if skim has finished (the user accepted or aborted)
    pub fn should_quit(&self) -> bool {
        self.app.should_quit
    }

    /// Process a single event loop iteration.
    ///
    /// This awaits the next event from the TUI, matcher, or IPC listener,
    /// processes it, and returns. Use this in your own event loop when you
    /// need fine-grained control over the application lifecycle.
    ///
    /// Returns `Ok(true)` if skim should quit, `Ok(false)` to continue.
    ///
    /// # Example
    ///
    /// ```ignore
    /// while !skim.tick().await? {
    ///     // do your own work between ticks
    /// }
    /// ```
    pub async fn tick(&mut self) -> Result<bool> {
        let matcher_interval = &mut self.matcher_interval;
        select! {
            event = self.tui.as_mut().expect("TUI should be initialized before the event loop can start").next() => {
                let evt = event.ok_or_eyre("Could not acquire next event")?;

                if let Event::Key(k) = &evt {
                  self.final_key = k.to_owned();
                } else {
                  self.final_event = evt.to_owned();
                }


                // Handle reload event separately
                if let Event::Reload(new_cmd) = &evt {
                    self.handle_reload(&new_cmd.clone());
                } else {
                    self.app.handle_event(self.tui.as_mut().expect("TUI should be initialized before handling events"), &evt)?;
                }

                // Check reader status and update
                self.check_reader();
            }
            _ = async {
                match matcher_interval {
                    Some(interval) => { interval.tick().await; },
                    None => std::future::pending::<()>().await,
                }
            } => {
              self.app.restart_matcher(false);
            }
            Ok(stream) = async {
                match &self.listener {
                    Some(l) => interprocess::local_socket::traits::tokio::Listener::accept(l).await,
                    None => std::future::pending().await,
                }
            } => {
                debug!("Listener accepted a connection");
                let event_tx_clone_ipc = self.tui.as_ref().expect("TUI should be initialized before listening").event_tx.clone();
                tokio::spawn(async move {
                    use tokio::io::AsyncBufReadExt;
                    let reader = tokio::io::BufReader::new(stream);
                    let mut lines = reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        debug!("listener: got {line}");
                        if let Ok(act) = ron::from_str::<Action>(&line) {
                            debug!("listener: parsed into action {act:?}");
                            _ = event_tx_clone_ipc.try_send(Event::Action(act));
                        }
                    }
                });
            }
        }

        Ok(self.app.should_quit)
    }

    /// Run the event loop on the current task until skim quits.
    ///
    /// This is a convenience wrapper around [`tick()`](Self::tick) that loops
    /// until the user accepts or aborts. Use `tick()` directly if you need
    /// to interleave your own logic between iterations.
    pub async fn run(&mut self) -> Result<()> {
        self.matcher_interval = Some(tokio::time::interval(Duration::from_millis(100)));
        trace!("Starting event loop");
        loop {
            if self.tick().await? {
                break Ok(());
            }
        }
    }

    /// Spawn the event loop and run a user-provided future concurrently.
    ///
    /// This consumes `self`, spawns the event loop as a local task, and runs
    /// `user_task` alongside it. When the user accepts or aborts in the TUI,
    /// the event loop completes and the [`SkimOutput`] is returned — regardless
    /// of whether `user_task` has finished.
    ///
    /// Use this when you need to send items or do other work concurrently
    /// while the TUI is running.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let output = skim.run_until(async {
    ///     for i in 1..=10 {
    ///         tx.send(vec![Arc::new(format!("item {i}"))]);
    ///         tokio::time::sleep(Duration::from_millis(100)).await;
    ///     }
    /// }).await?;
    /// ```
    pub async fn run_until<F: Future + 'static>(mut self, user_task: F) -> Result<SkimOutput> {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let handle = tokio::task::spawn_local(async move {
                    self.run().await?;
                    Ok(self.output())
                });
                tokio::task::spawn_local(user_task);
                handle.await?
            })
            .await
    }
}
