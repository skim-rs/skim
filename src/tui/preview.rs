use ansi_to_tui::IntoText;
use color_eyre::eyre::{Result, eyre};
use portable_pty::{PtyPair, PtySize, native_pty_system};
use ratatui::{
    prelude::Backend,
    style::Stylize,
    text::{Line, Text},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};
use std::process::Command;
use std::sync::mpsc;
use std::thread::JoinHandle;

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
    pub content: Arc<RwLock<Text<'static>>>,
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
            content: Arc::new(RwLock::new(Text::default())),
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
        *content = text;
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
            self.init_pty();
            trace!("spawning preview cmd {cmd} in pty");
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

            self.thread_handle = Some(std::thread::spawn(move || {
                let mut res = Vec::with_capacity(PREVIEW_MAX_BYTES);

                trace!("[{:?}] preview reader thread started", std::thread::current().id(),);

                // Read what's available with a simple timeout
                let mut buf = [0; 1];
                loop {
                    if interrupt_rx.try_recv().is_ok() {
                        trace!("[{:?}] interrupt signal received, exiting", std::thread::current().id());
                        return;
                    }

                    if res.len() >= PREVIEW_MAX_BYTES {
                        trace!("[{:?}] reached PREVIEW_MAX_BYTES, exiting", std::thread::current().id());
                        break;
                    }

                    // Use a small read with immediate processing
                    match reader.read_exact(&mut buf) {
                        Ok(()) => {
                            trace!("[{:?}] read 1 byte", std::thread::current().id());
                            res.push(buf[0]);

                            // Check interrupt before updating shared state
                            if interrupt_rx.try_recv().is_ok() {
                                trace!(
                                    "[{:?}] interrupt signal received during read block, exiting",
                                    std::thread::current().id(),
                                );
                                break;
                            }
                        }
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                                trace!("[{:?}] reached EOF", std::thread::current().id());
                            } else {
                                trace!("[{:?}] read error {:?}", std::thread::current().id(), e);
                            }
                            break;
                        }
                    }
                }

                trace!(
                    "[{:?}] read complete, {} bytes total",
                    std::thread::current().id(),
                    res.len()
                );

                // Final update
                if let Ok(mut c) = content.write() {
                    *c = res.into_text().unwrap_or_default();
                }
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

                let out = try_out.unwrap();

                if interrupt_rx.try_recv().is_ok() {
                    return;
                }

                {
                    let Ok(mut c) = content.write() else {
                        return;
                    };
                    if out.status.success() {
                        *c = out.stdout[..PREVIEW_MAX_BYTES.min(out.stdout.len())]
                            .iter()
                            .copied()
                            .filter(|c| *c != b'\r')
                            .collect::<Vec<u8>>()
                            .into_text()
                            .unwrap_or_default();
                    } else {
                        *c = out.stderr.to_owned().into_text().unwrap_or_default();
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
            content: Default::default(),
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
        res.init_pty();
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

        // Calculate total lines in content
        let total_lines = content.lines.len();

        // Create paragraph with optional block
        let mut paragraph = Paragraph::new(content.clone()).scroll((self.scroll_y, self.scroll_x));

        // Enable wrapping if wrap is true
        if self.wrap {
            paragraph = paragraph.wrap(ratatui::widgets::Wrap { trim: false });
        }
        let mut block = Block::new().style(self.theme.normal).border_style(self.theme.border);

        // Add scroll position indicator at bottom if scrolled
        if self.scroll_y > 0 && total_lines > 0 {
            let current_line = (self.scroll_y + 1) as usize; // +1 because scroll_y is 0-indexed but we want 1-indexed display
            let title = format!("{}/{}", current_line, total_lines);
            use ratatui::layout::Alignment;

            block = block.title_top(Line::from(title).alignment(Alignment::Right).reversed());
        }

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
        paragraph = paragraph.block(block);

        Clear.render(area, buf);
        paragraph.render(area, buf);

        SkimRender::default()
    }
}
