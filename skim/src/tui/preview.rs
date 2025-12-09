use ansi_to_tui::IntoText;
use color_eyre::eyre::Result;
use ratatui::{
    style::Stylize,
    text::{Line, Text},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};
use std::process::Command;
use tokio::task::JoinHandle;

use super::Direction;
use super::Event;
use super::backend::Tui;

use crate::theme::ColorTheme;
use crate::tui::widget::{SkimRender, SkimWidget};
use crate::{SkimItem, SkimOptions};
use std::sync::Arc;

// PreviewCallback for ratatui - returns Vec<String> instead of AnsiString
pub type PreviewCallbackFn = dyn Fn(Vec<Arc<dyn SkimItem>>) -> Vec<String> + Send + Sync + 'static;

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

pub struct Preview<'a> {
    pub content: Text<'a>,
    pub cmd: String,
    pub rows: u16,
    pub cols: u16,
    pub scroll_y: u16,
    pub scroll_x: u16,
    pub thread_handle: Option<JoinHandle<()>>,
    pub theme: Arc<ColorTheme>,
    pub border: bool,
    pub direction: Direction,
    pub wrap: bool,
}

impl Default for Preview<'_> {
    fn default() -> Self {
        Self {
            content: Text::default(),
            cmd: String::default(),
            rows: 0,
            cols: 0,
            scroll_y: 0,
            scroll_x: 0,
            thread_handle: None,
            theme: Arc::new(ColorTheme::default()),
            border: false,
            direction: Direction::Right,
            wrap: false,
        }
    }
}

impl Preview<'_> {
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

    pub fn content(&mut self, content: Vec<u8>) -> Result<()> {
        let text = content.to_owned().into_text()?;
        self.content = text;
        // Reset scroll when content changes
        self.scroll_y = 0;
        self.scroll_x = 0;
        Ok(())
    }

    pub fn content_with_position(&mut self, content: Vec<u8>, position: crate::PreviewPosition) -> Result<()> {
        let text = content.to_owned().into_text()?;
        self.content = text;
        // Apply position offsets
        let v_scroll = self.size_to_offset(position.v_scroll, true);
        let v_offset = self.size_to_offset(position.v_offset, true);
        self.scroll_y = v_scroll.saturating_add(v_offset);

        let h_scroll = self.size_to_offset(position.h_scroll, false);
        let h_offset = self.size_to_offset(position.h_offset, false);
        self.scroll_x = h_scroll.saturating_add(h_offset);
        Ok(())
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

    pub fn run(&mut self, tui: &mut Tui, cmd: &str) {
        self.cmd = cmd.to_string();
        let _event_tx = tui.event_tx.clone();
        let mut shell_cmd = Command::new("/bin/sh");
        shell_cmd
            .env("ROWS", self.rows.to_string())
            .env("COLUMS", self.cols.to_string())
            .env("PAGER", "")
            .arg("-c")
            .arg(cmd);
        if let Some(th) = &self.thread_handle {
            th.abort();
        }
        self.thread_handle = Some(tokio::spawn(async move {
            let try_out = shell_cmd.output();
            if try_out.is_err() {
                println!("Shell cmd in error: {:?}", try_out);
                // let _ = _event_tx.send(Event::Error(try_out.unwrap_err().to_string()));
                return;
            };

            let out = try_out.unwrap();

            if out.status.success() {
                _event_tx
                    .send(Event::PreviewReady(out.stdout))
                    .unwrap_or_else(|e| println!("Failed on success: {e}"));
            } else {
                _event_tx
                    .send(Event::PreviewReady(out.stderr))
                    .unwrap_or_else(|e| println!("Failed on error: {e}"));
                // .unwrap_or_else(|e| _event_tx.send(Event::Error(e.to_string())).unwrap());
            }
        }));
    }
}

impl<'a> SkimWidget for Preview<'a> {
    fn from_options(options: &SkimOptions, theme: Arc<ColorTheme>) -> Self {
        Self {
            theme,
            border: options.border,
            direction: options.preview_window.direction,
            ..Default::default()
        }
    }

    fn render(&mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) -> SkimRender {
        if self.rows != area.height || self.cols != area.width {
            self.rows = area.height;
            self.cols = area.width;
        }

        // Calculate total lines in content
        let total_lines = self.content.lines.len();

        // Create paragraph with optional block
        let mut paragraph = Paragraph::new(self.content.clone()).scroll((self.scroll_y, self.scroll_x));

        // Enable wrapping if wrap is true
        if self.wrap {
            paragraph = paragraph.wrap(ratatui::widgets::Wrap { trim: false });
        }
        let mut block = Block::new()
            .style(self.theme.normal())
            .border_style(self.theme.border());

        // Add scroll position indicator at bottom if scrolled
        if self.scroll_y > 0 && total_lines > 0 {
            let current_line = (self.scroll_y + 1) as usize; // +1 because scroll_y is 0-indexed but we want 1-indexed display
            let title = format!("{}/{}", current_line, total_lines);
            use ratatui::layout::Alignment;

            block = block.title_top(Line::from(title).alignment(Alignment::Right).reversed());
        }

        if self.border {
            block = block.borders(Borders::ALL);
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
