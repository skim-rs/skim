use ansi_to_tui::IntoText;
use color_eyre::eyre::Result;
use portable_pty::{PtyPair, PtySize, native_pty_system};
use ratatui::{
    prelude::Backend,
    style::Stylize,
    text::{Line, Text},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};
use std::process::Command;
use tokio::task::JoinHandle;

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
    pub theme: Arc<ColorTheme>,
    /// Border type, if borders are enabled
    pub border: Option<BorderType>,
    pub direction: Direction,
    pub wrap: bool,
    pty: Option<PtyPair>,
    pty_killer: Option<Box<dyn portable_pty::ChildKiller + Send + Sync>>,
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
            theme: Arc::new(ColorTheme::default()),
            border: None,
            direction: Direction::Right,
            wrap: false,
            pty: None,
            pty_killer: None,
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
        let pair = pty_system
            .openpty(PtySize {
                rows: 0,
                cols: 0,
                pixel_width: 0,
                pixel_height: 0,
            })
            .ok();
        self.pty = pair;
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
    pub fn kill(&mut self) {
        if let Some(th) = self.thread_handle.take() {
            th.abort();
        }
        if let Some(mut killer) = self.pty_killer.take() {
            let _ = killer.kill();
        }
    }

    pub fn run<B: Backend>(&mut self, tui: &mut Tui<B>, cmd: &str)
    where
        B::Error: Send + Sync + 'static,
    {
        self.kill();
        self.cmd = cmd.to_string();
        let event_tx_clone = tui.event_tx.clone();
        let content = self.content.clone();

        if let Some(ref pty) = self.pty {
            let mut shell_cmd = portable_pty::CommandBuilder::new("/bin/sh");
            shell_cmd.env("ROWS", self.rows.to_string());
            shell_cmd.env("COLUMNS", self.cols.to_string());
            shell_cmd.env("PAGER", "");
            shell_cmd.arg("-c");
            if let Ok(cwd) = nix::unistd::getcwd() {
                shell_cmd.cwd(cwd);
            }
            shell_cmd.arg(cmd);
            let Ok(child) = pty.slave.spawn_command(shell_cmd) else {
                warn!("Failed to spawn shell command");
                return;
            };
            let Ok(mut reader) = pty.master.try_clone_reader() else {
                warn!("Failed to acquire pty reader");
                return;
            };
            self.pty_killer = Some(child.clone_killer());
            self.thread_handle = Some(tokio::spawn(async move {
                let mut res = Vec::with_capacity(PREVIEW_MAX_BYTES);
                let mut n = 0;
                while n < PREVIEW_MAX_BYTES {
                    let mut buf = [0; 1024];
                    let Ok(k) = reader.read(&mut buf) else {
                        break;
                    };
                    res.extend_from_slice(&buf[..k]);
                    if k < 1024 {
                        break;
                    }
                    n += k;
                }

                let Ok(mut c) = content.write() else {
                    return;
                };
                *c = res.into_text().unwrap_or_default();
                event_tx_clone
                    .send(Event::PreviewReady)
                    .unwrap_or_else(|e| warn!("Failed on pty: {e}"));
            }));
        } else {
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
            self.thread_handle = Some(tokio::spawn(async move {
                let try_out = shell_cmd.output();
                if try_out.is_err() {
                    println!("Shell cmd in error: {:?}", try_out);
                    // let _ = _event_tx.send(Event::Error(try_out.unwrap_err().to_string()));
                    return;
                };

                let out = try_out.unwrap();

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
                    event_tx_clone
                        .send(Event::PreviewReady)
                        .unwrap_or_else(|e| println!("Failed on success: {e}"));
                } else {
                    *c = out.stderr.to_owned().into_text().unwrap_or_default();
                    event_tx_clone
                        .send(Event::PreviewReady)
                        .unwrap_or_else(|e| println!("Failed on error: {e}"));
                    // .unwrap_or_else(|e| _event_tx.send(Event::Error(e.to_string())).unwrap());
                }
            }));
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
            pty: None,
            pty_killer: None,
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
