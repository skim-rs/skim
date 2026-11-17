use super::{
    PopupLayout, PopupWindowDir, SkimPopup, current_dir_string, multiplexer_command, multiplexer_detected, popup_name,
    push_env_assignment, term_extent,
};
use crate::SkimOptions;
use crate::tui::Size;

use std::process::{Command, ExitStatus, Stdio};

pub fn is_available() -> bool {
    multiplexer_detected("ZELLIJ", "zellij")
}

pub(super) struct ZellijPopup {
    cmd: Command,
    env: String,
}

fn middle_coord(size: Size, var: &str) -> Size {
    match size {
        Size::Percent(p) => Size::Percent(100u16.saturating_sub(p) / 2),
        Size::Fixed(cells) => Size::Fixed(term_extent(var).saturating_sub(cells) / 2),
        Size::Neg(cells) => Size::Fixed(cells / 2),
    }
}

fn align_end_coord(size: Size, var: &str) -> Size {
    match size {
        Size::Percent(p) => Size::Percent(100 - p),
        Size::Fixed(cols) => Size::Fixed(term_extent(var).saturating_sub(cols)),
        Size::Neg(cells) => Size::Fixed(cells),
    }
}

impl ZellijPopup {
    fn build(options: &SkimOptions) -> Self {
        let mut cmd = multiplexer_command("zellij");
        cmd.arg("run")
            .arg("--floating")
            .arg("--block-until-exit")
            .arg("--close-on-exit")
            .args(["--pinned", "true"])
            .args(["--name", popup_name(options)])
            .args(["--cwd", &current_dir_string()]);

        if options.border == crate::tui::BorderType::ForceOff {
            cmd.args(["--borderless", "true"]);
        }

        let PopupLayout { dir, height, width } = PopupLayout::from_options(options);
        let height = Size::try_from(height).unwrap_or(Size::Percent(50));
        let width = Size::try_from(width).unwrap_or(Size::Percent(50));

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
        push_env_assignment(&mut self.env, key, value);
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
