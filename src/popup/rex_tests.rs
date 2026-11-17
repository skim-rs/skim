use super::*;
use crate::popup::test_util::{args, opts};

fn assert_near(value: &Value, expected: f64) {
    assert!(
        (value.as_f64().unwrap() - expected).abs() < 0.000_001,
        "{value} != {expected}"
    );
}

#[test]
fn center_creates_floating_layer() {
    let popup = RexPopup::build(&opts("center,60%,40%"));
    assert_eq!(&args(&popup.cmd)[..3], ["api", "call", "--session"]);
    assert_eq!(args(&popup.cmd).last().unwrap(), "session.new_layer");
    for (value, expected) in popup.params["bounds"]
        .as_array()
        .unwrap()
        .iter()
        .zip([0.2, 0.3, 0.6, 0.4])
    {
        assert_near(value, expected);
    }
    assert_eq!(popup.params["focus"], true);
}

#[test]
fn directions_map_to_api_splits() {
    for (dir, direction, side, ratio) in [
        ("top", "vertical", "before", 0.4),
        ("bottom", "vertical", "after", 0.6),
        ("left", "horizontal", "before", 0.4),
        ("right", "horizontal", "after", 0.6),
    ] {
        let popup = RexPopup::build(&opts(&format!("{dir},40%")));
        assert_eq!(args(&popup.cmd).last().unwrap(), "session.new_split");
        assert_eq!(popup.params["direction"], direction);
        assert_eq!(popup.params["side"], side);
        assert_near(&popup.params["ratio"], ratio);
    }
}

#[test]
fn default_size_is_half() {
    assert_near(&RexPopup::build(&opts("bottom")).params["ratio"], 0.5);
    let popup = RexPopup::build(&opts("center"));
    for (value, expected) in popup.params["bounds"]
        .as_array()
        .unwrap()
        .iter()
        .zip([0.25, 0.25, 0.5, 0.5])
    {
        assert_near(value, expected);
    }
}

#[test]
fn sizes_and_limits() {
    assert_eq!(size_to_ratio(Size::Fixed(20), 80), 0.25);
    assert_eq!(size_to_ratio(Size::Neg(20), 80), 0.75);
    assert_eq!(size_to_ratio(Size::Percent(100), 80), 1.0);
    assert_eq!(size_to_ratio(Size::Fixed(0), 0), 0.01);
    assert_near(&RexPopup::build(&opts("right,100%")).params["ratio"], 0.05);
    assert_eq!(
        RexPopup::build(&opts("center,100%")).params["bounds"],
        json!([0.0, 0.0, 1.0, 1.0])
    );
}

#[test]
fn command_sets_cwd_and_env() {
    let mut popup = RexPopup::build(&opts("center"));
    popup.cwd = "/some dir".to_string();
    popup.add_env("SKIM_FOO", "a b");
    popup.add_env("_SKIM_POPUP", "1");
    let request = popup.request(" 'sk' >out");
    let options = &request["layout"]["block"]["options"];
    assert_eq!(
        options["command"],
        json!(["sh", "-c", "cd /some' dir' && SKIM_FOO=a' b' _SKIM_POPUP=1  'sk' >out"])
    );
    assert_eq!(options["shell"], "none");
    assert_eq!(options["exit"]["on_completion"], false);
}

#[test]
#[cfg(unix)]
fn color_settings_reach_child_unchanged() {
    let mut popup = RexPopup::build(&opts("center"));
    let colors = "di=38;2;137;180;250:*.rs=38;2;166;227;161:";
    for (name, value) in [("LS_COLORS", colors), ("COLORTERM", "truecolor"), ("NO_COLOR", "1")] {
        assert!(super::super::should_forward_env(name));
        popup.add_env(name, value);
    }
    let output = Command::new("sh")
        .args([
            "-c",
            &popup.script("sh -c 'printf \"%s\\n\" \"$LS_COLORS\" \"$COLORTERM\" \"$NO_COLOR\"'"),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{colors}\ntruecolor\n1\n")
    );
}

#[cfg(unix)]
fn mock_popup(dir: &tempfile::TempDir, creation: &str, wait_status: u8, close_status: u8) -> RexPopup {
    use std::os::unix::fs::PermissionsExt;

    let bin = dir.path().join("rex");
    let log = dir.path().join("calls");
    std::fs::write(
        &bin,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}\n\
         case \"$1:$5\" in\n\
         api:session.new_layer) printf '%s\\n' {}; exit 0;;\n\
         wait:*) exit {wait_status};;\n\
         api:block.close) echo 'close failed' >&2; exit {close_status};;\n\
         *) exit 99;;\nesac\n",
            sh_quote(&log.to_string_lossy()),
            sh_quote(creation)
        ),
    )
    .unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut popup = RexPopup::build(&opts("center"));
    popup.cmd = Command::new(bin);
    popup
        .cmd
        .args(["api", "call", "--session", "session:test", "session.new_layer"]);
    popup.session = "session:test".into();
    popup
}

#[test]
#[cfg(unix)]
fn waits_for_child_and_closes_block_even_on_abort() {
    for code in [0, 130] {
        let dir = tempfile::tempdir().unwrap();
        let mut popup = mock_popup(&dir, r#"{"block_ids":["block:test"]}"#, code, 0);
        assert_eq!(popup.run_and_wait("sk").unwrap().code(), Some(i32::from(code)));
        let calls = std::fs::read_to_string(dir.path().join("calls")).unwrap();
        let lines: Vec<_> = calls.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1], "wait --session session:test block:test");
        assert_eq!(
            lines[2],
            "api call --session session:test block.close {\"block_id\":\"block:test\"}"
        );
    }
}

#[test]
#[cfg(unix)]
fn rejects_invalid_creation_results_and_cleanup_errors() {
    for response in ["not json", "{}", r#"{"block_ids":[]}"#] {
        let dir = tempfile::tempdir().unwrap();
        assert!(mock_popup(&dir, response, 0, 0).run_and_wait("sk").is_err());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("calls"))
                .unwrap()
                .lines()
                .count(),
            1
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let error = mock_popup(&dir, r#"{"block_ids":["block:test"]}"#, 0, 1)
        .run_and_wait("sk")
        .unwrap_err();
    assert!(error.to_string().contains("close failed"));
}

#[test]
#[cfg(unix)]
fn reports_creation_errors_without_waiting() {
    let dir = tempfile::tempdir().unwrap();
    let mut popup = mock_popup(&dir, "{}", 0, 0);
    std::fs::write(
        popup.cmd.get_program(),
        "#!/bin/sh\necho 'creation failed' >&2\nexit 1\n",
    )
    .unwrap();
    assert!(
        popup
            .run_and_wait("sk")
            .unwrap_err()
            .to_string()
            .contains("creation failed")
    );
    assert!(!dir.path().join("calls").exists());
}

#[test]
#[cfg(unix)]
fn accepts_a_pane_closed_by_the_user() {
    let dir = tempfile::tempdir().unwrap();
    let mut popup = mock_popup(&dir, r#"{"block_ids":["block:test"]}"#, 1, 1);
    let bin = popup.cmd.get_program();
    let script = std::fs::read_to_string(bin)
        .unwrap()
        .replace("close failed", "code=not_found");
    std::fs::write(bin, script).unwrap();
    assert_eq!(popup.run_and_wait("sk").unwrap().code(), Some(1));
}
