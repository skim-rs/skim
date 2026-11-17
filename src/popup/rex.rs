//! Rex (Superlogical) popup backend.
//!
//! Rex exposes `rex run [flags] -- COMMAND...`, which opens a new terminal
//! block; with `--wait` it blocks until the command exits and adopts its exit
//! status. Rex has no tmux-style transient popup on the CLI, so directional
//! popups are rendered as a split of the current block (`--split` + `--ratio`)
//! and a centered popup opens in a new window.
use super::{PopupWindowDir, SkimPopup};
use crate::SkimOptions;
use crate::tui::Size;

use std::fmt::Write as _;
use std::process::{Command, ExitStatus, Stdio};

/// Rex sets `$REX_SESSION` in every terminal it spawns.
pub fn is_available() -> bool {
    cfg!(unix)
        && std::env::var("REX_SESSION").is_ok_and(|s| !s.is_empty())
        && which::which("rex").is_ok()
}

pub(super) struct RexPopup {
    cmd: Command,
    env: String,
    cwd: String,
}

/// Terminal extent along an axis, read from `$COLUMNS`/`$LINES` with a fallback.
fn term_extent(var: &str, fallback: u16) -> u16 {
    std::env::var(var)
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(fallback)
}

/// Convert a popup size to the fraction of the split block given to the popup.
fn size_to_ratio(size: Size, total: u16) -> f32 {
    let ratio = match size {
        Size::Percent(p) => f32::from(p) / 100.0,
        Size::Fixed(cells) => f32::from(cells) / f32::from(total),
        Size::Neg(cells) => f32::from(total.saturating_sub(cells)) / f32::from(total),
    };
    ratio.clamp(0.05, 0.95)
}

impl RexPopup {
    fn build(options: &SkimOptions) -> Self {
        // `is_available` already guarantees rex is on PATH before we reach
        // here in production; fall back to the bare name so arg-building (and
        // tests) work even when the binary cannot be resolved.
        let mut cmd = Command::new(which::which("rex").unwrap_or_else(|_| "rex".into()));
        cmd.arg("run").arg("--wait");

        let arg = options.popup.as_ref().expect("this arg should be present to get here");
        let (raw_dir, size) = arg.split_once(',').unwrap_or((arg, "50%"));
        // Only the size along the split axis matters: `top,40%,80%` keeps 40%.
        let size = size.split_once(',').map_or(size, |(lhs, _)| lhs);
        let size = Size::try_from(size).unwrap_or(Size::Percent(50));

        let split = match PopupWindowDir::from(raw_dir) {
            PopupWindowDir::Top => Some(("above", term_extent("LINES", 24))),
            PopupWindowDir::Bottom => Some(("below", term_extent("LINES", 24))),
            PopupWindowDir::Left => Some(("left", term_extent("COLUMNS", 80))),
            PopupWindowDir::Right => Some(("right", term_extent("COLUMNS", 80))),
            // No floating layer on the CLI: a centered popup gets its own window
            PopupWindowDir::Center => None,
        };
        if let Some((side, total)) = split {
            cmd.arg(format!("--split={side}"))
                .arg(format!("--ratio={:.2}", size_to_ratio(size, total)));
        }

        Self {
            cmd,
            env: String::new(),
            cwd: std::env::current_dir()
                .ok()
                .map_or(".".to_string(), |d| d.to_string_lossy().to_string()),
        }
    }
}

impl RexPopup {
    /// The shell script run in the new block. Its cwd and env are not
    /// guaranteed to be ours, so both are set in the script itself.
    fn script(&self, command: &str) -> String {
        format!(
            "cd {} &&{} {command}",
            String::from_utf8_lossy(&shell_quote::Sh::quote_vec(&self.cwd)),
            self.env
        )
    }
}

impl SkimPopup for RexPopup {
    fn from_options(options: &SkimOptions) -> Box<dyn SkimPopup> {
        Box::new(Self::build(options))
    }

    fn add_env(&mut self, key: &str, value: &str) {
        let _ = write!(
            self.env,
            " {key}={}",
            String::from_utf8_lossy(&shell_quote::Sh::quote_vec(value))
        );
    }

    fn run_and_wait(&mut self, command: &str) -> std::io::Result<ExitStatus> {
        debug!("rex command: {command:?}");
        let script = self.script(command);
        self.cmd.arg("--").args(["sh", "-c", &script]);
        debug!("rex full command: {:?}", self.cmd);

        // `--wait` prints the creation result on stdout before waiting
        self.cmd
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .status()
    }
}

#[cfg(test)]
#[path = "rex_tests.rs"]
mod tests;
