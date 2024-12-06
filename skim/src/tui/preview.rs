use ansi_to_tui::IntoText;
use color_eyre::eyre::{bail, Result};
use ratatui::{
    text::Text,
    widgets::{Block, Clear, Paragraph, Widget},
};
use std::{
    process::{Command, ExitStatus},
    thread::sleep,
    time::Duration,
};
use tokio::task::JoinHandle;

use super::tui::Tui;
use super::Event;

pub struct Preview<'a> {
    pub content: Text<'a>,
    pub cmd: String,
    pub rows: u16,
    pub cols: u16,
    pub thread_handle: JoinHandle<()>,
}

impl Default for Preview<'_> {
    fn default() -> Self {
        Self {
            content: Text::default(),
            cmd: String::default(),
            rows: 0,
            cols: 0,
            thread_handle: tokio::spawn(async { }),
        }
    }
}

impl Preview<'_> {
    pub fn content(&mut self, content: &Vec<u8>) -> Result<()> {
        let text = content.clone().into_text()?;
        self.content = text;
        Ok(())
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
        self.thread_handle.abort();
        self.thread_handle = tokio::spawn(async move {
            let try_out = shell_cmd.output();
            if try_out.is_err() {
                let _ = _event_tx.send(Event::Error(try_out.unwrap_err().to_string()));
                return;
            };

            let out = try_out.unwrap();

            if out.status.success() {
              let _ = _event_tx
                  .send(Event::PreviewReady(out.stdout))
                  .unwrap_or_else(|e| _event_tx.send(Event::Error(e.to_string())).unwrap());
            } else {
              let _ = _event_tx
                  .send(Event::PreviewReady(out.stderr))
                  .unwrap_or_else(|e| _event_tx.send(Event::Error(e.to_string())).unwrap());
            }
        });
    }
}

impl Widget for &mut Preview<'_> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        if self.rows != area.height || self.cols != area.width {
            self.rows = area.height;
            self.cols = area.width;
        }
        let block = Block::bordered();
        Clear.render(area, buf);
        Paragraph::new(self.content.clone()).block(block).render(area, buf);
    }
}
