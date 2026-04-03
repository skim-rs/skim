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
        let mut cmd = Command::new(
            which::which("tmux").expect("tmux not found in path. This should have been caught by is_available"),
        );
        cmd.arg("display-popup").arg("-E").args([
            "-d",
            &std::env::current_dir()
                .ok()
                .map_or(".".to_string(), |d| d.to_string_lossy().to_string()),
        ]);

        let border = {
            use crate::tui::BorderType::{Plain, Rounded, Thick};
            match options.border {
                None => "none",
                Some(Plain) => "single",
                Some(Rounded) => "rounded",
                Some(Thick) => "heavy",
                Some(_) => "double",
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
mod tests {
    use super::*;
    use crate::options::SkimOptionsBuilder;

    /// Skip the test if `tmux` is not in PATH (CI environments without tmux).
    macro_rules! require_tmux {
        () => {
            if which::which("tmux").is_err() {
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

    fn opts_with_border(tmux: &str, border: crate::tui::BorderType) -> crate::SkimOptions {
        SkimOptionsBuilder::default()
            .popup(tmux)
            .border(border)
            .build()
            .expect("valid options")
    }

    #[test]
    fn border_none_does_not_panic() {
        require_tmux!();
        // Ensure each BorderType variant can be passed without panicking.
        for border in [
            crate::tui::BorderType::Plain,
            crate::tui::BorderType::Rounded,
            crate::tui::BorderType::Thick,
            crate::tui::BorderType::Double,
        ] {
            let _ = TmuxPopup::build(&opts_with_border("center", border));
        }
        // No border option
        let _ = TmuxPopup::build(&opts("center"));
    }

    fn args(popup: &TmuxPopup) -> Vec<String> {
        popup.cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    fn get_flag<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
        args.windows(2).find(|w| w[0] == flag).map(|w| w[1].as_str())
    }

    #[test]
    fn center_default_size() {
        require_tmux!();
        let popup = TmuxPopup::build(&opts("center"));
        let a = args(&popup);
        assert_eq!(get_flag(&a, "-h"), Some("50%"));
        assert_eq!(get_flag(&a, "-w"), Some("50%"));
        assert_eq!(get_flag(&a, "-x"), Some("C"));
        assert_eq!(get_flag(&a, "-y"), Some("C"));
    }

    #[test]
    fn center_no_direction_defaults_to_center() {
        require_tmux!();
        // Bare "50%" with no direction keyword defaults to Center
        let popup = TmuxPopup::build(&opts("50%"));
        let a = args(&popup);
        assert_eq!(get_flag(&a, "-x"), Some("C"));
        assert_eq!(get_flag(&a, "-y"), Some("C"));
    }

    #[test]
    fn top_direction() {
        require_tmux!();
        let popup = TmuxPopup::build(&opts("top,40%"));
        let a = args(&popup);
        assert_eq!(get_flag(&a, "-h"), Some("40%"));
        assert_eq!(get_flag(&a, "-w"), Some("100%"));
        assert_eq!(get_flag(&a, "-x"), Some("C"));
        assert_eq!(get_flag(&a, "-y"), Some("0%"));
    }

    #[test]
    fn bottom_direction() {
        require_tmux!();
        let popup = TmuxPopup::build(&opts("bottom,30%"));
        let a = args(&popup);
        assert_eq!(get_flag(&a, "-h"), Some("30%"));
        assert_eq!(get_flag(&a, "-w"), Some("100%"));
        assert_eq!(get_flag(&a, "-x"), Some("C"));
        assert_eq!(get_flag(&a, "-y"), Some("100%"));
    }

    #[test]
    fn left_direction() {
        require_tmux!();
        let popup = TmuxPopup::build(&opts("left,30%"));
        let a = args(&popup);
        assert_eq!(get_flag(&a, "-h"), Some("100%"));
        assert_eq!(get_flag(&a, "-w"), Some("30%"));
        assert_eq!(get_flag(&a, "-x"), Some("0%"));
        assert_eq!(get_flag(&a, "-y"), Some("C"));
    }

    #[test]
    fn right_direction() {
        require_tmux!();
        let popup = TmuxPopup::build(&opts("right,30%"));
        let a = args(&popup);
        assert_eq!(get_flag(&a, "-h"), Some("100%"));
        assert_eq!(get_flag(&a, "-w"), Some("30%"));
        assert_eq!(get_flag(&a, "-x"), Some("100%"));
        assert_eq!(get_flag(&a, "-y"), Some("C"));
    }

    #[test]
    fn two_dimensional_size_center() {
        // "center,WIDTH,HEIGHT" — for Center/Left/Right: height=rhs, width=lhs
        require_tmux!();
        let popup = TmuxPopup::build(&opts("center,60%,40%"));
        let a = args(&popup);
        assert_eq!(get_flag(&a, "-w"), Some("60%"));
        assert_eq!(get_flag(&a, "-h"), Some("40%"));
    }

    #[test]
    fn two_dimensional_size_top() {
        // "top,HEIGHT,WIDTH" — for Top/Bottom: height=lhs, width=rhs
        require_tmux!();
        let popup = TmuxPopup::build(&opts("top,30%,80%"));
        let a = args(&popup);
        assert_eq!(get_flag(&a, "-h"), Some("30%"));
        assert_eq!(get_flag(&a, "-w"), Some("80%"));
    }

    #[test]
    fn add_env_appends_e_flag() {
        require_tmux!();
        let mut popup = TmuxPopup::build(&opts("center"));
        popup.add_env("FOO", "bar");
        let a = args(&popup);
        assert!(a.windows(2).any(|w| w[0] == "-e" && w[1] == "FOO=bar"));
    }
}
