use super::*;
use crate::options::SkimOptionsBuilder;

fn opts(popup: &str) -> crate::SkimOptions {
    SkimOptionsBuilder::default()
        .popup(popup)
        .build()
        .expect("valid options")
}

fn args(popup: &RexPopup) -> Vec<String> {
    popup.cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
}

#[test]
fn always_runs_and_waits() {
    let a = args(&RexPopup::build(&opts("center")));
    assert_eq!(&a[..2], ["run", "--wait"]);
}

#[test]
fn center_opens_without_split() {
    let a = args(&RexPopup::build(&opts("center,60%")));
    assert!(!a.iter().any(|s| s.starts_with("--split") || s.starts_with("--ratio")));
}

#[test]
fn directions_map_to_splits() {
    for (dir, side) in [
        ("top", "above"),
        ("bottom", "below"),
        ("left", "left"),
        ("right", "right"),
    ] {
        let a = args(&RexPopup::build(&opts(&format!("{dir},40%"))));
        assert!(a.contains(&format!("--split={side}")), "{dir}: {a:?}");
        assert!(a.contains(&"--ratio=0.40".to_string()), "{dir}: {a:?}");
    }
}

#[test]
fn default_size_is_half() {
    let a = args(&RexPopup::build(&opts("bottom")));
    assert!(a.contains(&"--ratio=0.50".to_string()));
}

#[test]
fn two_sizes_keep_split_axis() {
    let a = args(&RexPopup::build(&opts("top,30%,80%")));
    assert!(a.contains(&"--ratio=0.30".to_string()));
}

#[test]
fn ratio_from_fixed_and_negative_sizes() {
    assert!((size_to_ratio(Size::Fixed(20), 80) - 0.25).abs() < f32::EPSILON);
    assert!((size_to_ratio(Size::Neg(20), 80) - 0.75).abs() < f32::EPSILON);
    // Clamped so the split never collapses either block
    assert!((size_to_ratio(Size::Percent(100), 80) - 0.95).abs() < f32::EPSILON);
    assert!((size_to_ratio(Size::Fixed(0), 80) - 0.05).abs() < f32::EPSILON);
}

#[test]
fn command_sets_cwd_and_env() {
    let mut popup = RexPopup::build(&opts("center"));
    popup.cwd = "/some dir".to_string();
    popup.add_env("SKIM_FOO", "a b");
    popup.add_env("_SKIM_POPUP", "1");
    assert_eq!(
        popup.script(" 'sk' >out"),
        "cd /some' dir' && SKIM_FOO=a' b' _SKIM_POPUP=1  'sk' >out"
    );
}
