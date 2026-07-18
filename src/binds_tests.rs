use super::*;
use event::Action::*;
#[test]
fn test_parse_action_chain() {
    let parsed = parse_action_chain(
        "execute-silent:1 {}+execute-silent:2 {+}+execute-silent:3 {+n}+reload+if-query-empty:reload+up",
    );
    assert!(parsed.is_ok());
    let res = parsed.unwrap();
    assert_eq!(
        res,
        vec![
            ExecuteSilent("1 {}".into()),
            ExecuteSilent("2 {+}".into()),
            ExecuteSilent("3 {+n}".into()),
            Reload(None),
            IfQueryEmpty("reload".into(), Some("up".into())),
        ]
    );
}
#[test]
fn test_parse_key() {
    assert_eq!(
        parse_key("a").unwrap(),
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty())
    );

    assert_eq!(
        parse_key("A").unwrap(),
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::SHIFT)
    );

    assert_eq!(
        parse_key("alt-a").unwrap(),
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::ALT)
    );

    assert_eq!(
        parse_key("alt-A").unwrap(),
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::ALT | KeyModifiers::SHIFT)
    );
    assert_eq!(
        parse_key("alt-shift-a").unwrap(),
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::ALT | KeyModifiers::SHIFT)
    );

    assert_eq!(
        parse_key("ctrl-a").unwrap(),
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)
    );

    assert_eq!(
        parse_key("ctrl-A").unwrap(),
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL | KeyModifiers::SHIFT)
    );
    assert_eq!(
        parse_key("ctrl-shift-a").unwrap(),
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL | KeyModifiers::SHIFT)
    );

    assert_eq!(
        parse_key("f10").unwrap(),
        KeyEvent::new(KeyCode::F(10), KeyModifiers::empty())
    );

    assert_eq!(
        parse_key("space").unwrap(),
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty())
    );
    assert_eq!(
        parse_key("enter").unwrap(),
        KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())
    );
    assert_eq!(
        parse_key("bspace").unwrap(),
        KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty())
    );
    assert_eq!(
        parse_key("bs").unwrap(),
        KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty())
    );
    assert_eq!(
        parse_key("up").unwrap(),
        KeyEvent::new(KeyCode::Up, KeyModifiers::empty())
    );
    assert_eq!(
        parse_key("down").unwrap(),
        KeyEvent::new(KeyCode::Down, KeyModifiers::empty())
    );
    assert_eq!(
        parse_key("left").unwrap(),
        KeyEvent::new(KeyCode::Left, KeyModifiers::empty())
    );
    assert_eq!(
        parse_key("right").unwrap(),
        KeyEvent::new(KeyCode::Right, KeyModifiers::empty())
    );
    assert_eq!(
        parse_key("tab").unwrap(),
        KeyEvent::new(KeyCode::Tab, KeyModifiers::empty())
    );
    assert_eq!(
        parse_key("btab").unwrap(),
        KeyEvent::new(KeyCode::BackTab, KeyModifiers::empty())
    );
    assert_eq!(
        parse_key("esc").unwrap(),
        KeyEvent::new(KeyCode::Esc, KeyModifiers::empty())
    );
    assert_eq!(
        parse_key("home").unwrap(),
        KeyEvent::new(KeyCode::Home, KeyModifiers::empty())
    );
    assert_eq!(
        parse_key("end").unwrap(),
        KeyEvent::new(KeyCode::End, KeyModifiers::empty())
    );
    assert_eq!(
        parse_key("pgup").unwrap(),
        KeyEvent::new(KeyCode::PageUp, KeyModifiers::empty())
    );
    assert_eq!(
        parse_key("pgdown").unwrap(),
        KeyEvent::new(KeyCode::PageDown, KeyModifiers::empty())
    );
    assert_eq!(
        parse_key("change").unwrap(),
        KeyEvent::new(KeyCode::F(255), KeyModifiers::empty())
    );
}

#[test]
fn parse_key_error_cases() {
    // Empty input.
    assert!(parse_key("").is_err());
    // Unknown modifier.
    assert!(parse_key("hyper-a").is_err());
    // Unknown key name.
    assert!(parse_key("notakey").is_err());
    // Invalid function-key index.
    assert!(parse_key("fXY").is_err());
}

#[test]
fn keymap_from_str_parses_bindings() {
    let keymap = KeyMap::from("ctrl-a:abort,enter:accept");
    // Both keys resolve to action chains.
    assert!(keymap.get(&parse_key("ctrl-a").unwrap()).is_some());
    assert!(keymap.get(&parse_key("enter").unwrap()).is_some());
}

#[test]
fn keymap_from_str_preserves_nested_bind_separators() {
    let keymap = KeyMap::from("ctrl-z:bind(ctrl-x:abort+up),ctrl-w:unbind(ctrl-x,ctrl-y)");

    assert_eq!(
        keymap.get(&parse_key("ctrl-z").unwrap()),
        Some(&vec![Bind("ctrl-x:abort+up".into())])
    );
    assert_eq!(
        keymap.get(&parse_key("ctrl-w").unwrap()),
        Some(&vec![Unbind("ctrl-x,ctrl-y".into())])
    );
}

#[test]
fn parse_action_chain_preserves_nested_bind_chain() {
    assert_eq!(
        parse_action_chain("bind(ctrl-x:abort+up)").unwrap(),
        vec![Bind("ctrl-x:abort+up".into())]
    );
}

#[test]
fn parse_keymaps_collects_iterator() {
    let keymap = parse_keymaps(["ctrl-x:abort", "up:up"].into_iter());
    assert!(keymap.get(&parse_key("ctrl-x").unwrap()).is_some());
}

#[test]
fn parse_action_chain_unknown_is_error() {
    assert!(parse_action_chain("not-a-real-action").is_err());
}

#[test]
fn parse_action_chain_accept_execute_reload_with_args() {
    // `accept:hello`, `execute(...)` and `reload(...)` parse to the expected actions.
    assert_eq!(
        parse_action_chain("accept:hello").unwrap(),
        vec![Accept(Some("hello".into()))]
    );
    assert_eq!(
        parse_action_chain("execute(echo foo)").unwrap(),
        vec![Execute("echo foo".into())]
    );
    assert_eq!(
        parse_action_chain("reload(echo hello)").unwrap(),
        vec![Reload(Some("echo hello".into()))]
    );
    assert_eq!(parse_action_chain("reload").unwrap(), vec![Reload(None)]);
}
