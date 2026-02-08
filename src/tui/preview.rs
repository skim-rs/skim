use ansi_to_tui::IntoText;
use color_eyre::eyre::{Result, eyre};
use portable_pty::{PtyPair, PtySize, native_pty_system};
use ratatui::{
    prelude::Backend,
    style::Stylize,
    text::{Line, Text},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};
use std::io::Read;
use std::process::Command;
use std::sync::mpsc;
use std::thread::JoinHandle;
use tui_term::vt100;
use tui_term::widget::PseudoTerminal;

use super::Direction;
use super::Event;
use super::Tui;

use crate::theme::ColorTheme;
use crate::tui::BorderType;
use crate::tui::widget::{SkimRender, SkimWidget};
use crate::{SkimItem, SkimOptions};
use std::sync::{Arc, RwLock};

// PreviewCallback for ratatui - returns Vec<String> instead of AnsiString
pub type PreviewCallbackFn = dyn Fn(Vec<Arc<dyn SkimItem>>) -> Vec<String> + Send + Sync + 'static;
const PREVIEW_MAX_BYTES: usize = 100 * 1024;

/// Preview content can be either parsed text or a terminal screen
pub(crate) enum PreviewContent {
    /// Simple text content (for non-PTY previews and callbacks)
    Text(Text<'static>),
    /// Terminal screen (for PTY previews with cursor positioning)
    Terminal(Arc<RwLock<vt100::Parser>>),
}

impl Default for PreviewContent {
    fn default() -> Self {
        PreviewContent::Text(Text::default())
    }
}

/// Callback function for generating preview content
#[derive(Clone)]
pub struct PreviewCallback {
    inner: Arc<PreviewCallbackFn>,
}

impl<F> From<F> for PreviewCallback
where
    F: Fn(Vec<Arc<dyn SkimItem>>) -> Vec<String> + Send + Sync + 'static,
{
    fn from(func: F) -> Self {
        Self { inner: Arc::new(func) }
    }
}

impl std::ops::Deref for PreviewCallback {
    type Target = dyn Fn(Vec<Arc<dyn SkimItem>>) -> Vec<String> + Send + Sync + 'static;

    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}

pub struct Preview {
    pub(crate) content: Arc<RwLock<PreviewContent>>,
    pub cmd: String,
    pub rows: u16,
    pub cols: u16,
    pub scroll_y: u16,
    pub scroll_x: u16,
    pub thread_handle: Option<JoinHandle<()>>,
    /// Channel to signal thread interruption
    interrupt_tx: Option<mpsc::Sender<()>>,
    pub theme: Arc<ColorTheme>,
    /// Border type, if borders are enabled
    pub border: Option<BorderType>,
    pub direction: Direction,
    pub wrap: bool,
    pty: Option<PtyPair>,
    pty_child: Option<Box<dyn portable_pty::Child + Send + Sync>>,
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            content: Arc::new(RwLock::new(PreviewContent::Text(Text::default()))),
            cmd: String::default(),
            rows: 0,
            cols: 0,
            scroll_y: 0,
            scroll_x: 0,
            thread_handle: None,
            interrupt_tx: None,
            theme: Arc::new(ColorTheme::default()),
            border: None,
            direction: Direction::Right,
            wrap: false,
            pty: None,
            pty_child: None,
        }
    }
}

impl Preview {
    /// Convert a Size value to an actual offset based on preview dimensions
    fn size_to_offset(&self, size: super::Size, is_vertical: bool) -> u16 {
        match size {
            super::Size::Fixed(n) => n,
            super::Size::Percent(p) => {
                let dimension = if is_vertical { self.rows } else { self.cols };
                (dimension as u32 * p as u32 / 100) as u16
            }
        }
    }

    fn init_pty(&mut self) {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows: self.rows,
            cols: self.cols,
            pixel_width: 0,
            pixel_height: 0,
        });
        match pair {
            Ok(p) => self.pty = Some(p),
            Err(e) => warn!("failed to init preview pty: {e:?}"),
        }
    }

    pub fn content(&mut self, content: Vec<u8>) -> Result<()> {
        let text = content.to_owned().into_text()?;
        let Ok(mut content) = self.content.write() else {
            return Err(color_eyre::eyre::eyre!("Failed to acquire content for writing"));
        };
        *content = PreviewContent::Text(text);
        self.scroll_y = 0;
        self.scroll_x = 0;
        Ok(())
    }

    pub fn content_with_position(&mut self, content: Vec<u8>, position: crate::PreviewPosition) -> Result<()> {
        self.content(content).map(|_| {
            // Apply position offsets
            let v_scroll = self.size_to_offset(position.v_scroll, true);
            let v_offset = self.size_to_offset(position.v_offset, true);
            self.scroll_y = v_scroll.saturating_add(v_offset);

            let h_scroll = self.size_to_offset(position.h_scroll, false);
            let h_offset = self.size_to_offset(position.h_offset, false);
            self.scroll_x = h_scroll.saturating_add(h_offset);
        })
    }

    pub fn scroll_up(&mut self, lines: u16) {
        self.scroll_y = self.scroll_y.saturating_sub(lines);
    }

    pub fn scroll_down(&mut self, lines: u16) {
        self.scroll_y = self.scroll_y.saturating_add(lines);
    }

    pub fn scroll_left(&mut self, cols: u16) {
        self.scroll_x = self.scroll_x.saturating_sub(cols);
    }

    pub fn scroll_right(&mut self, cols: u16) {
        self.scroll_x = self.scroll_x.saturating_add(cols);
    }

    pub fn set_offset(&mut self, offset: u16) {
        self.scroll_y = offset.saturating_sub(1); // -1 because line numbers are 1-indexed
    }

    pub fn page_up(&mut self) {
        let page_size = self.rows.saturating_sub(2); // Account for borders
        self.scroll_up(page_size);
    }

    pub fn page_down(&mut self) {
        let page_size = self.rows.saturating_sub(2); // Account for borders
        self.scroll_down(page_size);
    }
    /// Kill the preview child process and interrupt the reader thread.
    pub fn kill(&mut self) {
        if let Some(tx) = self.interrupt_tx.take() {
            let _ = tx.send(());
        }

        if let Some(mut child) = self.pty_child.take() {
            trace!("killing pty child process");
            match child.try_wait() {
                Ok(Some(status)) => {
                    trace!("child already exited with status: {status:?}");
                }
                Ok(None) => {
                    trace!("child still running, sending kill signal");
                    if let Err(e) = child.kill() {
                        trace!("failed to kill pty child: {e:?}");
                    }
                }
                Err(e) => {
                    debug!("error checking child status: {e:?}");
                    let _ = child.kill();
                }
            }
        }
    }

    pub fn spawn<B: Backend>(&mut self, tui: &mut Tui<B>, cmd: &str) -> Result<()>
    where
        B::Error: Send + Sync + 'static,
    {
        self.kill();
        self.cmd = cmd.to_string();

        let event_tx_clone = tui.event_tx.clone();
        let content = self.content.clone();

        if let Some(pty) = self.pty.take() {
            trace!("spawning preview cmd {cmd} in pty");
            self.init_pty();
            trace!("initalized pty");
            let mut shell_cmd = portable_pty::CommandBuilder::new("/bin/sh");
            shell_cmd.env("ROWS", self.rows.to_string());
            shell_cmd.env("COLUMNS", self.cols.to_string());
            shell_cmd.env("PAGER", "");
            shell_cmd.arg("-c");
            if let Ok(cwd) = nix::unistd::getcwd() {
                shell_cmd.cwd(cwd);
            }
            shell_cmd.arg(cmd);
            self.pty_child = Some(pty.slave.spawn_command(shell_cmd).map_err(|e| {
                warn!("{:#?}", e.backtrace());
                eyre!(Box::<dyn std::error::Error + Send + Sync + 'static>::from(e))
            })?);

            let mut reader = pty
                .master
                .try_clone_reader()
                .map_err(|e| eyre!(Box::<dyn std::error::Error + Send + Sync + 'static>::from(e)))?;

            let (interrupt_tx, interrupt_rx) = mpsc::channel();
            self.interrupt_tx = Some(interrupt_tx);

            // Create vt100 parser for PTY output
            let parser = Arc::new(RwLock::new(vt100::Parser::new(self.rows, self.cols, 0)));

            // Update content to use the parser
            if let Ok(mut c) = content.write() {
                *c = PreviewContent::Terminal(parser.clone());
            }

            self.thread_handle = Some(std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                let mut processed_buf = Vec::new();

                trace!("preview reader thread started");

                loop {
                    if interrupt_rx.try_recv().is_ok() {
                        trace!("interrupt signal received, exiting");
                        return;
                    }

                    match reader.read(&mut buf) {
                        Ok(0) => {
                            trace!("reached EOF");
                            break;
                        }
                        Ok(size) => {
                            trace!("read {} bytes", size);
                            processed_buf.extend_from_slice(&buf[..size]);

                            if let Ok(mut parser_guard) = parser.write() {
                                let parser_ref: &mut vt100::Parser = &mut parser_guard;
                                parser_ref.process(&processed_buf);
                            }

                            // Clear the processed portion of the buffer
                            processed_buf.clear();

                            // Check interrupt after processing
                            if interrupt_rx.try_recv().is_ok() {
                                trace!("interrupt signal received during read block, exiting");
                                return;
                            }
                        }
                        Err(e) => {
                            trace!("read error {:?}", e);
                            break;
                        }
                    }
                }

                trace!("read complete");
                let _ = event_tx_clone.send(Event::PreviewReady);
            }));
        } else {
            trace!("spawning preview cmd {cmd}");
            let mut shell_cmd = Command::new("/bin/sh");
            shell_cmd
                .env("ROWS", self.rows.to_string())
                .env("COLUMNS", self.cols.to_string())
                .env("PAGER", "")
                .arg("-c")
                .arg(cmd);
            if let Ok(cwd) = nix::unistd::getcwd() {
                shell_cmd.current_dir(cwd);
            }

            let (interrupt_tx, interrupt_rx) = mpsc::channel();
            self.interrupt_tx = Some(interrupt_tx);

            self.thread_handle = Some(std::thread::spawn(move || {
                if interrupt_rx.try_recv().is_ok() {
                    return;
                }

                let try_out = shell_cmd.output();
                if try_out.is_err() {
                    println!("Shell cmd in error: {:?}", try_out);
                    return;
                };

                let mut out = try_out.unwrap();

                if interrupt_rx.try_recv().is_ok() {
                    return;
                }

                if let Ok(mut c) = content.write() {
                    if out.status.success() {
                        out.stdout.resize(PREVIEW_MAX_BYTES.min(out.stdout.len()), 0);
                        *c = PreviewContent::Text(out.stdout.into_text().unwrap_or_default());
                    } else {
                        *c = PreviewContent::Text(out.stderr.to_owned().into_text().unwrap_or_default());
                    }
                }

                trace!("sending ready ping");
                let _ = event_tx_clone.send(Event::PreviewReady);
            }));
        }
        Ok(())
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        if let Some(pty) = self.pty.take() {
            let w = pty.master.take_writer();
            drop(w);
        }
        self.kill();
    }
}

impl SkimWidget for Preview {
    fn from_options(options: &SkimOptions, theme: Arc<ColorTheme>) -> Self {
        let mut res = Self {
            theme,
            border: options.border,
            direction: options.preview_window.direction,
            wrap: options.preview_window.wrap,
            content: Arc::new(RwLock::new(PreviewContent::default())),
            cmd: Default::default(),
            rows: 0,
            cols: 0,
            scroll_y: 0,
            scroll_x: 0,
            thread_handle: None,
            interrupt_tx: None,
            pty: None,
            pty_child: None,
        };
        #[cfg(target_os = "linux")]
        if std::env::var("SKIM_FLAG_NO_PREVIEW_PTY").is_err() {
            res.init_pty();
        }
        res
    }

    fn render(&mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) -> SkimRender {
        if self.rows != area.height || self.cols != area.width {
            self.rows = area.height;
            self.cols = area.width;
            self.pty.as_ref().map(|p| {
                p.master.resize(PtySize {
                    rows: self.rows,
                    cols: self.cols,
                    ..Default::default()
                })
            });
        }
        let Ok(content) = self.content.try_read() else {
            return SkimRender::default();
        };

        let mut block = Block::new().style(self.theme.normal).border_style(self.theme.border);

        // Add borders based on direction and border setting
        if let Some(border_type) = self.border {
            block = block.borders(Borders::ALL).border_type(border_type.into());
        } else {
            // No border on preview itself - separator will be drawn between areas
            match self.direction {
                Direction::Up => block = block.borders(Borders::BOTTOM),
                Direction::Down => block = block.borders(Borders::TOP),
                Direction::Left => block = block.borders(Borders::RIGHT),
                Direction::Right => block = block.borders(Borders::LEFT),
            };
        }

        Clear.render(area, buf);

        match &*content {
            PreviewContent::Text(text) => {
                // Calculate total lines in content
                let total_lines = text.lines.len();

                // Create paragraph with optional block
                let mut paragraph = Paragraph::new(text.clone()).scroll((self.scroll_y, self.scroll_x));

                // Enable wrapping if wrap is true
                if self.wrap {
                    paragraph = paragraph.wrap(ratatui::widgets::Wrap { trim: false });
                }

                // Add scroll position indicator at top-right if scrolled
                if self.scroll_y > 0 && total_lines > 0 {
                    let current_line = (self.scroll_y + 1) as usize; // +1 because scroll_y is 0-indexed but we want 1-indexed display
                    let title = format!("{}/{}", current_line, total_lines);
                    use ratatui::layout::Alignment;

                    block = block.title_top(Line::from(title).alignment(Alignment::Right).reversed());
                }

                paragraph = paragraph.block(block);
                paragraph.render(area, buf);
            }
            PreviewContent::Terminal(parser) => {
                // For terminal content, use PseudoTerminal widget
                if let Ok(parser_guard) = parser.try_read() {
                    let parser_ref: &vt100::Parser = &parser_guard;
                    let screen = parser_ref.screen();
                    let pseudo_term = PseudoTerminal::new(screen).block(block);
                    pseudo_term.render(area, buf);
                }
            }
        }

        SkimRender::default()
    }
}
