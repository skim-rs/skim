use ansi_to_tui::IntoText;
use color_eyre::eyre::{bail, Result};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use ratatui::{
    text::Text,
    widgets::{Paragraph, Widget},
};

use super::tui::Tui;
use super::app::App;
use super::Event;

#[derive(Default)]
pub struct Preview {
    pub content: Vec<u8>,
    pub size: PtySize,
    started: bool,
}

impl Preview {
    pub fn start(&mut self) {
        self.started = true;
    }

    pub fn append(&mut self, mut other: Vec<u8>) {
        if self.started {
            self.started = false;
            self.content = Vec::new()
        }
        self.content.append(&mut other);
    }
}

impl Widget for &mut Preview {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        if self.size.rows != area.height || self.size.cols != area.width {
            self.size = PtySize {
                rows: area.height,
                cols: area.width,
                pixel_width: 0,
                pixel_height: 0,
            }
        }
        let text = self
            .content
            .clone()
            .into_text()
            .unwrap_or(Text::raw("error unwrapping output as ANSI text"));
        Paragraph::new(text).render(area, buf);
    }
}

pub fn run_preview(app: &mut App, tui: &mut Tui) -> Result<()> {
    let join_handle = &app.preview_handle;
    join_handle.abort();
    let cmd = format!("bat {}", app.input.to_string());
    let _event_tx = tui.event_tx.clone();
    let mut cmd_builder = CommandBuilder::new("/bin/sh");
    cmd_builder.arg("-c");
    cmd_builder.arg(cmd);
    cmd_builder.cwd(std::env::current_dir()?);
    cmd_builder.env("PAGER", "");
    let pty_pair = native_pty_system().openpty(app.preview.size).unwrap();
    if let Err(e) = pty_pair.slave.spawn_command(cmd_builder) {
        bail!(e);
    };
    drop(pty_pair.slave);
    let master = pty_pair.master;
    {
        // Drop writer on purpose
        let _ = master.take_writer().unwrap();
    }
    app.preview_handle = tokio::spawn(async move {
        let mut buf: Vec<u8> = vec![];
        let mut reader = master
            .try_clone_reader()
            .expect("Pty reader should be available for cloning");
        if let Err(e) = reader.read_to_end(&mut buf) {
            let _ = _event_tx.send(Event::Error(e.to_string()));
        };

        let _ = _event_tx
            .send(Event::PreviewReady(buf))
            .unwrap_or_else(|e| _event_tx.send(Event::Error(e.to_string())).unwrap());
        drop(reader);
    });
    Ok(())
}
