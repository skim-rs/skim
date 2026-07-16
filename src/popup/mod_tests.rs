use super::*;

// ── PopupWindowDir::from ──────────────────────────────────────────────────

#[test]
fn popup_window_dir_known_values() {
    assert_eq!(PopupWindowDir::from("center"), PopupWindowDir::Center);
    assert_eq!(PopupWindowDir::from("top"), PopupWindowDir::Top);
    assert_eq!(PopupWindowDir::from("bottom"), PopupWindowDir::Bottom);
    assert_eq!(PopupWindowDir::from("left"), PopupWindowDir::Left);
    assert_eq!(PopupWindowDir::from("right"), PopupWindowDir::Right);
}

#[test]
fn popup_window_dir_unknown_falls_back_to_center() {
    assert_eq!(PopupWindowDir::from(""), PopupWindowDir::Center);
    assert_eq!(PopupWindowDir::from("foobar"), PopupWindowDir::Center);
    assert_eq!(PopupWindowDir::from("CENTER"), PopupWindowDir::Center); // case-sensitive
}

// ── sanitize_value ────────────────────────────────────────────────────────

#[test]
fn sanitize_value_no_semicolon() {
    assert_eq!(sanitize_value("hello".to_string()), "hello");
    assert_eq!(sanitize_value("foo=bar".to_string()), "foo=bar");
    assert_eq!(sanitize_value(String::new()), "");
}

#[test]
fn sanitize_value_trailing_semicolon_is_escaped() {
    assert_eq!(sanitize_value("hello;".to_string()), "hello\\;");
    assert_eq!(sanitize_value(";".to_string()), "\\;");
}

#[test]
fn sanitize_value_semicolon_in_middle_unchanged() {
    assert_eq!(sanitize_value("hel;lo".to_string()), "hel;lo");
    assert_eq!(sanitize_value("a;b;c".to_string()), "a;b;c");
}

// ── push_quoted_arg ───────────────────────────────────────────────────────
// `push_quoted_arg` always quotes with `shell_quote::Sh`; it does not read the
// SHELL env var, so these tests need no env setup or serialisation.

#[test]
fn push_quoted_arg_simple_word() {
    let mut s = String::new();
    push_quoted_arg(&mut s, "hello");
    assert_eq!(s, " hello");
}

#[test]
fn push_quoted_arg_spaces_are_quoted() {
    let mut s = String::new();
    push_quoted_arg(&mut s, "hello world");
    // The result must preserve both words and not be a bare unquoted string
    assert!(s.contains("hello"));
    assert!(s.contains("world"));
    assert_ne!(s.trim(), "hello world"); // must be quoted somehow
}

#[test]
fn push_quoted_arg_appends_with_space_prefix() {
    let mut s = String::from("sk");
    push_quoted_arg(&mut s, "--flag");
    assert!(s.starts_with("sk "));
}

// ── SkimPopupOutput ───────────────────────────────────────────────────────

#[test]
fn popup_output_text_returns_line() {
    let out = SkimPopupOutput {
        line: "hello world".to_string(),
    };
    assert_eq!(out.text(), "hello world");
}

// ── check_env ─────────────────────────────────────────────────────────────

#[test]
#[serial_test::serial]
fn check_env_false_when_already_in_popup() {
    // SAFETY: serialised by #[serial]; no concurrent reads of _SKIM_POPUP.
    unsafe { std::env::set_var("_SKIM_POPUP", "1") };
    // Already inside a popup → never re-enter regardless of multiplexer.
    assert!(!check_env());
    unsafe { std::env::remove_var("_SKIM_POPUP") };
}

#[test]
#[serial_test::serial]
fn check_env_reflects_multiplexer_availability() {
    // SAFETY: serialised by #[serial]; no concurrent reads of _SKIM_POPUP.
    unsafe { std::env::remove_var("_SKIM_POPUP") };
    // Outside a popup, the result mirrors whether a multiplexer is available.
    let expected = tmux::is_available() || zellij::is_available();
    assert_eq!(check_env(), expected);
}
