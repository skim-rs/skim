use ansi_to_tui::IntoText;
use color_eyre::eyre::{Result, eyre};
use portable_pty::{PtyPair, PtySize, native_pty_system};
use ratatui::{
    layout::Alignment,
    prelude::Backend,
    style::Stylize,
    text::{Line, Text},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::Protocol as ImageProtocol;
use tui_term::vt100;
use tui_term::widget::PseudoTerminal;

use std::sync::mpsc;
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;
use std::time::Instant;
use std::{env, io::Read};

use super::statusline::spinner_char;
use super::util::{find_csi_end, find_osc_end, handle_csi_query, handle_osc_query};
use super::widget::{SkimRender, SkimWidget};
use super::{BorderType, Direction, Event, Tui};

use crate::theme::ColorTheme;
use crate::{SkimItem, SkimOptions};

// PreviewCallback for ratatui - returns Vec<String> instead of AnsiString
pub type PreviewCallbackFn = dyn Fn(Vec<Arc<dyn SkimItem>>) -> Vec<String> + Send + Sync + 'static;
const PREVIEW_MAX_BYTES: usize = 1024 * 1024;
const VT_SCROLLBACK: usize = 100_000;

/// Preview content options
pub(crate) enum PreviewContent {
    /// Simple text content (for non-PTY previews and callbacks)
    Text(Text<'static>),
    /// Terminal screen (for PTY previews with cursor positioning)
    Terminal(Arc<RwLock<vt100::Parser>>),
    /// Image
    Image {
        source: image::DynamicImage,
        protocol: Option<ImageProtocol>,
        size: ratatui::layout::Size,
    },
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
    /// Border type
    pub border: BorderType,
    pub direction: Direction,
    pub wrap: bool,
    pty: Option<PtyPair>,
    pty_child: Option<Box<dyn portable_pty::Child + Send + Sync>>,
    image: bool,
    image_picker: Option<Picker>,
    pub total_lines: u16,
    loading: bool,
    spinner_start: Instant,
}

impl Default for Preview {
    fn default() -> Self {
        Self::_default()
    }
}

impl Preview {
    fn image_protocol(
        picker: Option<&Picker>,
        source: image::DynamicImage,
        size: ratatui::layout::Size,
    ) -> std::result::Result<ImageProtocol, ratatui_image::errors::Errors> {
        let fallback;
        let picker = if let Some(picker) = picker {
            picker
        } else {
            fallback = Picker::halfblocks();
            &fallback
        };
        picker.new_protocol(source, size, ratatui_image::Resize::Fit(None))
    }

    pub(crate) fn set_image_picker(&mut self, picker: Option<Picker>) {
        self.image_picker = picker;
    }

    /// Convert a Size value to an actual offset based on preview dimensions
    fn size_to_offset(&self, size: super::Size, is_vertical: bool) -> u16 {
        match size {
            super::Size::Fixed(n) => n,
            super::Size::Percent(p) => {
                let dimension = if is_vertical { self.rows } else { self.cols };
                // Result is at most dimension (a u16), so truncation cannot occur.
                u16::try_from(u32::from(dimension) * u32::from(p) / 100).unwrap_or(u16::MAX)
            }
            super::Size::Neg(n) => {
                let dimension = if is_vertical { self.rows } else { self.cols };
                dimension.saturating_sub(n)
            }
        }
    }

    fn init_pty(&mut self) {
        let pty_system = native_pty_system();
        let cols = if self.wrap { self.cols } else { 1024 };
        let pair = pty_system.openpty(PtySize {
            rows: self.rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
        match pair {
            Ok(p) => self.pty = Some(p),
            Err(e) => warn!("failed to init preview pty: {e:?}"),
        }
    }

    /// Filter out terminal query sequences from the output and respond to them.
    /// This prevents programs like delta from waiting for responses and timing out.
    /// Returns the filtered output with query sequences removed.
    fn filter_and_respond_to_queries(data: &[u8], writer: &mut Box<dyn std::io::Write + Send>) -> Vec<u8> {
        let mut result = Vec::new();
        let mut i = 0;

        while i < data.len() {
            if data[i] == b'\x1b' && i + 1 < data.len() {
                match data[i + 1] {
                    b']' => {
                        // OSC sequences: ESC ] ... ST (where ST is ESC \ or BEL)
                        if let Some(end) = find_osc_end(&data[i..]) {
                            let seq = &data[i..i + end];
                            handle_osc_query(seq, writer);
                            i += end;
                            continue;
                        }
                    }
                    b'[' => {
                        // CSI sequences: ESC [ ... final_byte
                        if let Some(end) = find_csi_end(&data[i..]) {
                            let seq = &data[i..i + end];
                            if handle_csi_query(seq, writer) {
                                // It was a query, filter it out
                                i += end;
                                continue;
                            }
                            // Not a query, keep it in output
                        }
                    }
                    _ => {}
                }
            }
            result.push(data[i]);
            i += 1;
        }

        result
    }

    pub fn content(&mut self, content: &[u8]) -> Result<()> {
        let text = content.to_owned().into_text()?;
        let Ok(mut content) = self.content.write() else {
            return Err(color_eyre::eyre::eyre!("Failed to acquire content for writing"));
        };
        self.total_lines = text.lines.len().try_into().unwrap();
        *content = PreviewContent::Text(text);
        self.scroll_y = 0;
        self.scroll_x = 0;
        self.loading = false;
        Ok(())
    }

    pub(crate) fn is_loading(&self) -> bool {
        self.loading
    }

    pub(crate) fn mark_ready(&mut self) {
        self.loading = false;
    }

    pub fn content_with_position(&mut self, content: &[u8], position: crate::PreviewPosition) -> Result<()> {
        self.content(content).map(|()| {
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
        trace!(
            "scrolling down by {lines} lines, ({} total, {} rows)",
            self.total_lines, self.rows
        );
        if self.total_lines > 0 {
            self.scroll_y = self
                .scroll_y
                .saturating_add(lines)
                .min(self.total_lines.saturating_sub(self.rows.saturating_sub(1)));
        } else {
            // We might not have the actual total_lines value
            self.scroll_y = self.scroll_y.saturating_add(lines);
        }
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

    #[allow(clippy::too_many_lines)]
    pub fn spawn<B: Backend>(&mut self, tui: &mut Tui<B>, cmd: &str) -> Result<()>
    where
        B::Error: Send + Sync + 'static,
    {
        self.kill();
        self.cmd = cmd.to_string();
        self.loading = true;

        // Reset scroll position and manual_scroll flag for new preview
        self.scroll_y = 0;
        self.scroll_x = 0;

        let event_tx_clone = tui.event_tx.clone();
        let content = self.content.clone();

        if self.image {
            let cmd = self.cmd.clone();
            self.thread_handle = Some(std::thread::spawn(move || {
                let res: PreviewContent;
                if let Ok(Ok(decoded)) = image::ImageReader::open(&cmd).map(image::ImageReader::decode) {
                    res = PreviewContent::Image {
                        source: decoded,
                        protocol: None,
                        size: ratatui::layout::Size::default(),
                    };
                } else {
                    res = PreviewContent::Text(Text::raw(format!("Failed to open {cmd} as image")));
                }
                if let Ok(mut c) = content.write() {
                    *c = res;
                    let _ = event_tx_clone.blocking_send(Event::PreviewReady);
                }
            }));

            return Ok(());
        }

        if let Some(pty) = self.pty.take() {
            // Ensure the PTY has the correct display dimensions before spawning.
            // init_pty() creates PTYs with 1024 cols for non-wrap mode (for the vt100 parser's
            // horizontal scrolling), but the child process needs to see the actual display size.
            // render() only resizes when the area changes, so if spawn() is called twice at the
            // same area size, the second PTY would still have 1024 cols.
            if self.rows > 0 && self.cols > 0 {
                let _ = pty.master.resize(PtySize {
                    rows: self.rows,
                    cols: self.cols,
                    pixel_width: 0,
                    pixel_height: 0,
                });
            }
            trace!(
                "spawning preview cmd {cmd} in pty (rows={}, cols={})",
                self.rows, self.cols
            );
            self.init_pty();
            trace!("initialized pty");
            // no PTY on windows, we can keep sh
            let mut shell_cmd = portable_pty::CommandBuilder::new("/bin/sh");
            shell_cmd.env("ROWS", self.rows.to_string());
            shell_cmd.env("COLUMNS", self.cols.to_string());
            shell_cmd.env("PAGER", "");
            shell_cmd.arg("-c");
            if let Ok(cwd) = env::current_dir() {
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

            // Get a writer to respond to terminal queries
            let mut esc_writer = pty
                .master
                .take_writer()
                .map_err(|e| eyre!(Box::<dyn std::error::Error + Send + Sync + 'static>::from(e)))?;

            let (interrupt_tx, interrupt_rx) = mpsc::channel();
            self.interrupt_tx = Some(interrupt_tx);

            // Create vt100 parser for PTY output with large scrollback buffer
            let cols = if self.wrap { self.cols } else { 1024 };
            let parser = Arc::new(RwLock::new(vt100::Parser::new(self.rows, cols, VT_SCROLLBACK)));

            // Update content to use the parser
            if let Ok(mut c) = content.write() {
                *c = PreviewContent::Terminal(parser.clone());
            }

            self.thread_handle = Some(std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                let mut unprocessed_buf = Vec::new();

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
                            trace!("read {size} bytes");
                            unprocessed_buf.extend_from_slice(&buf[..size]);

                            // Filter out terminal query sequences and respond to them
                            // This prevents programs like delta from waiting for responses
                            let filtered = Self::filter_and_respond_to_queries(&unprocessed_buf, &mut esc_writer);

                            if let Ok(mut parser_guard) = parser.write() {
                                parser_guard.process(&filtered);
                                parser_guard.screen_mut().set_scrollback(VT_SCROLLBACK);
                            }

                            // Clear the processed portion of the buffer
                            unprocessed_buf.clear();

                            // Check interrupt after processing
                            if interrupt_rx.try_recv().is_ok() {
                                trace!("interrupt signal received during read block, exiting");
                                return;
                            }
                        }
                        Err(e) => {
                            trace!("read error {e:?}");
                            break;
                        }
                    }
                }

                trace!("read complete");
                let _ = event_tx_clone.blocking_send(Event::PreviewReady);
            }));
        } else {
            trace!("spawning preview cmd {cmd}");
            let mut shell_cmd = crate::shell_cmd(cmd);
            shell_cmd
                .env("ROWS", self.rows.to_string())
                .env("COLUMNS", self.cols.to_string())
                .env("PAGER", "");
            if let Ok(cwd) = env::current_dir() {
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
                    log::info!("Shell cmd in error: {try_out:?}");
                    let _ = event_tx_clone.blocking_send(Event::PreviewReady);
                    return;
                }

                let mut out = try_out.unwrap();

                if interrupt_rx.try_recv().is_ok() {
                    return;
                }

                if let Ok(mut c) = content.write() {
                    if out.status.success() {
                        out.stdout.resize(PREVIEW_MAX_BYTES.min(out.stdout.len()), 0);
                        *c = PreviewContent::Text(out.stdout.into_text().unwrap_or_default());
                    } else {
                        *c = PreviewContent::Text(out.stderr.clone().into_text().unwrap_or_default());
                    }
                }

                trace!("sending ready ping");
                let _ = event_tx_clone.blocking_send(Event::PreviewReady);
            }));
        }
        Ok(())
    }

    fn render_text(
        &self,
        mut outer: Block,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::prelude::Buffer,
        text: &Text,
    ) -> u16 {
        // Calculate total lines in content
        let total_lines: u16 = text.lines.len().try_into().unwrap();

        // Create paragraph with optional block
        let mut paragraph = Paragraph::new(text.clone()).scroll((self.scroll_y, self.scroll_x));

        // Enable wrapping if wrap is true
        if self.wrap {
            paragraph = paragraph.wrap(ratatui::widgets::Wrap { trim: false });
        }

        // Add scroll position indicator at top-right if scrolled
        if self.scroll_y > 0 && self.total_lines > 0 {
            let current_line = (self.scroll_y + 1) as usize; // +1 because scroll_y is 0-indexed but we want 1-indexed display
            let title = format!("{}/{}", current_line, self.total_lines);

            outer = outer.title_top(Line::from(title).alignment(Alignment::Right).reversed());
        }

        paragraph = paragraph.block(outer);
        paragraph.render(area, buf);
        total_lines
    }

    fn render_pty(
        &self,
        mut outer: Block,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::prelude::Buffer,
        parser: &std::sync::RwLock<tui_term::vt100::Parser>,
    ) -> u16 {
        let mut total_lines = 0u16;
        // For terminal content, manipulate scrollback to implement scrolling
        if let Ok(mut parser_guard) = parser.try_write() {
            let scrollback_len = parser_guard.screen().scrollback();
            // Reset scrollback to its full size first
            parser_guard.screen_mut().set_scrollback(VT_SCROLLBACK);
            // If the scrollback is not empty, we seem to be off by one
            total_lines = (scrollback_len.saturating_sub(1) + parser_guard.screen().contents().lines().count())
                .try_into()
                .unwrap();
            if self.scroll_y > 0 {
                trace!("scrolling in vt buffer: {}/{}", self.scroll_y, total_lines);
                // Reduce scrollback by scroll_y to show earlier content
                parser_guard
                    .screen_mut()
                    .set_scrollback(scrollback_len.saturating_sub(self.scroll_y.into()));
            }
        }

        // Render using PseudoTerminal widget for proper terminal emulation
        if let Ok(parser_guard) = parser.try_read() {
            let screen = parser_guard.screen();

            // Add scroll position indicator if scrolled
            if self.scroll_y > 0 && total_lines > 0 {
                let title = format!("{}/{}", self.scroll_y + 1, total_lines);
                outer = outer.title_top(Line::from(title).alignment(Alignment::Right).reversed());
            }

            // Use PseudoTerminal widget to render the vt100 screen
            let pseudo_term = PseudoTerminal::new(screen)
                .cursor(tui_term::widget::Cursor::default().visibility(false))
                .block(outer);
            pseudo_term.render(area, buf);
        }

        // Reset scrollback after rendering
        if self.scroll_y > 0
            && let Ok(mut parser_guard) = parser.try_write()
        {
            parser_guard.screen_mut().set_scrollback(VT_SCROLLBACK);
        }
        total_lines
    }
    fn render_image(
        &self,
        mut outer: Block,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::prelude::Buffer,
        source: &image::DynamicImage,
        protocol: &mut Option<ImageProtocol>,
        size: &mut ratatui::layout::Size,
    ) {
        let title = format!("{}x{}", source.width(), source.height());
        outer = outer.title_top(Line::from(title).alignment(Alignment::Right).reversed());

        let inner = outer.inner(area);
        outer.render(area, buf);
        let image_size = ratatui::layout::Size::new(inner.width, inner.height);
        if image_size.width > 0 && image_size.height > 0 && (protocol.is_none() || *size != image_size) {
            match Self::image_protocol(self.image_picker.as_ref(), source.clone(), image_size) {
                Ok(new_protocol) => {
                    *protocol = Some(new_protocol);
                    *size = image_size;
                }
                Err(err) => {
                    warn!("failed to render image preview: {err:?}");
                }
            }
        }
        if let Some(protocol) = protocol {
            let image = ratatui_image::Image::new(protocol).allow_clipping(true);
            image.render(inner, buf);
        }
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
        #[cfg_attr(not(target_os = "linux"), allow(unused_mut))]
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
            image: options.image,
            image_picker: options.image_picker.clone(),
            total_lines: 0,
            loading: false,
            spinner_start: Instant::now(),
        };
        #[cfg(target_os = "linux")]
        if options.preview_window.pty {
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
        let Ok(mut content) = self.content.try_write() else {
            return SkimRender::default();
        };

        let mut block = Block::new().style(self.theme.normal).border_style(self.theme.border);

        // Add borders based on direction and border setting
        if let Some(border_type) = self.border.into_ratatui() {
            block = block.borders(Borders::ALL).border_type(border_type);
        } else {
            // No border on preview itself - separator will be drawn between areas
            match self.direction {
                Direction::Up => block = block.borders(Borders::BOTTOM),
                Direction::Down => block = block.borders(Borders::TOP),
                Direction::Left => block = block.borders(Borders::RIGHT),
                Direction::Right => block = block.borders(Borders::LEFT),
            }
        }

        Clear.render(area, buf);
        let spinner_area = block.inner(area);

        match &mut *content {
            PreviewContent::Text(text) => self.total_lines = self.render_text(block, area, buf, text),
            PreviewContent::Terminal(parser) => self.total_lines = self.render_pty(block, area, buf, parser.as_ref()),
            PreviewContent::Image { source, protocol, size } => {
                self.render_image(block, area, buf, source, protocol, size);
            }
        }

        if self.loading && spinner_area.width > 0 && spinner_area.height > 0 {
            let x = spinner_area.x + spinner_area.width.saturating_sub(1);
            let y = spinner_area.y + spinner_area.height.saturating_sub(1);
            buf.set_string(x, y, spinner_char(self.spinner_start).to_string(), self.theme.spinner);
        }

        SkimRender::default()
    }
}
