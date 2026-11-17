//! Helpers shared by the popup backends' unit tests.
use std::process::Command;

use crate::SkimOptions;
use crate::options::SkimOptionsBuilder;

/// Options with `--popup <popup>`.
pub(super) fn opts(popup: &str) -> SkimOptions {
    SkimOptionsBuilder::default()
        .popup(popup)
        .build()
        .expect("valid options")
}

/// Options with `--popup <popup>` and the given border.
pub(super) fn opts_with_border(popup: &str, border: crate::tui::BorderType) -> SkimOptions {
    SkimOptionsBuilder::default()
        .popup(popup)
        .border(border)
        .build()
        .expect("valid options")
}

/// Options with `--popup <popup> --popup-name <name>`.
pub(super) fn opts_with_name(popup: &str, name: &str) -> SkimOptions {
    let mut opts = opts(popup);
    opts.popup_name = Some(name.to_string());
    opts
}

/// The arguments a backend's command was built with.
pub(super) fn args(cmd: &Command) -> Vec<String> {
    cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
}

/// The value following `flag` in `args`.
pub(super) fn get_flag<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2).find(|w| w[0] == flag).map(|w| w[1].as_str())
}
