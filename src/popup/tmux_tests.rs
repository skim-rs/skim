use super::*;
use crate::options::SkimOptionsBuilder;

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
    let popup = TmuxPopup::build(&opts("center"));
    let a = args(&popup);
    assert_eq!(get_flag(&a, "-h"), Some("50%"));
    assert_eq!(get_flag(&a, "-w"), Some("50%"));
    assert_eq!(get_flag(&a, "-x"), Some("C"));
    assert_eq!(get_flag(&a, "-y"), Some("C"));
}

#[test]
fn center_no_direction_defaults_to_center() {
    // Bare "50%" with no direction keyword defaults to Center
    let popup = TmuxPopup::build(&opts("50%"));
    let a = args(&popup);
    assert_eq!(get_flag(&a, "-x"), Some("C"));
    assert_eq!(get_flag(&a, "-y"), Some("C"));
}

#[test]
fn top_direction() {
    let popup = TmuxPopup::build(&opts("top,40%"));
    let a = args(&popup);
    assert_eq!(get_flag(&a, "-h"), Some("40%"));
    assert_eq!(get_flag(&a, "-w"), Some("100%"));
    assert_eq!(get_flag(&a, "-x"), Some("C"));
    assert_eq!(get_flag(&a, "-y"), Some("0%"));
}

#[test]
fn bottom_direction() {
    let popup = TmuxPopup::build(&opts("bottom,30%"));
    let a = args(&popup);
    assert_eq!(get_flag(&a, "-h"), Some("30%"));
    assert_eq!(get_flag(&a, "-w"), Some("100%"));
    assert_eq!(get_flag(&a, "-x"), Some("C"));
    assert_eq!(get_flag(&a, "-y"), Some("100%"));
}

#[test]
fn left_direction() {
    let popup = TmuxPopup::build(&opts("left,30%"));
    let a = args(&popup);
    assert_eq!(get_flag(&a, "-h"), Some("100%"));
    assert_eq!(get_flag(&a, "-w"), Some("30%"));
    assert_eq!(get_flag(&a, "-x"), Some("0%"));
    assert_eq!(get_flag(&a, "-y"), Some("C"));
}

#[test]
fn right_direction() {
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
    let popup = TmuxPopup::build(&opts("center,60%,40%"));
    let a = args(&popup);
    assert_eq!(get_flag(&a, "-w"), Some("60%"));
    assert_eq!(get_flag(&a, "-h"), Some("40%"));
}

#[test]
fn two_dimensional_size_top() {
    // "top,HEIGHT,WIDTH" — for Top/Bottom: height=lhs, width=rhs
    let popup = TmuxPopup::build(&opts("top,30%,80%"));
    let a = args(&popup);
    assert_eq!(get_flag(&a, "-h"), Some("30%"));
    assert_eq!(get_flag(&a, "-w"), Some("80%"));
}

#[test]
fn add_env_appends_e_flag() {
    let mut popup = TmuxPopup::build(&opts("center"));
    popup.add_env("FOO", "bar");
    let a = args(&popup);
    assert!(a.windows(2).any(|w| w[0] == "-e" && w[1] == "FOO=bar"));
}
