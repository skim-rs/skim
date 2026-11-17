//! Rex popup backend: floating layers for centered popups, splits for edges.
use std::io;
use std::process::{Command, ExitStatus, Stdio};

use serde_json::{Value, json};

use super::{
    PopupLayout, PopupWindowDir, SkimPopup, current_dir_string, multiplexer_command, multiplexer_detected,
    push_env_assignment, sh_quote, term_extent,
};
use crate::SkimOptions;
use crate::tui::Size;

/// Rex sets `$REX_SESSION` in every terminal it spawns.
pub fn is_available() -> bool {
    multiplexer_detected("REX_SESSION", "rex")
}

pub(super) struct RexPopup {
    cmd: Command,
    params: Value,
    session: String,
    env: String,
    cwd: String,
}

fn size_to_ratio(size: Size, total: u16) -> f32 {
    let total = total.max(1);
    let ratio = match size {
        Size::Percent(p) => f32::from(p) / 100.0,
        Size::Fixed(cells) => f32::from(cells) / f32::from(total),
        Size::Neg(cells) => f32::from(total.saturating_sub(cells)) / f32::from(total),
    };
    ratio.clamp(0.01, 1.0)
}

impl RexPopup {
    fn build(options: &SkimOptions) -> Self {
        let session = std::env::var("REX_SESSION").unwrap_or_default();
        let mut cmd = multiplexer_command("rex");
        cmd.args(["api", "call", "--session", &session]);

        let PopupLayout { dir, height, width } = PopupLayout::from_options(options);
        let height = size_to_ratio(Size::try_from(height).unwrap_or(Size::Percent(50)), term_extent("ROWS"));
        let width = size_to_ratio(
            Size::try_from(width).unwrap_or(Size::Percent(50)),
            term_extent("COLUMNS"),
        );
        let mut params = json!({"focus": true});
        if dir == PopupWindowDir::Center {
            cmd.arg("session.new_layer");
            params["bounds"] = json!([(1.0 - width) / 2.0, (1.0 - height) / 2.0, width, height]);
        } else {
            cmd.arg("session.new_split");
            let vertical = matches!(dir, PopupWindowDir::Top | PopupWindowDir::Bottom);
            let before = matches!(dir, PopupWindowDir::Top | PopupWindowDir::Left);
            let ratio = if vertical { height } else { width }.clamp(0.05, 0.95);
            params["direction"] = json!(if vertical { "vertical" } else { "horizontal" });
            params["side"] = json!(if before { "before" } else { "after" });
            // The API ratio is the before part, not necessarily the new pane.
            params["ratio"] = json!(if before { ratio } else { 1.0 - ratio });
            if let Ok(block) = std::env::var("REX_BLOCK")
                && !block.is_empty()
            {
                params["anchor_id"] = json!(block);
            }
        }
        Self {
            cmd,
            params,
            session,
            env: String::new(),
            cwd: current_dir_string(),
        }
    }

    fn script(&self, command: &str) -> String {
        format!("cd {} &&{} {command}", sh_quote(&self.cwd), self.env)
    }

    fn request(&self, command: &str) -> Value {
        let mut params = self.params.clone();
        params["layout"] = json!({"block": {
            "flavor": "com.superlogical.terminal.shell",
            "options": {
                "command": ["sh", "-c", self.script(command)],
                "shell": "none",
                // Keep fast commands available until `rex wait` reads their status.
                "exit": {"on_completion": false}
            }
        }});
        params
    }
}

impl SkimPopup for RexPopup {
    fn from_options(options: &SkimOptions) -> Box<dyn SkimPopup> {
        Box::new(Self::build(options))
    }

    fn add_env(&mut self, key: &str, value: &str) {
        push_env_assignment(&mut self.env, key, value);
    }

    fn run_and_wait(&mut self, command: &str) -> io::Result<ExitStatus> {
        if self.session.is_empty() {
            return Err(io::Error::other("REX_SESSION is empty"));
        }
        let request = self.request(command);
        let output = self.cmd.arg(request.to_string()).stdin(Stdio::null()).output()?;
        if !output.status.success() {
            return Err(io::Error::other(String::from_utf8_lossy(&output.stderr).into_owned()));
        }
        let result: Value = serde_json::from_slice(&output.stdout)?;
        let block = result["block_ids"][0]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| io::Error::other("Rex creation result has no block ID"))?;
        let session = &self.session;
        let status = Command::new(self.cmd.get_program())
            .args(["wait", "--session", session, block])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status();
        let cleanup = Command::new(self.cmd.get_program())
            .args(["api", "call", "--session", session, "block.close"])
            .arg(json!({"block_id": block}).to_string())
            .stdin(Stdio::null())
            .output()?;
        // A user can close the pane before or during the wait.
        if !cleanup.status.success() && !String::from_utf8_lossy(&cleanup.stderr).contains("code=not_found") {
            return Err(io::Error::other(String::from_utf8_lossy(&cleanup.stderr).into_owned()));
        }
        status
    }
}

#[cfg(test)]
#[path = "rex_tests.rs"]
mod tests;
