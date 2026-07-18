use super::*;

const NO_ARG_ACTIONS: &[&str] = &[
    "abort",
    "append-and-select",
    "backward-char",
    "backward-delete-char",
    "backward-delete-char/eof",
    "backward-kill-word",
    "backward-word",
    "beginning-of-line",
    "cancel",
    "clear-screen",
    "delete-char",
    "delete-char/eof",
    "deselect-all",
    "end-of-line",
    "first",
    "forward-char",
    "forward-word",
    "ignore",
    "kill-line",
    "kill-word",
    "last",
    "next-history",
    "previous-history",
    "redraw",
    "refresh-cmd",
    "refresh-preview",
    "restart-matcher",
    "rotate-mode",
    "select",
    "select-all",
    "suppress",
    "toggle",
    "toggle-all",
    "toggle-in",
    "toggle-interactive",
    "toggle-out",
    "toggle-preview",
    "toggle-preview-wrap",
    "toggle-sort",
    "top",
    "unix-line-discard",
    "unix-word-rubout",
    "yank",
];

#[test]
fn parse_all_no_arg_actions() {
    for name in NO_ARG_ACTIONS {
        let action = parse_action(name).unwrap_or_else(|| panic!("expected `{name}` to parse"));
        assert_eq!(action.name(), *name);
    }
}

#[test]
fn parse_numeric_actions_default_to_one() {
    for name in [
        "down",
        "up",
        "half-page-down",
        "half-page-up",
        "page-down",
        "page-up",
        "preview-up",
        "preview-down",
        "preview-left",
        "preview-right",
        "preview-page-up",
        "preview-page-down",
        "scroll-left",
        "scroll-right",
        "select-row",
    ] {
        let action = parse_action(name).unwrap_or_else(|| panic!("expected `{name}` to parse"));
        assert_eq!(action.name(), name);
    }
}

#[test]
fn parse_numeric_actions_with_colon_arg() {
    assert_eq!(parse_action("down:3"), Some(Action::Down(3)));
    assert_eq!(parse_action("up:5"), Some(Action::Up(5)));
    assert_eq!(parse_action("half-page-down:2"), Some(Action::HalfPageDown(2)));
    assert_eq!(parse_action("preview-up:4"), Some(Action::PreviewUp(4)));
    assert_eq!(parse_action("select-row:7"), Some(Action::SelectRow(7)));
}

#[test]
fn parse_numeric_actions_with_paren_arg() {
    assert_eq!(parse_action("down(3)"), Some(Action::Down(3)));
    assert_eq!(parse_action("scroll-right(2)"), Some(Action::ScrollRight(2)));
}

#[test]
fn parse_string_arg_actions() {
    for (spec, name) in [
        ("execute:ls -la", "execute"),
        ("execute-silent:touch x", "execute-silent"),
        ("set-query:hello", "set-query"),
        ("set-preview-cmd:cat {}", "set-preview-cmd"),
        ("add-char:z", "add-char"),
    ] {
        assert_eq!(parse_action(spec).map(|action| action.name()), Some(name));
    }

    assert_eq!(
        parse_action("execute:ls -la"),
        Some(Action::Execute("ls -la".to_string()))
    );
    assert_eq!(
        parse_action("execute-silent:touch x"),
        Some(Action::ExecuteSilent("touch x".to_string()))
    );
    assert_eq!(
        parse_action("set-query:hello"),
        Some(Action::SetQuery("hello".to_string()))
    );
    assert_eq!(
        parse_action("set-preview-cmd:cat {}"),
        Some(Action::SetPreviewCmd("cat {}".to_string()))
    );
    assert_eq!(parse_action("add-char:z"), Some(Action::AddChar('z')));
}

#[test]
fn parse_optional_arg_actions() {
    for name in ["accept", "set-header", "reload"] {
        assert_eq!(parse_action(name).map(|action| action.name()), Some(name));
    }

    assert_eq!(parse_action("accept"), Some(Action::Accept(None)));
    assert_eq!(
        parse_action("accept:enter"),
        Some(Action::Accept(Some("enter".to_string())))
    );
    assert_eq!(parse_action("set-header"), Some(Action::SetHeader(None)));
    assert_eq!(parse_action("reload"), Some(Action::Reload(None)));
    assert_eq!(
        parse_action("reload:find ."),
        Some(Action::Reload(Some("find .".to_string())))
    );
}

#[test]
fn parse_bind_and_unbind_actions() {
    // `bind` captures the whole `key:action` spec as its string argument, using
    // either the paren or colon form.
    assert_eq!(
        parse_action("bind(ctrl-a:accept)"),
        Some(Action::Bind("ctrl-a:accept".to_string()))
    );
    assert_eq!(
        parse_action("bind:ctrl-a:accept"),
        Some(Action::Bind("ctrl-a:accept".to_string()))
    );
    // `unbind` captures a comma-separated list of keys, like fzf's `unbind(...)`.
    assert_eq!(
        parse_action("unbind(ctrl-a)"),
        Some(Action::Unbind("ctrl-a".to_string()))
    );
    assert_eq!(
        parse_action("unbind(ctrl-a,ctrl-b)"),
        Some(Action::Unbind("ctrl-a,ctrl-b".to_string()))
    );
}

#[test]
fn parse_bind_and_unbind_require_argument() {
    // Without an argument both actions are rejected rather than silently
    // producing an empty binding.
    assert_eq!(parse_action("bind"), None);
    assert_eq!(parse_action("bind:"), None);
    assert_eq!(parse_action("unbind"), None);
    assert_eq!(parse_action("unbind:"), None);
}

#[test]
fn parse_if_chains_then_only() {
    for name in ["if-query-empty", "if-query-not-empty", "if-non-matched"] {
        let spec = format!("{name}:abort");
        assert_eq!(parse_action(&spec).map(|action| action.name()), Some(name));
    }

    assert_eq!(
        parse_action("if-query-empty:abort"),
        Some(Action::IfQueryEmpty("abort".to_string(), None))
    );
    assert_eq!(
        parse_action("if-non-matched:ignore"),
        Some(Action::IfNonMatched("ignore".to_string(), None))
    );
}

#[test]
fn parse_if_chains_then_and_else() {
    assert_eq!(
        parse_action("if-query-not-empty:abort+ignore"),
        Some(Action::IfQueryNotEmpty("abort".to_string(), Some("ignore".to_string())))
    );
}

#[test]
fn parse_numeric_action_with_invalid_arg_falls_back_to_default() {
    // A non-numeric argument is ignored and the default count is used.
    assert_eq!(parse_action("down:abc"), Some(Action::Down(1)));
    assert_eq!(parse_action("page-up:xyz"), Some(Action::PageUp(1)));
    // SelectRow defaults to 0 rather than 1.
    assert_eq!(parse_action("select-row:nope"), Some(Action::SelectRow(0)));
}

#[test]
fn parse_unknown_action_returns_none() {
    assert_eq!(parse_action("not-a-real-action"), None);
}

#[test]
fn parse_action_trailing_separator_yields_no_arg() {
    // A separator with nothing after it (`act:`) is treated as if no argument
    // was supplied, so optional-arg actions fall back to their `None` form
    // rather than being handed an empty string.
    assert_eq!(parse_action("accept:"), Some(Action::Accept(None)));
    assert_eq!(parse_action("reload:"), Some(Action::Reload(None)));
    assert_eq!(parse_action("set-header:"), Some(Action::SetHeader(None)));
    // Numeric actions fall back to their default count for the same reason.
    assert_eq!(parse_action("down:"), Some(Action::Down(1)));
    assert_eq!(parse_action("select-row:"), Some(Action::SelectRow(0)));
}

#[test]
fn parse_if_chain_with_trailing_plus_has_empty_else() {
    // A trailing `+` yields a then-branch with no otherwise-branch.
    assert_eq!(
        parse_action("if-query-empty:abort+"),
        Some(Action::IfQueryEmpty("abort".to_string(), None))
    );
}

#[test]
fn parse_if_chain_unknown_kind_returns_none() {
    // An `if-` prefixed action that is not one of the known kinds is rejected.
    assert_eq!(parse_action("if-bogus:abort"), None);
}

#[test]
fn action_callback_debug_is_opaque() {
    let cb = ActionCallback::new_sync(|_app| Ok(vec![]));
    assert_eq!(format!("{cb:?}"), "ActionCallback");
}

#[test]
fn action_callback_async_constructor_builds() {
    // The async constructor wraps the closure without invoking it.
    let cb = ActionCallback::new(|_app| async move { Ok(vec![Event::Render]) });
    // Cloning shares the same inner callback.
    let _clone = cb.clone();
    assert_eq!(format!("{cb:?}"), "ActionCallback");
}
