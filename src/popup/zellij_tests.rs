use super::*;
use crate::options::SkimOptionsBuilder;

fn opts(popup: &str) -> crate::SkimOptions {
    SkimOptionsBuilder::default()
        .popup(popup)
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
// Tests that mutate COLUMNS are annotated with #[serial] so they never run
// concurrently. `set_var`/`remove_var` are `unsafe fn` in Rust ≥ 1.81
// (edition 2024); the SAFETY invariant holds because #[serial] serialises
// access so no other thread reads the var while it is being written.

#[test]
fn middle_coord_percent() {
    // 50% wide in a 100% viewport → offset should be 25%
    assert_eq!(middle_coord(Size::Percent(50), "COLUMNS"), Size::Percent(25));
}

#[test]
#[serial_test::serial]
fn middle_coord_fixed_uses_env_var() {
    // SAFETY: serialised by #[serial]; no concurrent reads of COLUMNS.
    unsafe { std::env::set_var("COLUMNS", "80") };
    // 20 cols wide → offset = (80 - 20) / 2 = 30
    assert_eq!(middle_coord(Size::Fixed(20), "COLUMNS"), Size::Fixed(30));
    unsafe { std::env::remove_var("COLUMNS") };
}

#[test]
#[serial_test::serial]
fn middle_coord_fixed_fallback() {
    // SAFETY: serialised by #[serial]; no concurrent reads of COLUMNS.
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
fn middle_coord_neg() {
    // Negative sizes are halved into a fixed offset.
    assert_eq!(middle_coord(Size::Neg(20), "COLUMNS"), Size::Fixed(10));
}

#[test]
fn align_end_coord_neg() {
    // Negative sizes map straight to a fixed offset.
    assert_eq!(align_end_coord(Size::Neg(20), "COLUMNS"), Size::Fixed(20));
}

#[test]
#[serial_test::serial]
fn align_end_coord_fixed_uses_env_var() {
    // SAFETY: serialised by #[serial]; no concurrent reads of COLUMNS.
    unsafe { std::env::set_var("COLUMNS", "80") };
    // 20 cols wide → end offset = 80 - 20 = 60
    assert_eq!(align_end_coord(Size::Fixed(20), "COLUMNS"), Size::Fixed(60));
    unsafe { std::env::remove_var("COLUMNS") };
}

// ── from_options / build ─────────────────────────────────────────────────

#[test]
fn center_default_size() {
    let popup = ZellijPopup::build(&opts("center"));
    let a = args(&popup);
    assert_eq!(get_flag(&a, "--height"), Some("50%"));
    assert_eq!(get_flag(&a, "--width"), Some("50%"));
}

#[test]
fn top_direction() {
    let popup = ZellijPopup::build(&opts("top,40%"));
    let a = args(&popup);
    assert_eq!(get_flag(&a, "--height"), Some("40%"));
    assert_eq!(get_flag(&a, "--width"), Some("100%"));
    assert_eq!(get_flag(&a, "-y"), Some("0"));
}

#[test]
fn left_direction() {
    let popup = ZellijPopup::build(&opts("left,30%"));
    let a = args(&popup);
    assert_eq!(get_flag(&a, "--width"), Some("30%"));
    assert_eq!(get_flag(&a, "-x"), Some("0"));
}

#[test]
#[serial_test::serial]
fn right_direction() {
    // SAFETY: serialised by #[serial]; no concurrent reads of COLUMNS.
    unsafe { std::env::set_var("COLUMNS", "80") };
    let popup = ZellijPopup::build(&opts("right,25%"));
    let a = args(&popup);
    // width = 25%, x = align_end_coord(25%, "COLUMNS") = 75%
    assert_eq!(get_flag(&a, "--width"), Some("25%"));
    assert_eq!(get_flag(&a, "-x"), Some("75%"));
    unsafe { std::env::remove_var("COLUMNS") };
}

#[test]
#[serial_test::serial]
fn bottom_direction() {
    // SAFETY: serialised by #[serial]; no concurrent reads of ROWS.
    unsafe { std::env::set_var("ROWS", "40") };
    let popup = ZellijPopup::build(&opts("bottom,25%"));
    let a = args(&popup);
    assert_eq!(get_flag(&a, "--height"), Some("25%"));
    assert_eq!(get_flag(&a, "--width"), Some("100%"));
    // y = align_end_coord(25%, "ROWS") = 75%
    assert_eq!(get_flag(&a, "-y"), Some("75%"));
    unsafe { std::env::remove_var("ROWS") };
}

#[test]
fn explicit_height_and_width() {
    // Two comma-separated sizes give explicit height,width per direction.
    let popup = ZellijPopup::build(&opts("center,40%,30%"));
    let a = args(&popup);
    // Center: (height, width) = (rhs, lhs) = (30%, 40%)
    assert_eq!(get_flag(&a, "--height"), Some("30%"));
    assert_eq!(get_flag(&a, "--width"), Some("40%"));
}

#[test]
fn from_options_builds_popup() {
    // Smoke test that the trait constructor wraps `build` without panicking.
    let _popup: Box<dyn SkimPopup> = ZellijPopup::from_options(&opts("center"));
}

#[test]
fn borderless_when_no_border_option() {
    let popup = ZellijPopup::build(
        &SkimOptionsBuilder::default()
            .popup("center")
            .no_border(true)
            .build()
            .unwrap(),
    );
    let a = args(&popup);
    assert!(a.contains(&"--borderless".to_string()));
}

#[test]
fn no_borderless_when_border_set() {
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
    let mut popup = ZellijPopup::build(&opts("center"));
    popup.add_env("FOO", "bar");
    popup.add_env("BAZ", "qux");
    assert_eq!(popup.env, " FOO=bar BAZ=qux");
}
