use super::{PopupWindowDir, SkimPopup};
use crate::SkimOptions;
use crate::tui::Size;

use std::fmt::Write as _;
use std::process::{Command, ExitStatus, Stdio};

pub fn is_available() -> bool {
    std::env::var("ZELLIJ").is_ok() && which::which("zellij").is_ok()
}

pub(super) struct ZellijPopup {
    cmd: Command,
    env: String,
}

fn middle_coord(size: Size, var: &str) -> Size {
    match size {
        Size::Percent(p) => Size::Percent(100u16.saturating_sub(p) / 2),
        Size::Fixed(cells) => Size::Fixed(
            std::env::var(var)
                .map_or(80u16, |s| s.parse().unwrap_or(80))
                .saturating_sub(cells)
                / 2,
        ),
        Size::Neg(cells) => Size::Fixed(cells / 2),
    }
}

fn align_end_coord(size: Size, var: &str) -> Size {
    match size {
        Size::Percent(p) => Size::Percent(100 - p),
        Size::Fixed(cols) => Size::Fixed(
            std::env::var(var)
                .map_or(80u16, |s| s.parse().unwrap_or(80))
                .saturating_sub(cols),
        ),
        Size::Neg(cells) => Size::Fixed(cells),
    }
}

impl ZellijPopup {
    fn build(options: &SkimOptions) -> Self {
        // `is_available` already guarantees zellij is on PATH before we reach
        // here in production; fall back to the bare name so arg-building (and
        // tests) work even when the binary cannot be resolved.
        let mut cmd = Command::new(which::which("zellij").unwrap_or_else(|_| "zellij".into()));
        cmd.arg("run")
            .arg("--floating")
            .arg("--block-until-exit")
            .arg("--close-on-exit")
            .args(["--pinned", "true"])
            .args(["--name", "skim"])
            .args([
                "--cwd",
                &std::env::current_dir()
                    .ok()
                    .map_or(".".to_string(), |d| d.to_string_lossy().to_string()),
            ]);

        if options.border == crate::tui::BorderType::ForceOff {
            cmd.args(["--borderless", "true"]);
        }

        let arg = options.popup.as_ref().expect("this arg should be present to get here");

        let (raw_dir, size) = arg.split_once(',').unwrap_or((arg, "50%"));
        let dir = PopupWindowDir::from(raw_dir);
        let (height, width) = if let Some((lhs, rhs)) = size.split_once(',') {
            let parsed_rhs = Size::try_from(rhs).unwrap_or(Size::Percent(50));
            let parsed_lhs = Size::try_from(lhs).unwrap_or(Size::Percent(50));

            match dir {
                PopupWindowDir::Center | PopupWindowDir::Left | PopupWindowDir::Right => (parsed_rhs, parsed_lhs),
                PopupWindowDir::Top | PopupWindowDir::Bottom => (parsed_lhs, parsed_rhs),
            }
        } else {
            let parsed_size = Size::try_from(size).unwrap_or(Size::Percent(50));
            let full_size = Size::Percent(100);
            match dir {
                PopupWindowDir::Left | PopupWindowDir::Right => (full_size, parsed_size),
                PopupWindowDir::Top | PopupWindowDir::Bottom => (parsed_size, full_size),
                PopupWindowDir::Center => (parsed_size, parsed_size),
            }
        };

        let (x, y) = match dir {
            PopupWindowDir::Center => {
                let x = middle_coord(width, "COLUMNS");
                let y = middle_coord(height, "ROWS");
                (x, y)
            }
            PopupWindowDir::Top => (middle_coord(width, "COLUMNS"), Size::Fixed(0)),
            PopupWindowDir::Bottom => (middle_coord(width, "COLUMNS"), align_end_coord(height, "ROWS")),
            PopupWindowDir::Left => (Size::Fixed(0), middle_coord(height, "ROWS")),
            PopupWindowDir::Right => (align_end_coord(width, "COLUMNS"), middle_coord(height, "ROWS")),
        };

        cmd.args(["--height", &height.to_string()])
            .args(["--width", &width.to_string()])
            .args(["-x", &x.to_string()])
            .args(["-y", &y.to_string()]);

        Self {
            cmd,
            env: String::new(),
        }
    }
}

impl SkimPopup for ZellijPopup {
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
        debug!("zellij command: {command:?}");
        self.cmd
            .arg("--")
            .args(["sh", "-c", format!("{} {command}", self.env).trim()]);
        debug!("zellij full command: {:?}", self.cmd);

        self.cmd
            // .stdout(Stdio::null())
            // .stderr(Stdio::null())
            .stdin(Stdio::null())
            .status()
    }
}

#[cfg(test)]
#[path = "zellij_tests.rs"]
mod tests;
