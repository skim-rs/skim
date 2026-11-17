//! Rex (Superlogical) popup backend.
//!
//! Rex exposes `rex run [flags] -- COMMAND...`, which opens a new terminal
//! block; with `--wait` it blocks until the command exits and adopts its exit
//! status. Rex has no tmux-style transient popup on the CLI, so directional
//! popups are rendered as a split of the current block (`--split` + `--ratio`)
//! and a centered popup opens in a new window.
use super::{
    PopupLayout, PopupWindowDir, SkimPopup, current_dir_string, multiplexer_command, multiplexer_detected,
    push_env_assignment, sh_quote, term_extent,
};
use crate::SkimOptions;
use crate::tui::Size;

use std::process::{Command, ExitStatus, Stdio};

/// Rex sets `$REX_SESSION` in every terminal it spawns.
pub fn is_available() -> bool {
    cfg!(unix) && multiplexer_detected("REX_SESSION", "rex")
}

pub(super) struct RexPopup {
    cmd: Command,
    env: String,
    cwd: String,
}

/// Convert a popup size to the fraction of the split block given to the popup.
fn size_to_ratio(size: Size, total: u16) -> f32 {
    let total = total.max(1);
    let ratio = match size {
        Size::Percent(p) => f32::from(p) / 100.0,
        Size::Fixed(cells) => f32::from(cells) / f32::from(total),
        Size::Neg(cells) => f32::from(total.saturating_sub(cells)) / f32::from(total),
    };
    ratio.clamp(0.05, 0.95)
}

impl RexPopup {
    fn build(options: &SkimOptions) -> Self {
        let mut cmd = multiplexer_command("rex");
        cmd.arg("run").arg("--wait");

        let PopupLayout { dir, height, width } = PopupLayout::from_options(options);
        // Only the size along the split axis matters.
        let split = match dir {
            PopupWindowDir::Top => Some(("above", height, "ROWS")),
            PopupWindowDir::Bottom => Some(("below", height, "ROWS")),
            PopupWindowDir::Left => Some(("left", width, "COLUMNS")),
            PopupWindowDir::Right => Some(("right", width, "COLUMNS")),
            // No floating layer on the CLI: a centered popup gets its own window
            PopupWindowDir::Center => None,
        };
        if let Some((side, size, extent_var)) = split {
            let size = Size::try_from(size).unwrap_or(Size::Percent(50));
            cmd.arg(format!("--split={side}"))
                .arg(format!("--ratio={:.2}", size_to_ratio(size, term_extent(extent_var))));
        }

        Self {
            cmd,
            env: String::new(),
            cwd: current_dir_string(),
        }
    }

    /// The shell script run in the new block. Its cwd and env are not
    /// guaranteed to be ours, so both are set in the script itself.
    fn script(&self, command: &str) -> String {
        format!("cd {} &&{} {command}", sh_quote(&self.cwd), self.env)
    }
}

impl SkimPopup for RexPopup {
    fn from_options(options: &SkimOptions) -> Box<dyn SkimPopup> {
        Box::new(Self::build(options))
    }

    fn add_env(&mut self, key: &str, value: &str) {
        push_env_assignment(&mut self.env, key, value);
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
