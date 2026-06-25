use crate::SkimOptions;

use super::{PopupWindowDir, SkimPopup};
use std::process::{Command, ExitStatus, Stdio};

pub fn is_available() -> bool {
    cfg!(unix) && std::env::var("TMUX").is_ok() && which::which("tmux").is_ok()
}

pub(super) struct TmuxPopup {
    cmd: Command,
}

impl TmuxPopup {
    fn build(options: &SkimOptions) -> Self {
        let arg = options.popup.as_ref().expect("this arg should be present to get here");
        // `is_available` already guarantees tmux is on PATH before we reach here
        // in production; fall back to the bare name so arg-building (and tests)
        // work even when the binary cannot be resolved.
        let mut cmd = Command::new(which::which("tmux").unwrap_or_else(|_| "tmux".into()));
        cmd.arg("display-popup").arg("-E").args([
            "-d",
            &std::env::current_dir()
                .ok()
                .map_or(".".to_string(), |d| d.to_string_lossy().to_string()),
        ]);

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

        let (raw_dir, size) = arg.split_once(',').unwrap_or((arg, "50%"));
        let dir = PopupWindowDir::from(raw_dir);
        let (height, width) = if let Some((lhs, rhs)) = size.split_once(',') {
            match dir {
                PopupWindowDir::Center | PopupWindowDir::Left | PopupWindowDir::Right => (rhs, lhs),
                PopupWindowDir::Top | PopupWindowDir::Bottom => (lhs, rhs),
            }
        } else {
            match dir {
                PopupWindowDir::Left | PopupWindowDir::Right => ("100%", size),
                PopupWindowDir::Top | PopupWindowDir::Bottom => (size, "100%"),
                PopupWindowDir::Center => (size, size),
            }
        };

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
