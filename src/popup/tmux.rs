use crate::SkimOptions;

use super::{PopupLayout, PopupWindowDir, SkimPopup, current_dir_string, multiplexer_command, multiplexer_detected};
use std::process::{Command, ExitStatus, Stdio};

pub fn is_available() -> bool {
    cfg!(unix) && multiplexer_detected("TMUX", "tmux")
}

/// Escape a user-provided popup title so tmux treats it as literal text.
///
/// `-T` is a format string, so `#` must be doubled, and tmux ends a command at
/// an argument ending in `;`, which must be backslash-escaped.
fn escape_tmux_format(label: &str) -> String {
    let escaped = label.replace('#', "##");
    match escaped.strip_suffix(';') {
        Some(without) => format!("{without}\\;"),
        None => escaped,
    }
}

pub(super) struct TmuxPopup {
    cmd: Command,
}

impl TmuxPopup {
    fn build(options: &SkimOptions) -> Self {
        let mut cmd = multiplexer_command("tmux");
        cmd.arg("display-popup").arg("-E").args(["-d", &current_dir_string()]);

        // `-T` is a tmux format (tmux 3.4+), so escape it and only set it when
        // the user asked for a label, keeping older tmux and untitled popups
        // working as before.
        if let Some(label) = options.popup_name.as_deref() {
            cmd.args(["-T", &escape_tmux_format(label)]);
        }

        let border = {
            use crate::tui::BorderType::{ForceOff, None, Plain, Rounded, Thick};
            match options.border {
                ForceOff => "none",
                None | Plain => "single",
                Rounded => "rounded",
                Thick => "heavy",
                _ => "double",
            }
        };

        let PopupLayout { dir, height, width } = PopupLayout::from_options(options);

        let (x, y) = match dir {
            PopupWindowDir::Center => ("C", "C"),
            PopupWindowDir::Top => ("C", "0%"),
            PopupWindowDir::Bottom => ("C", "100%"),
            PopupWindowDir::Left => ("0%", "C"),
            PopupWindowDir::Right => ("100%", "C"),
        };

        cmd.args(["-h", height])
            .args(["-w", width])
            .args(["-x", x])
            .args(["-y", y])
            .args(["-b", border]);

        Self { cmd }
    }
}

impl SkimPopup for TmuxPopup {
    fn from_options(options: &SkimOptions) -> Box<dyn SkimPopup> {
        Box::new(Self::build(options)) as Box<dyn SkimPopup>
    }

    fn add_env(&mut self, key: &str, value: &str) {
        self.cmd.args(["-e", &format!("{key}={value}")]);
    }

    fn run_and_wait(&mut self, command: &str) -> std::io::Result<ExitStatus> {
        debug!("tmux command: {command:?}");
        self.cmd.args(["sh", "-c", command]);

        debug!("tmux full command: {:?}", self.cmd);
        self.cmd
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .status()
    }
}

#[cfg(test)]
#[path = "tmux_tests.rs"]
mod tests;
