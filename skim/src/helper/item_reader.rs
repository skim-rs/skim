//! Helper utilities for converting input sources into skim item streams.

use std::env;
use std::error::Error;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;

use regex::Regex;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::field::FieldRange;
use crate::helper::item::DefaultSkimItem;
use crate::reader::CommandCollector;
use crate::{SkimItem, SkimItemReceiver, SkimItemSender, SkimOptions};

const DELIMITER_STR: &str = r"[\t\n ]+";
const READ_BUFFER_SIZE: usize = 1024;

pub enum CollectorInput {
    Pipe(Box<dyn BufRead + Send>),
    Command(String),
}

/// Options for configuring how items are read and parsed
#[derive(Debug)]
pub struct SkimItemReaderOption {
    buf_size: usize,
    use_ansi_color: bool,
    transform_fields: Vec<FieldRange>,
    matching_fields: Vec<FieldRange>,
    delimiter: Regex,
    line_ending: u8,
    show_error: bool,
}

impl Default for SkimItemReaderOption {
    fn default() -> Self {
        Self {
            buf_size: READ_BUFFER_SIZE,
            line_ending: b'\n',
            use_ansi_color: false,
            transform_fields: Vec::new(),
            matching_fields: Vec::new(),
            delimiter: Regex::new(DELIMITER_STR).unwrap(),
            show_error: false,
        }
    }
}

impl SkimItemReaderOption {
    /// Creates reader options from skim options
    pub fn from_options(options: &SkimOptions) -> Self {
        Self {
            buf_size: READ_BUFFER_SIZE,
            line_ending: if options.read0 { b'\0' } else { b'\n' },
            use_ansi_color: options.ansi,
            transform_fields: options
                .with_nth
                .iter()
                .filter_map(|f| FieldRange::from_str(f))
                .collect(),
            matching_fields: options.nth.iter().filter_map(|f| FieldRange::from_str(f)).collect(),
            delimiter: options.delimiter.clone(),
            show_error: options.show_cmd_error,
        }
    }

    /// Sets the buffer size for reading
    pub fn buf_size(mut self, buf_size: usize) -> Self {
        self.buf_size = buf_size;
        self
    }

    /// Sets the line ending character (default: '\n')
    pub fn line_ending(mut self, line_ending: u8) -> Self {
        self.line_ending = line_ending;
        self
    }

    /// Enables or disables ANSI color code parsing
    pub fn ansi(mut self, enable: bool) -> Self {
        self.use_ansi_color = enable;
        self
    }

    /// Sets the field delimiter regex
    pub fn delimiter(mut self, delimiter: Regex) -> Self {
        self.delimiter = delimiter;
        self
    }

    /// Sets the fields to display (transform) from the input
    pub fn with_nth<'a, T>(mut self, with_nth: T) -> Self
    where
        T: Iterator<Item = &'a str>,
    {
        self.transform_fields = with_nth.filter_map(FieldRange::from_str).collect();
        self
    }

    /// Sets the transform fields directly
    pub fn transform_fields(mut self, transform_fields: Vec<FieldRange>) -> Self {
        self.transform_fields = transform_fields;
        self
    }

    /// Sets the fields to use for matching
    pub fn nth<'a, T>(mut self, nth: T) -> Self
    where
        T: Iterator<Item = &'a str>,
    {
        self.matching_fields = nth.filter_map(FieldRange::from_str).collect();
        self
    }

    /// Sets the matching fields directly
    pub fn matching_fields(mut self, matching_fields: Vec<FieldRange>) -> Self {
        self.matching_fields = matching_fields;
        self
    }

    /// Enables reading null-terminated lines instead of newline-terminated
    pub fn read0(mut self, enable: bool) -> Self {
        if enable {
            self.line_ending = b'\0';
        } else {
            self.line_ending = b'\n';
        }
        self
    }

    /// Sets whether to show command errors
    pub fn show_error(mut self, show_error: bool) -> Self {
        self.show_error = show_error;
        self
    }

    /// Builds the options (currently a no-op, returns self)
    pub fn build(self) -> Self {
        self
    }

    /// Returns true if no field transformations or ANSI parsing is needed
    pub fn is_simple(&self) -> bool {
        !self.use_ansi_color && self.matching_fields.is_empty() && self.transform_fields.is_empty()
    }
}

/// Reader for converting various input sources into streams of skim items
pub struct SkimItemReader {
    option: Arc<SkimItemReaderOption>,
}

impl Default for SkimItemReader {
    fn default() -> Self {
        Self {
            option: Arc::new(Default::default()),
        }
    }
}

impl SkimItemReader {
    /// Creates a new item reader with the given options
    pub fn new(option: SkimItemReaderOption) -> Self {
        Self {
            option: Arc::new(option),
        }
    }

    /// Sets the reader options
    pub fn option(mut self, option: SkimItemReaderOption) -> Self {
        self.option = Arc::new(option);
        self
    }
}

impl SkimItemReader {
    /// Converts a BufRead source into a stream of skim items
    pub fn of_bufread(&self, source: impl BufRead + Send + 'static) -> SkimItemReceiver {
        if self.option.is_simple() {
            self.raw_bufread(source)
        } else {
            self.read_and_collect_from_command(Arc::new(AtomicUsize::new(0)), CollectorInput::Pipe(Box::new(source)))
                .0
        }
    }

    /// helper: convert bufread into SkimItemReceiver
    fn raw_bufread(&self, mut source: impl BufRead + Send + 'static) -> SkimItemReceiver {
        let (tx_item, rx_item): (SkimItemSender, SkimItemReceiver) = unbounded_channel();
        let line_ending = self.option.line_ending;
        let use_ansi = self.option.use_ansi_color;
        let delimiter = self.option.delimiter.clone();
        thread::spawn(move || {
            let mut buffer = Vec::with_capacity(1024);
            let mut idx = 0;
            loop {
                buffer.clear();
                // start reading
                match source.read_until(line_ending, &mut buffer) {
                    Ok(n) => {
                        if n == 0 {
                            break;
                        }

                        if buffer.ends_with(b"\r\n") {
                            buffer.pop();
                            buffer.pop();
                        } else if buffer.ends_with(b"\n") || buffer.ends_with(b"\0") {
                            buffer.pop();
                        }

                        let string = String::from_utf8_lossy(&buffer);
                        //let result = tx_item.send(Arc::new(string.into_owned()));
                        let result = tx_item.send(Arc::new(DefaultSkimItem::new(
                            string.to_string(),
                            use_ansi,
                            &[],
                            &[],
                            &delimiter,
                            idx,
                        )));
                        if result.is_err() {
                            break;
                        }
                        idx += 1;
                    }
                    Err(_err) => {} // String not UTF8 or other error, skip.
                }
            }
        });
        rx_item
    }

    /// components_to_stop == 0 => all the threads have been stopped
    /// return (channel_for_receive_item, channel_to_stop_command)
    fn read_and_collect_from_command(
        &self,
        components_to_stop: Arc<AtomicUsize>,
        input: CollectorInput,
    ) -> (UnboundedReceiver<Arc<dyn SkimItem>>, UnboundedSender<i32>) {
        let (command, mut source) = match input {
            CollectorInput::Pipe(pipe) => (None, pipe),
            CollectorInput::Command(cmd) => get_command_output(&cmd).expect("command not found"),
        };

        let (tx_interrupt, mut rx_interrupt) = unbounded_channel();
        let (tx_item, rx_item): (SkimItemSender, SkimItemReceiver) = unbounded_channel::<Arc<dyn SkimItem>>();

        let started = Arc::new(AtomicBool::new(false));
        let started_clone = started.clone();
        let components_to_stop_clone = components_to_stop.clone();
        let tx_item_clone = tx_item.clone();
        let send_error = self.option.show_error;
        // listening to close signal and kill command if needed
        thread::spawn(move || {
            debug!("collector: command killer start");
            components_to_stop_clone.fetch_add(1, Ordering::SeqCst);
            started_clone.store(true, Ordering::SeqCst); // notify parent that it is started

            let _ = rx_interrupt.blocking_recv();
            if let Some(mut child) = command {
                // clean up resources
                let _ = child.kill();
                let _ = child.wait();

                if send_error {
                    let has_error = child
                        .try_wait()
                        .map(|os| os.map(|s| !s.success()).unwrap_or(true))
                        .unwrap_or(false);
                    if has_error {
                        let output = child.wait_with_output().expect("could not retrieve error message");
                        for line in String::from_utf8_lossy(&output.stderr).lines() {
                            let _ = tx_item_clone.send(Arc::new(line.to_string()));
                        }
                    }
                }
            }

            components_to_stop_clone.fetch_sub(1, Ordering::SeqCst);
            debug!("collector: command killer stop");
        });

        while !started.load(Ordering::SeqCst) {
            // busy waiting for the thread to start. (components_to_stop is added)
        }

        let started = Arc::new(AtomicBool::new(false));
        let started_clone = started.clone();
        let tx_interrupt_clone = tx_interrupt.clone();
        let option = self.option.clone();
        thread::spawn(move || {
            debug!("collector: command collector start");
            components_to_stop.fetch_add(1, Ordering::SeqCst);
            started_clone.store(true, Ordering::SeqCst); // notify parent that it is started

            let mut buffer = Vec::with_capacity(option.buf_size);
            let mut line_idx = 0;
            loop {
                buffer.clear();

                // start reading
                match source.read_until(option.line_ending, &mut buffer) {
                    Ok(0) => break,
                    Ok(_) => {
                        if buffer.ends_with(b"\r\n") {
                            buffer.pop();
                            buffer.pop();
                        } else if buffer.ends_with(b"\n") || buffer.ends_with(b"\0") {
                            buffer.pop();
                        }

                        let line = String::from_utf8_lossy(&buffer).to_string();

                        trace!("got item {} with index {} from command", line.clone(), line_idx);

                        let raw_item = DefaultSkimItem::new(
                            line,
                            option.use_ansi_color,
                            &option.transform_fields,
                            &option.matching_fields,
                            &option.delimiter,
                            line_idx,
                        );

                        match tx_item.send(Arc::new(raw_item)) {
                            Ok(_) => {}
                            Err(_) => {
                                debug!("collector: failed to send item, quit");
                                break;
                            }
                        }
                        line_idx += 1;
                    }
                    Err(_err) => {} // String not UTF8 or other error, skip.
                }
            }

            let _ = tx_interrupt_clone.send(1); // ensure the waiting thread will exit
            components_to_stop.fetch_sub(1, Ordering::SeqCst);
            debug!("collector: command collector stop");
        });

        while !started.load(Ordering::SeqCst) {
            // busy waiting for the thread to start. (components_to_stop is added)
        }

        (rx_item, tx_interrupt)
    }
}

impl CommandCollector for SkimItemReader {
    fn invoke(&mut self, cmd: &str, components_to_stop: Arc<AtomicUsize>) -> (SkimItemReceiver, UnboundedSender<i32>) {
        self.read_and_collect_from_command(components_to_stop, CollectorInput::Command(cmd.to_string()))
    }
}

type CommandOutput = (Option<Child>, Box<dyn BufRead + Send>);

fn get_command_output(cmd: &str) -> Result<CommandOutput, Box<dyn Error>> {
    let shell = env::var("SHELL").unwrap_or_else(|_| "sh".to_string());
    let mut command: Child = Command::new(shell)
        .arg("-c")
        .arg(cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = command
        .stdout
        .take()
        .ok_or_else(|| "command output: unwrap failed".to_owned())?;

    Ok((Some(command), Box::new(BufReader::new(stdout))))
}
