use crate::{SkimOptions, tui::Size};
use std::fmt::Write as _;

use super::{PopupWindowDir, SkimPopup};
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
        Size::Percent(p) => Size::Percent(50 - p / 2),
        Size::Fixed(cols) => {
            Size::Fixed((std::env::var(var).map(|s| s.parse().unwrap_or(80)).unwrap_or(80) - cols) / 2)
        }
    }
}

fn align_end_coord(size: Size, var: &str) -> Size {
    match size {
        Size::Percent(p) => Size::Percent(100 - p),
        Size::Fixed(cols) => Size::Fixed(std::env::var(var).map(|s| s.parse().unwrap_or(80)).unwrap_or(80) - cols),
    }
}

impl ZellijPopup {
    fn build(options: &SkimOptions) -> Self {
        let mut cmd = Command::new(
            which::which("zellij").expect("zellij not found in path. This should have been caught by is_available"),
        );
        cmd.arg("run")
            .arg("--floating")
            .arg("--block-until-exit")
            .arg("--close-on-exit")
            .args(["--pinned", "true"])
            .args(["--name", "skim"])
            .args(["--cwd", std::env::current_dir().unwrap().to_str().unwrap()]);

        if options.border.is_none() {
            cmd.arg("--borderless");
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
            PopupWindowDir::Bottom => (align_end_coord(height, "ROWS"), middle_coord(width, "COLUMNS")),
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
        let _ = write!(self.env, " {key}={value}");
    }

    fn run_and_wait(&mut self, command: &str) -> std::io::Result<ExitStatus> {
        debug!("zellij command: {command:?}");
        self.cmd.arg("--").args(["sh", "-c", command]);
        debug!("zellij full command: {:?}", self.cmd);

        self.cmd
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .status()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::SkimOptionsBuilder;

    /// Skip the test if `zellij` is not in PATH (CI environments without zellij).
    macro_rules! require_zellij {
        () => {
            if which::which("zellij").is_err() {
                return;
            }
        };
    }

    fn opts(tmux: &str) -> crate::SkimOptions {
        SkimOptionsBuilder::default()
            .popup(tmux)
            .build()
            .expect("valid options")
    }

    fn args(popup: &ZellijPopup) -> Vec<String> {
        popup.cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    fn get_flag<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
        args.windows(2).find(|w| w[0] == flag).map(|w| w[1].as_str())
    }

    // ── middle_coord ────────────────────────────────────────────────────────

    #[test]
    fn middle_coord_percent() {
        // 50% wide in a 100% viewport → offset should be 25%
        assert_eq!(middle_coord(Size::Percent(50), "COLUMNS"), Size::Percent(25));
    }

    #[test]
    fn middle_coord_fixed_uses_env_var() {
        // Override COLUMNS to a known value so the test is deterministic
        // SAFETY: single-threaded test binary section; no concurrent env reads.
        unsafe { std::env::set_var("COLUMNS", "80") };
        // 20 cols wide → offset = (80 - 20) / 2 = 30
        assert_eq!(middle_coord(Size::Fixed(20), "COLUMNS"), Size::Fixed(30));
    }

    #[test]
    fn middle_coord_fixed_fallback() {
        // SAFETY: single-threaded test binary section; no concurrent env reads.
        unsafe { std::env::remove_var("COLUMNS") };
        // fallback width = 80; (80 - 20) / 2 = 30
        assert_eq!(middle_coord(Size::Fixed(20), "COLUMNS"), Size::Fixed(30));
    }

    // ── align_end_coord ──────────────────────────────────────────────────────

    #[test]
    fn align_end_coord_percent() {
        // 30% → end offset = 70%
        assert_eq!(align_end_coord(Size::Percent(30), "COLUMNS"), Size::Percent(70));
    }

    #[test]
    fn align_end_coord_fixed_uses_env_var() {
        // SAFETY: single-threaded test binary section; no concurrent env reads.
        unsafe { std::env::set_var("COLUMNS", "80") };
        // 20 cols wide → end offset = 80 - 20 = 60
        assert_eq!(align_end_coord(Size::Fixed(20), "COLUMNS"), Size::Fixed(60));
    }

    // ── from_options / build ─────────────────────────────────────────────────

    #[test]
    fn center_default_size() {
        require_zellij!();
        let popup = ZellijPopup::build(&opts("center"));
        let a = args(&popup);
        assert_eq!(get_flag(&a, "--height"), Some("50%"));
        assert_eq!(get_flag(&a, "--width"), Some("50%"));
    }

    #[test]
    fn top_direction() {
        require_zellij!();
        let popup = ZellijPopup::build(&opts("top,40%"));
        let a = args(&popup);
        assert_eq!(get_flag(&a, "--height"), Some("40%"));
        assert_eq!(get_flag(&a, "--width"), Some("100%"));
        assert_eq!(get_flag(&a, "-y"), Some("0"));
    }

    #[test]
    fn left_direction() {
        require_zellij!();
        let popup = ZellijPopup::build(&opts("left,30%"));
        let a = args(&popup);
        assert_eq!(get_flag(&a, "--width"), Some("30%"));
        assert_eq!(get_flag(&a, "-x"), Some("0"));
    }

    #[test]
    fn right_direction() {
        require_zellij!();
        // SAFETY: single-threaded test binary section; no concurrent env reads.
        unsafe { std::env::set_var("COLUMNS", "80") };
        let popup = ZellijPopup::build(&opts("right,25%"));
        let a = args(&popup);
        // width = 25%, x = align_end_coord(25%, "COLUMNS") = 75%
        assert_eq!(get_flag(&a, "--width"), Some("25%"));
        assert_eq!(get_flag(&a, "-x"), Some("75%"));
    }

    #[test]
    fn borderless_when_no_border_option() {
        require_zellij!();
        let popup = ZellijPopup::build(&opts("center"));
        let a = args(&popup);
        assert!(a.contains(&"--borderless".to_string()));
    }

    #[test]
    fn no_borderless_when_border_set() {
        require_zellij!();
        let opts = SkimOptionsBuilder::default()
            .popup("center")
            .border(crate::tui::BorderType::Plain)
            .build()
            .expect("valid options");
        let popup = ZellijPopup::build(&opts);
        let a = args(&popup);
        assert!(!a.contains(&"--borderless".to_string()));
    }

    #[test]
    fn add_env_appends_to_env_string() {
        require_zellij!();
        let mut popup = ZellijPopup::build(&opts("center"));
        popup.add_env("FOO", "bar");
        popup.add_env("BAZ", "qux");
        assert_eq!(popup.env, " FOO=bar BAZ=qux");
    }
}
