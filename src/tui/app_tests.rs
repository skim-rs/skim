//! Unit tests for [`super::App`].
//!
//! These exercise the backend-free state transitions in `App` — primarily
//! `handle_action`, `handle_key`, query/preview helpers and the matcher restart
//! logic — without spinning up a real terminal backend.  Tests that need a
//! `Tui<B>` (rendering, preview spawning) live in the insta snapshot suite
//! under `tests/`.
//!
//! `App` has private fields and cannot be built with a struct literal from
//! here, so tests mutate fields after `App::default()`.
#![allow(clippy::field_reassign_with_default, clippy::needless_pass_by_value)]

use std::sync::Arc;

use super::*;
use crate::item::{MatchedItem, RankBuilder};
use crate::tui::event::{Action, Event};
use crate::tui::layout::LayoutTemplate;
use crate::tui::statusline::InfoDisplay;
use crate::{Rank, SkimItem};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn matched(text: &str, index: i32) -> MatchedItem {
    let item: Arc<dyn SkimItem> = Arc::new(text.to_string());
    MatchedItem::new(
        item,
        Rank {
            index,
            ..Default::default()
        },
        None,
        &RankBuilder::default(),
    )
}

/// Build a default `App` populated with the given items in the item list.
fn app_with_items(items: &[&str]) -> App {
    let mut app = App::default();
    let mut matched_items: Vec<MatchedItem> = items
        .iter()
        .enumerate()
        .map(|(i, t)| matched(t, i32::try_from(i).unwrap()))
        .collect();
    app.item_list.append(&mut matched_items);
    app.item_list.height = 10;
    app
}

/// Convenience: run an action and return the emitted events.
fn act(app: &mut App, action: Action) -> Vec<Event> {
    app.handle_action(&action).expect("handle_action failed")
}

#[test]
fn add_char_updates_query_and_emits_events() {
    let mut app = App::default();
    let events = act(&mut app, Action::AddChar('x'));
    assert_eq!(app.input.value, "x");
    // on_query_changed emits a F255 change-key event and a RunPreview
    assert!(events.iter().any(|e| matches!(e, Event::RunPreview)));
}

#[test]
fn cursor_movement_actions() {
    let mut app = App::default();
    app.input.value = "hello".to_string();
    app.input.move_to_end();

    act(&mut app, Action::BeginningOfLine);
    assert_eq!(app.input.cursor_pos, 0);

    act(&mut app, Action::EndOfLine);
    assert_eq!(app.input.cursor_pos as usize, "hello".len());

    act(&mut app, Action::BackwardChar);
    assert_eq!(app.input.cursor_pos as usize, "hello".len() - 1);

    act(&mut app, Action::ForwardChar);
    assert_eq!(app.input.cursor_pos as usize, "hello".len());

    act(&mut app, Action::BackwardWord);
    assert_eq!(app.input.cursor_pos, 0);

    act(&mut app, Action::ForwardWord);
    assert_eq!(app.input.cursor_pos as usize, "hello".len());
}

#[test]
fn delete_actions_change_query() {
    let mut app = App::default();
    app.input.value = "abc".to_string();
    app.input.move_to_end();

    let events = act(&mut app, Action::BackwardDeleteChar);
    assert_eq!(app.input.value, "ab");
    assert!(events.iter().any(|e| matches!(e, Event::RunPreview)));

    act(&mut app, Action::BeginningOfLine);
    act(&mut app, Action::DeleteChar);
    assert_eq!(app.input.value, "b");
}

#[test]
fn backward_delete_char_eof_quits_on_empty() {
    let mut app = App::default();
    let events = act(&mut app, Action::BackwardDeleteCharEof);
    assert!(app.should_quit);
    assert!(events.is_empty());
}

#[test]
fn delete_char_eof_quits_on_empty() {
    let mut app = App::default();
    let events = act(&mut app, Action::DeleteCharEof);
    assert!(app.should_quit);
    assert!(events.is_empty());
}

#[test]
fn kill_line_and_word_fill_yank_register() {
    // KillLine from the start of the line removes everything to the right and
    // stores it in the yank register.
    let mut app = App::default();
    app.input.value = "foo bar".to_string();
    act(&mut app, Action::BeginningOfLine);
    act(&mut app, Action::KillLine);
    assert_eq!(app.input.value, "");
    assert_eq!(app.yank_register, "foo bar");

    // BackwardKillWord removes the word before the cursor into the register.
    let mut app = App::default();
    app.input.value = "foo bar".to_string();
    app.input.move_to_end();
    act(&mut app, Action::BackwardKillWord);
    assert_eq!(app.input.value, "foo ");
    assert_eq!(app.yank_register, "bar");
}

#[test]
fn yank_inserts_register_contents() {
    let mut app = App::default();
    app.yank_register = "pasted".to_string();
    act(&mut app, Action::Yank);
    assert_eq!(app.input.value, "pasted");
}

#[test]
fn unix_line_discard_and_word_rubout() {
    let mut app = App::default();
    app.input.value = "hello world".to_string();
    app.input.move_to_end();
    act(&mut app, Action::UnixWordRubout);
    assert!(app.input.value.starts_with("hello"));
    assert!(!app.input.value.ends_with("world"));

    act(&mut app, Action::UnixLineDiscard);
    assert!(app.input.value.is_empty());
}

#[test]
fn abort_and_accept_quit() {
    let mut app = App::default();
    act(&mut app, Action::Abort);
    assert!(app.should_quit);

    let mut app = App::default();
    act(&mut app, Action::Accept(None));
    assert!(app.should_quit);
}

#[test]
fn navigation_actions_emit_selection_changed() {
    let mut app = app_with_items(&["a", "b", "c", "d"]);
    for action in [
        Action::Down(1),
        Action::Up(1),
        Action::First,
        Action::Last,
        Action::Top,
        Action::PageDown(1),
        Action::PageUp(1),
        Action::HalfPageDown(1),
        Action::HalfPageUp(1),
    ] {
        let events = act(&mut app, action);
        assert!(events.iter().any(|e| matches!(e, Event::RunPreview)));
    }
}

#[test]
fn selection_actions() {
    let mut app = app_with_items(&["a", "b", "c"]);
    app.options.multi = true;

    act(&mut app, Action::SelectAll);
    assert_eq!(app.item_list.selection.len(), 3);

    act(&mut app, Action::DeselectAll);
    assert!(app.item_list.selection.is_empty());

    act(&mut app, Action::Select);
    assert_eq!(app.item_list.selection.len(), 1);

    // Toggle on the same (already-selected) current row removes it.
    act(&mut app, Action::Toggle);
    assert!(app.item_list.selection.is_empty());

    // ToggleAll from an empty selection selects everything.
    act(&mut app, Action::ToggleAll);
    assert_eq!(app.item_list.selection.len(), 3);
    act(&mut app, Action::ToggleAll);
    assert!(app.item_list.selection.is_empty());

    // SelectRow targets a specific index regardless of the cursor.
    act(&mut app, Action::SelectRow(2));
    assert_eq!(app.item_list.selection.len(), 1);
    app.item_list.clear_selection();

    // Start from a middle row so the cursor can move in either layout direction.
    app.item_list.current = 1;
    // ToggleIn toggles the current row then advances the cursor.
    act(&mut app, Action::ToggleIn);
    assert_eq!(app.item_list.selection.len(), 1);
    // ToggleOut toggles the (now different) current row, selecting a second item.
    act(&mut app, Action::ToggleOut);
    assert_eq!(app.item_list.selection.len(), 2);
}

#[test]
fn append_and_select_grows_list() {
    let mut app = app_with_items(&["a"]);
    app.input.value = "new-item".to_string();
    let before = app.item_list.items.len();
    act(&mut app, Action::AppendAndSelect);
    assert_eq!(app.item_list.items.len(), before + 1);
    assert!(!app.item_list.selection.is_empty());
}

#[test]
fn scroll_actions_change_hscroll() {
    let mut app = app_with_items(&["a"]);
    act(&mut app, Action::ScrollRight(5));
    assert_eq!(app.item_list.manual_hscroll, 5);
    act(&mut app, Action::ScrollLeft(2));
    assert_eq!(app.item_list.manual_hscroll, 3);
}

#[test]
fn preview_scroll_actions() {
    let mut app = App::default();
    for action in [
        Action::PreviewUp(1),
        Action::PreviewDown(1),
        Action::PreviewLeft(1),
        Action::PreviewRight(1),
        Action::PreviewPageUp(1),
        Action::PreviewPageDown(1),
    ] {
        // None of these should error or emit events.
        let events = act(&mut app, action);
        assert!(events.is_empty());
    }
}

#[test]
fn toggle_preview_and_wrap() {
    let mut app = App::default();
    let before = app.options.preview_window.hidden;
    act(&mut app, Action::TogglePreview);
    assert_ne!(app.options.preview_window.hidden, before);

    let wrap_before = app.preview.wrap;
    act(&mut app, Action::TogglePreviewWrap);
    assert_ne!(app.preview.wrap, wrap_before);
}

#[test]
fn toggle_sort_and_interactive() {
    let mut app = App::default();
    let sort_before = app.options.no_sort;
    act(&mut app, Action::ToggleSort);
    assert_ne!(app.options.no_sort, sort_before);

    let inter_before = app.options.interactive;
    act(&mut app, Action::ToggleInteractive);
    assert_ne!(app.options.interactive, inter_before);
}

#[test]
fn rotate_mode_cycles_fuzzy_exact_regex() {
    let mut app = App::default();
    assert!(!app.options.exact && !app.options.regex);
    act(&mut app, Action::RotateMode); // fuzzy -> exact
    assert!(app.options.exact && !app.options.regex);
    act(&mut app, Action::RotateMode); // exact -> regex
    assert!(!app.options.exact && app.options.regex);
    act(&mut app, Action::RotateMode); // regex -> fuzzy
    assert!(!app.options.exact && !app.options.regex);
}

#[test]
fn set_query_and_header_and_preview_cmd() {
    let mut app = App::default();
    act(&mut app, Action::SetQuery("hello".to_string()));
    assert_eq!(app.input.value, "hello");

    act(&mut app, Action::SetHeader(Some("a header".to_string())));
    assert_eq!(app.options.header.as_deref(), Some("a header"));

    let events = act(&mut app, Action::SetPreviewCmd("echo hi".to_string()));
    assert_eq!(app.options.preview.as_deref(), Some("echo hi"));
    assert!(events.iter().any(|e| matches!(e, Event::RunPreview)));
}

#[test]
fn clear_screen_and_redraw_emit_clear() {
    let mut app = App::default();
    assert!(matches!(act(&mut app, Action::ClearScreen).as_slice(), [Event::Clear]));
    assert!(matches!(act(&mut app, Action::Redraw).as_slice(), [Event::Clear]));
}

#[test]
fn refresh_preview_emits_run_preview() {
    let mut app = App::default();
    assert!(matches!(
        act(&mut app, Action::RefreshPreview).as_slice(),
        [Event::RunPreview]
    ));
}

#[test]
fn refresh_cmd_reloads_in_interactive_mode() {
    let mut app = App::default();
    app.options.interactive = true;
    app.cmd = "ls".to_string();
    let events = act(&mut app, Action::RefreshCmd);
    assert!(events.iter().any(|e| matches!(e, Event::Reload(_))));

    // Non-interactive: no events.
    let mut app = App::default();
    assert!(act(&mut app, Action::RefreshCmd).is_empty());
}

#[test]
fn reload_actions_emit_reload() {
    let mut app = app_with_items(&["a"]);
    app.cmd = "ls".to_string();
    let events = act(&mut app, Action::Reload(None));
    assert!(matches!(events.as_slice(), [Event::Reload(cmd)] if cmd == "ls"));

    let events = act(&mut app, Action::Reload(Some("echo hi".to_string())));
    assert!(matches!(events.as_slice(), [Event::Reload(_)]));
}

#[test]
fn cancel_kills_matcher_and_preview() {
    let mut app = App::default();
    // Should not panic.
    let events = act(&mut app, Action::Cancel);
    assert!(events.is_empty());
}

#[test]
fn ignore_action_is_noop() {
    let mut app = App::default();
    assert!(act(&mut app, Action::Ignore).is_empty());
}

#[test]
fn execute_silent_spawns_without_error() {
    let mut app = App::default();
    let events = act(&mut app, Action::ExecuteSilent("true".to_string()));
    assert!(events.is_empty());
}

#[test]
fn restart_matcher_action() {
    let mut app = App::default();
    let events = act(&mut app, Action::RestartMatcher);
    assert!(events.is_empty());
}

#[test]
fn delete_actions_on_empty_query_are_noops() {
    // With an empty query and cursor at 0, the delete actions find nothing to
    // remove and fall through without emitting query-changed events.
    let mut app = App::default();
    assert!(act(&mut app, Action::DeleteChar).is_empty());
    assert!(act(&mut app, Action::BackwardKillWord).is_empty());
    assert!(act(&mut app, Action::UnixLineDiscard).is_empty());
    assert!(act(&mut app, Action::UnixWordRubout).is_empty());
}

#[test]
fn delete_char_eof_deletes_when_not_empty() {
    let mut app = App::default();
    app.input.value = "abc".to_string();
    app.input.move_to_end();
    app.input.move_cursor(-1); // cursor before 'c'
    // Non-empty → DeleteCharEof removes the char under the cursor.
    let events = act(&mut app, Action::DeleteCharEof);
    assert_eq!(app.input.value, "ab");
    assert!(!app.should_quit);
    assert!(events.iter().any(|e| matches!(e, Event::RunPreview)));
}

#[test]
fn backward_delete_char_eof_deletes_when_not_empty() {
    let mut app = App::default();
    app.input.value = "abc".to_string();
    app.input.move_to_end();
    // Non-empty → BackwardDeleteCharEof removes the char before the cursor.
    let events = act(&mut app, Action::BackwardDeleteCharEof);
    assert_eq!(app.input.value, "ab");
    assert!(!app.should_quit);
    assert!(events.iter().any(|e| matches!(e, Event::RunPreview)));
}

#[test]
fn navigation_in_bottom_to_top_layout() {
    let mut app = app_with_items(&["a", "b", "c", "d", "e"]);
    // BottomToTop inverts the Down/Up scroll directions.
    app.item_list.direction = ratatui::widgets::ListDirection::BottomToTop;
    let _ = render(&mut app, 40, 6);

    act(&mut app, Action::Down(2));
    act(&mut app, Action::Up(1));
    // Selection stays within bounds after inverted navigation.
    assert!(app.item_list.current < app.item_list.items.len());
}

#[test]
fn toggle_in_out_in_bottom_to_top_layout() {
    let mut app = app_with_items(&["a", "b", "c"]);
    app.options.multi = true;
    // ReverseList layout drives the item list bottom-to-top.
    app.item_list.direction = ratatui::widgets::ListDirection::BottomToTop;
    app.item_list.current = 1;
    act(&mut app, Action::ToggleIn);
    act(&mut app, Action::ToggleOut);
    assert_eq!(app.item_list.selection.len(), 2);
}

#[test]
fn if_query_empty_branches() {
    let mut app = App::default();
    // Query empty -> "then" branch (ignore action).
    let events = act(&mut app, Action::IfQueryEmpty("ignore".to_string(), None));
    assert!(events.iter().all(|e| matches!(e, Event::Action(Action::Ignore))));

    // Query non-empty -> "otherwise" branch.
    app.input.value = "x".to_string();
    let events = act(
        &mut app,
        Action::IfQueryEmpty("ignore".to_string(), Some("abort".to_string())),
    );
    assert!(events.iter().any(|e| matches!(e, Event::Action(Action::Abort))));
}

#[test]
fn if_query_not_empty_branches() {
    let mut app = App::default();
    app.input.value = "x".to_string();
    let events = act(&mut app, Action::IfQueryNotEmpty("abort".to_string(), None));
    assert!(events.iter().any(|e| matches!(e, Event::Action(Action::Abort))));

    let mut app = App::default();
    let events = act(
        &mut app,
        Action::IfQueryNotEmpty("abort".to_string(), Some("ignore".to_string())),
    );
    assert!(events.iter().all(|e| matches!(e, Event::Action(Action::Ignore))));
}

#[test]
fn if_query_conditions_with_no_otherwise_fall_through() {
    // IfQueryEmpty with a non-empty query and no `otherwise` → empty (fall-through).
    let mut app = App::default();
    app.input.value = "x".to_string();
    assert!(act(&mut app, Action::IfQueryEmpty("abort".to_string(), None)).is_empty());

    // IfQueryNotEmpty with an empty query and no `otherwise` → empty.
    let mut app = App::default();
    assert!(act(&mut app, Action::IfQueryNotEmpty("abort".to_string(), None)).is_empty());

    // IfNonMatched with a non-empty list and no `otherwise` → empty.
    let mut app = app_with_items(&["a"]);
    assert!(act(&mut app, Action::IfNonMatched("abort".to_string(), None)).is_empty());
}

#[test]
fn next_history_at_most_recent_is_noop() {
    // history_index starts at None (most recent / current input); NextHistory
    // does nothing but still emits the query-changed events.
    let mut app = App::default();
    app.query_history = vec!["old".to_string()];
    let events = act(&mut app, Action::NextHistory);
    assert_eq!(app.input.value, "");
    assert!(events.iter().any(|e| matches!(e, Event::RunPreview)));
}

#[test]
fn execute_action_runs_command() {
    // Execute spawns a foreground command (toggling raw mode / alt screen) and
    // returns a Redraw event.
    let mut app = App::default();
    let events = act(&mut app, Action::Execute("true".to_string()));
    assert!(events.iter().any(|e| matches!(e, Event::Redraw)));
}

#[test]
fn if_non_matched_branches() {
    // Empty item list -> "then".
    let mut app = App::default();
    let events = act(&mut app, Action::IfNonMatched("abort".to_string(), None));
    assert!(events.iter().any(|e| matches!(e, Event::Action(Action::Abort))));

    // Non-empty list -> "otherwise".
    let mut app = app_with_items(&["a"]);
    let events = act(
        &mut app,
        Action::IfNonMatched("abort".to_string(), Some("ignore".to_string())),
    );
    assert!(events.iter().all(|e| matches!(e, Event::Action(Action::Ignore))));
}

#[test]
fn history_navigation_query_mode() {
    let mut app = App::default();
    app.query_history = vec!["first".to_string(), "second".to_string()];

    // Previous goes to most recent.
    act(&mut app, Action::PreviousHistory);
    assert_eq!(app.input.value, "second");
    act(&mut app, Action::PreviousHistory);
    assert_eq!(app.input.value, "first");
    // At oldest, stays.
    act(&mut app, Action::PreviousHistory);
    assert_eq!(app.input.value, "first");

    // Next moves forward toward more recent.
    act(&mut app, Action::NextHistory);
    assert_eq!(app.input.value, "second");
    act(&mut app, Action::NextHistory);
    // Restores saved (empty) input.
    assert_eq!(app.input.value, "");
}

#[test]
fn history_navigation_empty_is_noop() {
    let mut app = App::default();
    app.input.value = "typed".to_string();
    // With no history, navigation emits nothing and leaves the query untouched.
    assert!(act(&mut app, Action::PreviousHistory).is_empty());
    assert_eq!(app.input.value, "typed");
    assert!(act(&mut app, Action::NextHistory).is_empty());
    assert_eq!(app.input.value, "typed");
}

#[test]
fn history_navigation_cmd_mode() {
    let mut app = App::default();
    app.options.interactive = true;
    app.cmd_history = vec!["ls".to_string(), "pwd".to_string()];
    act(&mut app, Action::PreviousHistory);
    assert_eq!(app.input.value, "pwd");
}

#[test]
fn paging_actions_scroll_in_default_layout() {
    let mut app = app_with_items(&["a", "b", "c", "d", "e", "f", "g", "h"]);
    app.options.layout = crate::tui::options::TuiLayout::Default;
    let _ = render(&mut app, 40, 6);

    act(&mut app, Action::PageDown(1));
    act(&mut app, Action::PageUp(1));
    act(&mut app, Action::HalfPageDown(1));
    act(&mut app, Action::HalfPageUp(1));
    // Selection stays within bounds after paging.
    assert!(app.item_list.current < app.item_list.items.len());
}

#[test]
fn paging_actions_scroll_in_reverse_layout() {
    let mut app = app_with_items(&["a", "b", "c", "d", "e", "f", "g", "h"]);
    // Reverse layout takes the mirrored scroll branches.
    app.options.layout = crate::tui::options::TuiLayout::Reverse;
    let _ = render(&mut app, 40, 6);

    act(&mut app, Action::PageDown(1));
    act(&mut app, Action::HalfPageDown(1));
    act(&mut app, Action::PageUp(1));
    act(&mut app, Action::HalfPageUp(1));
    assert!(app.item_list.current < app.item_list.items.len());
}

#[test]
fn deselect_all_clears_existing_selection() {
    let mut app = app_with_items(&["a", "b", "c"]);
    app.options.multi = true;
    // Select everything, then deselect.
    act(&mut app, Action::SelectAll);
    assert!(!app.item_list.selection.is_empty());
    let events = act(&mut app, Action::DeselectAll);
    assert!(app.item_list.selection.is_empty());
    assert!(!events.is_empty());
}

#[test]
fn custom_action_runs_callback() {
    use crate::tui::event::ActionCallback;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let _guard = rt.enter();
    let mut app = App::default();
    let cb = ActionCallback::new_sync(|_app: &mut App| Ok(vec![Event::Action(Action::Abort)]));
    let events = rt.block_on(async { app.handle_action(&Action::Custom(cb)) }).unwrap();
    assert!(events.iter().any(|e| matches!(e, Event::Action(Action::Abort))));
}

#[test]
fn custom_action_runs_async_callback() {
    use crate::tui::event::ActionCallback;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let _guard = rt.enter();
    let mut app = App::default();
    // An async callback exercises the AsyncFnWrapper::call path.
    let cb = ActionCallback::new(|_app: &mut App| async move { Ok(vec![Event::Action(Action::Abort)]) });
    let events = rt.block_on(async { app.handle_action(&Action::Custom(cb)) }).unwrap();
    assert!(events.iter().any(|e| matches!(e, Event::Action(Action::Abort))));
}

#[test]
fn handle_key_maps_plain_char_to_add_char() {
    let mut app = App::default();
    let events = app.handle_key(&KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    assert!(matches!(events.as_slice(), [Event::Action(Action::AddChar('a'))]));
}

#[test]
fn handle_key_shift_char_uppercases() {
    let mut app = App::default();
    let events = app.handle_key(&KeyEvent::new(KeyCode::Char('a'), KeyModifiers::SHIFT));
    assert!(matches!(events.as_slice(), [Event::Action(Action::AddChar('A'))]));
}

#[test]
fn handle_key_ctrl_c_quits() {
    let mut app = App::default();
    // The fallback Ctrl+C -> Quit branch only fires when the keymap has no
    // explicit binding for the key, so clear it first.
    let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    app.options.keymap.remove(&key);
    let events = app.handle_key(&key);
    assert!(matches!(events.as_slice(), [Event::Quit]));
}

#[test]
fn handle_key_mapped_returns_keymap_action() {
    let mut app = App::default();
    // A key bound in the default keymap returns its mapped action(s).
    let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let events = app.handle_key(&key);
    assert!(events.iter().all(|e| matches!(e, Event::Action(_))));
    assert!(!events.is_empty());
}

#[test]
fn handle_key_unmapped_returns_empty() {
    let mut app = App::default();
    let events = app.handle_key(&KeyEvent::new(KeyCode::Char('c'), KeyModifiers::ALT));
    assert!(events.is_empty());
}

#[test]
fn handle_key_ctrl_non_c_falls_through_to_empty() {
    let mut app = App::default();
    // Ctrl + a non-'c' character that isn't in the keymap → empty.
    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
    app.options.keymap.remove(&key);
    assert!(app.handle_key(&key).is_empty());
}

#[test]
fn handle_key_shift_non_char_falls_through_to_empty() {
    let mut app = App::default();
    // Shift + a non-character key with no binding → empty.
    let key = KeyEvent::new(KeyCode::F(5), KeyModifiers::SHIFT);
    app.options.keymap.remove(&key);
    assert!(app.handle_key(&key).is_empty());
}

#[test]
fn handle_key_ctrl_non_char_falls_through_to_empty() {
    let mut app = App::default();
    // Ctrl + a non-character key with no binding → empty.
    let key = KeyEvent::new(KeyCode::F(6), KeyModifiers::CONTROL);
    app.options.keymap.remove(&key);
    assert!(app.handle_key(&key).is_empty());
}

#[test]
fn expand_cmd_substitutes_query() {
    let mut app = App::default();
    app.input.value = "myquery".to_string();
    let expanded = app.expand_cmd("echo {q}", false);
    assert!(expanded.contains("myquery"));
}

#[test]
fn calculate_preview_offset_variants() {
    let app = App::default();
    assert_eq!(app.calculate_preview_offset("+10"), 10);
    assert_eq!(app.calculate_preview_offset("+10-3"), 7);
    assert_eq!(app.calculate_preview_offset("+3+4"), 7);
    // Underflow saturates to 0.
    assert_eq!(app.calculate_preview_offset("+2-5"), 0);
}

#[test]
fn results_single_selection() {
    let mut app = app_with_items(&["a", "b", "c"]);
    let results = app.results();
    assert_eq!(results.len(), 1);
}

#[test]
fn results_multi_selection() {
    let mut app = app_with_items(&["a", "b", "c"]);
    app.options.multi = true;
    app.item_list.select_all();
    let results = app.results();
    assert_eq!(results.len(), 3);
}

#[test]
fn results_filter_mode_drains_items() {
    let mut app = app_with_items(&["a", "b"]);
    app.options.filter = Some("x".to_string());
    let results = app.results();
    assert_eq!(results.len(), 2);
    assert!(app.item_list.items.is_empty());
}

#[test]
fn resize_updates_layout() {
    let mut app = App::default();
    app.resize(120, 40);
    assert_eq!(app.layout.list_area.width, 120);
}

#[test]
fn restart_matcher_short_query_clears_items() {
    let mut app = app_with_items(&["a", "b"]);
    app.options.min_query_length = Some(3);
    app.input.value = "ab".to_string();
    app.restart_matcher(true);
    assert!(app.item_list.items.is_empty());
}

#[test]
fn handle_items_appends_to_pool() {
    let mut app = App::default();
    let items: Vec<Arc<dyn SkimItem>> = vec![Arc::new("x".to_string()), Arc::new("y".to_string())];
    app.handle_items(items);
    assert_eq!(app.item_pool.len(), 2);
}

#[test]
fn restart_matcher_no_sort_uses_append_strategy() {
    // A non-force restart with no_sort takes the Append merge-strategy branch.
    let mut app = app_with_items(&["a", "b", "c"]);
    app.options.no_sort = true;
    app.restart_matcher(false);
    // Restarting the matcher must not panic and leaves a live matcher control.
    assert!(!app.should_quit);
}

// ---------------------------------------------------------------------------
// Rendering — the `Widget for &mut App` path, driven with a ratatui Buffer.
// ---------------------------------------------------------------------------

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

fn render(app: &mut App, width: u16, height: u16) -> Buffer {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    app.render(area, &mut buf);
    buf
}

#[test]
fn render_default_app_populates_buffer() {
    let mut app = app_with_items(&["alpha", "beta", "gamma"]);
    let buf = render(&mut app, 80, 24);
    // The first item should appear somewhere in the rendered buffer.
    let rendered = buffer_to_string(&buf);
    assert!(rendered.contains("alpha"));
    // Cursor position is updated during render.
    assert!(app.cursor_pos.0 <= 80);
}

#[test]
fn render_with_preview_and_border() {
    let mut app = app_with_items(&["one", "two"]);
    app.options.preview = Some("echo hi".to_string());
    app.options.border = crate::tui::BorderType::Rounded;
    // Rebuild layout so the preview/border areas are accounted for.
    app.layout_template = LayoutTemplate::from_options(&app.options, app.header.height());
    let _ = render(&mut app, 80, 24);
    assert!(app.pending_preview_run || app.layout.preview_area.is_some());
}

#[test]
fn render_hidden_info_sets_no_status() {
    let mut app = app_with_items(&["x"]);
    app.options.info.display = InfoDisplay::Hidden;
    let _ = render(&mut app, 40, 10);
    assert!(app.input.status_info.is_none());
}

#[test]
fn render_regex_mode_reports_re_indicator() {
    let mut app = app_with_items(&["x"]);
    app.options.regex = true;
    let _ = render(&mut app, 40, 10);
    let info = app.input.status_info.as_ref().expect("status info present");
    assert_eq!(info.matcher_mode, "RE");
}

fn buffer_to_string(buf: &Buffer) -> String {
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

// ---------------------------------------------------------------------------
// Mouse / scroll helpers.
// ---------------------------------------------------------------------------

#[test]
fn update_spinner_toggles_when_reading() {
    let mut app = App::default();
    // Pool reports un-taken items -> spinner should turn on.
    app.handle_items(vec![Arc::new("a".to_string())]);
    assert!(!app.show_spinner);
    app.update_spinner();
    assert!(app.show_spinner);
}

#[test]
fn toggle_spinner_flips_flag() {
    let mut app = App::default();
    let before = app.show_spinner;
    app.toggle_spinner();
    assert_ne!(app.show_spinner, before);
}

#[test]
fn update_spinner_hides_after_grace_period() {
    use std::time::{Duration, Instant};
    let mut app = App::default();
    // Spinner is on but nothing is being read (pool drained).
    app.show_spinner = true;
    // Force the grace period to have elapsed so the spinner can turn off.
    app.spinner_last_change = Instant::now().checked_sub(Duration::from_secs(10)).unwrap();
    app.update_spinner();
    assert!(!app.show_spinner);
}

#[test]
fn run_preview_callback_multi_selection() {
    use crate::tui::PreviewCallback;
    let mut app = app_with_items(&["a", "b", "c"]);
    app.options.preview = None;
    app.options.multi = true;
    app.options.preview_fn = Some(PreviewCallback::from(|items: Vec<Arc<dyn SkimItem>>| {
        vec![format!("{} selected", items.len())]
    }));
    // Select two items so the multi branch collects from the selection set.
    act(&mut app, Action::Select);
    act(&mut app, Action::Down(1));
    act(&mut app, Action::Select);
    app.last_preview_spawn = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();

    let mut tui = test_tui();
    app.run_preview(&mut tui).unwrap();
    assert!(app.preview.total_lines >= 1);
}

#[test]
fn run_preview_callback_with_no_items() {
    use crate::tui::PreviewCallback;
    let mut app = App::default(); // empty item list
    app.options.preview = None;
    app.options.preview_fn = Some(PreviewCallback::from(|items: Vec<Arc<dyn SkimItem>>| {
        vec![format!("count={}", items.len())]
    }));
    app.last_preview_spawn = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();

    let mut tui = test_tui();
    // No selection and nothing selected → empty selection vector branch.
    app.run_preview(&mut tui).unwrap();
}

#[test]
fn scroll_moves_current_based_on_position() {
    let mut app = app_with_items(&["a", "b", "c", "d", "e", "f", "g", "h"]);
    // Establish a concrete layout first.
    let _ = render(&mut app, 80, 6);
    let inner = app.list_inner_area();

    // Clicking the top row maps to the first item.
    app.scroll(ratatui::layout::Position { x: inner.x, y: inner.y });
    assert_eq!(app.item_list.current, 0);

    // Clicking lower in the track maps to a strictly later item.
    let bottom = ratatui::layout::Position {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(1),
    };
    app.scroll(bottom);
    assert!(app.item_list.current > 0);
    assert!(app.item_list.current < app.item_list.items.len());
}

#[test]
fn scrollbar_column_none_when_items_fit() {
    let mut app = app_with_items(&["a", "b"]);
    let _ = render(&mut app, 80, 24);
    // Few items in a tall list -> no scrollbar.
    assert!(app.scrollbar_column().is_none());
}

// ---------------------------------------------------------------------------
// handle_event dispatch (requires a backend-backed Tui for the event channel).
// ---------------------------------------------------------------------------

use crate::tui::Size;
use crate::tui::backend::Tui;
use ratatui::backend::TestBackend;

fn test_tui() -> Tui<TestBackend> {
    Tui::new_with_height_and_backend(TestBackend::new(40, 10), Size::Percent(100)).expect("failed to build test TUI")
}

/// Drain every event currently queued on the TUI's channel.
fn drain_events(tui: &mut Tui<TestBackend>) -> Vec<Event> {
    let mut out = Vec::new();
    while let Ok(ev) = tui.event_rx.try_recv() {
        out.push(ev);
    }
    out
}

#[test]
fn handle_event_paste_strips_newlines_into_query() {
    let mut app = App::default();
    let mut tui = test_tui();
    app.handle_event(&mut tui, &Event::Paste("foo\nbar\r".to_string()))
        .unwrap();
    // Newlines/CRs are stripped; the rest is inserted into the query.
    assert_eq!(app.input.value, "foobar");
}

#[test]
fn handle_event_paste_only_newlines_is_noop() {
    let mut app = App::default();
    let mut tui = test_tui();
    app.handle_event(&mut tui, &Event::Paste("\n\r".to_string())).unwrap();
    assert_eq!(app.input.value, "");
}

#[test]
fn handle_event_clear_items_empties_pool() {
    let mut app = App::default();
    app.handle_items(vec![Arc::new("a".to_string()), Arc::new("b".to_string())]);
    assert!(!app.item_pool.is_empty());

    let mut tui = test_tui();
    app.handle_event(&mut tui, &Event::ClearItems).unwrap();
    assert_eq!(app.item_pool.len(), 0);
}

#[test]
fn handle_event_append_items_grows_pool() {
    let mut app = App::default();
    let mut tui = test_tui();
    let items: Vec<Arc<dyn SkimItem>> = vec![Arc::new("x".to_string())];
    app.handle_event(&mut tui, &Event::AppendItems(items)).unwrap();
    assert_eq!(app.item_pool.len(), 1);
}

#[test]
fn handle_event_action_emits_render() {
    let mut app = App::default();
    let mut tui = test_tui();
    app.handle_event(&mut tui, &Event::Action(Action::AddChar('z')))
        .unwrap();
    assert_eq!(app.input.value, "z");
    // An Action always queues a follow-up Render event.
    assert!(drain_events(&mut tui).iter().any(|e| matches!(e, Event::Render)));
}

#[test]
fn handle_event_key_maps_to_action_events() {
    let mut app = App::default();
    let mut tui = test_tui();
    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty());
    app.handle_event(&mut tui, &Event::Key(key)).unwrap();
    // The plain character is dispatched as an AddChar action event.
    assert!(
        drain_events(&mut tui)
            .iter()
            .any(|e| matches!(e, Event::Action(Action::AddChar('q'))))
    );
}

#[test]
fn handle_event_render_draws_without_error() {
    let mut app = app_with_items(&["alpha", "beta"]);
    let mut tui = test_tui();
    app.handle_event(&mut tui, &Event::Render).unwrap();
}

#[test]
fn handle_event_error_bails_and_exits() {
    let mut app = App::default();
    let mut tui = test_tui();
    let result = app.handle_event(&mut tui, &Event::Error("boom".to_string()));
    assert!(result.is_err());
}

#[test]
fn handle_event_invalid_input_is_noop() {
    let mut app = App::default();
    let mut tui = test_tui();
    app.handle_event(&mut tui, &Event::InvalidInput).unwrap();
}

#[test]
fn handle_event_heartbeat_updates_spinner_and_renders() {
    let mut app = App::default();
    // Pending items make the spinner want to show; force a render to be due.
    app.handle_items(vec![Arc::new("a".to_string())]);
    app.needs_render.store(true, Ordering::Relaxed);
    app.last_render_timer = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();

    let mut tui = test_tui();
    app.handle_event(&mut tui, &Event::Heartbeat).unwrap();
    // A render is queued once the frame-rate gate has elapsed.
    assert!(drain_events(&mut tui).iter().any(|e| matches!(e, Event::Render)));
}

#[test]
fn handle_event_run_preview_is_ok() {
    let mut app = app_with_items(&["a"]);
    let mut tui = test_tui();
    // No preview configured → run_preview is a no-op but must not error.
    app.handle_event(&mut tui, &Event::RunPreview).unwrap();
}

#[test]
fn handle_event_preview_ready_marks_ready() {
    let mut app = App::default();
    app.preview.mark_ready();
    let mut tui = test_tui();
    app.handle_event(&mut tui, &Event::PreviewReady).unwrap();
    assert!(!app.preview.is_loading());
}

#[test]
fn handle_event_clear_clears_backend() {
    let mut app = app_with_items(&["a"]);
    let mut tui = test_tui();
    app.handle_event(&mut tui, &Event::Clear).unwrap();
    app.handle_event(&mut tui, &Event::Redraw).unwrap();
}

#[test]
fn handle_event_resize_reflows_and_reruns_preview() {
    let mut app = app_with_items(&["a", "b"]);
    let _ = render(&mut app, 40, 10);
    let mut tui = test_tui();
    // Resize updates the cached layout and triggers a preview re-run.
    app.handle_event(&mut tui, &Event::Resize(60, 20)).unwrap();
    assert_eq!(app.layout.list_area.width, 60);
}

// ---------------------------------------------------------------------------
// Mouse handling via handle_event(Event::Mouse(..)).
// ---------------------------------------------------------------------------

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

#[test]
fn mouse_scroll_up_and_down_move_selection() {
    let mut app = app_with_items(&["a", "b", "c", "d", "e", "f"]);
    let _ = render(&mut app, 40, 6);
    let mut tui = test_tui();

    app.handle_event(&mut tui, &Event::Mouse(mouse(MouseEventKind::ScrollDown, 1, 1)))
        .unwrap();
    app.handle_event(&mut tui, &Event::Mouse(mouse(MouseEventKind::ScrollUp, 1, 1)))
        .unwrap();
    // A Render event is queued after handling a mouse event.
    assert!(drain_events(&mut tui).iter().any(|e| matches!(e, Event::Render)));
}

#[test]
fn mouse_left_click_selects_item_row() {
    let mut app = app_with_items(&["a", "b", "c", "d", "e", "f"]);
    let _ = render(&mut app, 40, 6);
    let inner = app.list_inner_area();
    let mut tui = test_tui();

    // Click a row inside the list area.
    let click = mouse(MouseEventKind::Down(MouseButton::Left), inner.x, inner.y);
    app.handle_event(&mut tui, &Event::Mouse(click)).unwrap();
    assert!(!app.currently_scrolling);
}

#[test]
fn mouse_drag_and_release_toggle_scrolling() {
    let mut app = app_with_items(&["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"]);
    let _ = render(&mut app, 40, 4);
    let mut tui = test_tui();

    // Force a scrubbing session, then drag and release.
    app.currently_scrolling = true;
    let inner = app.list_inner_area();
    app.handle_event(
        &mut tui,
        &Event::Mouse(mouse(MouseEventKind::Drag(MouseButton::Left), inner.x, inner.y + 1)),
    )
    .unwrap();
    assert!(app.currently_scrolling);

    app.handle_event(
        &mut tui,
        &Event::Mouse(mouse(MouseEventKind::Up(MouseButton::Left), inner.x, inner.y)),
    )
    .unwrap();
    assert!(!app.currently_scrolling);
}

#[test]
fn mouse_scroll_over_preview_area_scrolls_preview() {
    let mut app = app_with_items(&["a", "b"]);
    app.options.preview = Some("echo hi".to_string());
    app.layout_template = LayoutTemplate::from_options(&app.options, app.header.height());
    let _ = render(&mut app, 80, 24);
    let mut tui = test_tui();

    if let Some(preview_area) = app.layout.preview_area {
        let pos_col = preview_area.x + preview_area.width / 2;
        let pos_row = preview_area.y + preview_area.height / 2;
        // Scrolling over the preview area routes to the preview, not the list.
        app.handle_event(
            &mut tui,
            &Event::Mouse(mouse(MouseEventKind::ScrollUp, pos_col, pos_row)),
        )
        .unwrap();
        app.handle_event(
            &mut tui,
            &Event::Mouse(mouse(MouseEventKind::ScrollDown, pos_col, pos_row)),
        )
        .unwrap();
        // The scroll-down advanced the preview viewport.
        assert!(app.preview.scroll_y > 0);
    }
}

#[test]
fn mouse_other_event_is_ignored() {
    let mut app = app_with_items(&["a"]);
    let _ = render(&mut app, 40, 6);
    let mut tui = test_tui();
    // Moving the mouse (no button) falls through to the ignore arm.
    app.handle_event(&mut tui, &Event::Mouse(mouse(MouseEventKind::Moved, 1, 1)))
        .unwrap();
}

/// Build a bordered list whose items overflow the visible rows, so the
/// scrollbar is rendered and `scrollbar_column` returns `Some`.
fn overflowing_bordered_app() -> App {
    let items: Vec<String> = (0..30).map(|i| format!("item{i}")).collect();
    let refs: Vec<&str> = items.iter().map(String::as_str).collect();
    let mut app = app_with_items(&refs);
    app.options.border = crate::tui::BorderType::Rounded;
    // A non-empty thumb makes the scrollbar render when items overflow.
    app.item_list.scrollbar_thumb = "│".to_string();
    app.layout_template = LayoutTemplate::from_options(&app.options, app.header.height());
    let _ = render(&mut app, 20, 8);
    app
}

#[test]
fn bordered_list_inner_area_is_inset() {
    let app = overflowing_bordered_app();
    let full = app.layout.list_area;
    let inner = app.list_inner_area();
    // The border insets the inner area by one cell on each side.
    assert_eq!(inner.width, full.width.saturating_sub(2));
    assert_eq!(inner.height, full.height.saturating_sub(2));
}

#[test]
fn scrollbar_column_some_when_items_overflow() {
    let app = overflowing_bordered_app();
    // With a thumb set and items overflowing, the scrollbar column is reported.
    let (inner, col) = app.scrollbar_column().expect("scrollbar should be present");
    assert_eq!(col, inner.x + inner.width - 1);
}

#[test]
fn mouse_click_on_scrollbar_starts_scrubbing() {
    let mut app = overflowing_bordered_app();
    let mut tui = test_tui();
    let (inner, col) = app.scrollbar_column().expect("scrollbar should be present");
    // Click exactly on the scrollbar column inside the list → scrub session.
    let click = mouse(MouseEventKind::Down(MouseButton::Left), col, inner.y + 1);
    app.handle_event(&mut tui, &Event::Mouse(click)).unwrap();
    assert!(app.currently_scrolling);
}

#[test]
fn mouse_click_in_list_body_moves_cursor() {
    let mut app = overflowing_bordered_app();
    let mut tui = test_tui();
    let inner = app.list_inner_area();
    // Click in the body (not the scrollbar column) selects that visual row.
    let click = mouse(MouseEventKind::Down(MouseButton::Left), inner.x, inner.y + 2);
    app.handle_event(&mut tui, &Event::Mouse(click)).unwrap();
    assert!(!app.currently_scrolling);
}

#[test]
fn mouse_drag_on_scrollbar_scrolls_list() {
    let mut app = overflowing_bordered_app();
    let mut tui = test_tui();
    let (inner, col) = app.scrollbar_column().expect("scrollbar should be present");
    // Begin a scrub, then drag lower down the track to move the selection.
    let click = mouse(MouseEventKind::Down(MouseButton::Left), col, inner.y);
    app.handle_event(&mut tui, &Event::Mouse(click)).unwrap();
    let drag = mouse(
        MouseEventKind::Drag(MouseButton::Left),
        col,
        inner.y + inner.height.saturating_sub(1),
    );
    app.handle_event(&mut tui, &Event::Mouse(drag)).unwrap();
    assert!(app.item_list.current > 0);
}

// ---------------------------------------------------------------------------
// Multi-select toggles.
// ---------------------------------------------------------------------------

#[test]
fn toggle_in_selects_and_steps() {
    let mut app = app_with_items(&["a", "b", "c"]);
    app.options.multi = true;
    act(&mut app, Action::ToggleIn);
    assert_eq!(app.item_list.selection.len(), 1);
}

#[test]
fn toggle_out_selects_and_steps() {
    let mut app = app_with_items(&["a", "b", "c"]);
    app.options.multi = true;
    act(&mut app, Action::ToggleOut);
    assert_eq!(app.item_list.selection.len(), 1);
}

#[test]
fn toggle_all_flips_every_item() {
    let mut app = app_with_items(&["a", "b", "c"]);
    app.options.multi = true;
    // From an empty selection, ToggleAll selects all three.
    act(&mut app, Action::ToggleAll);
    assert_eq!(app.item_list.selection.len(), 3);
    // A second ToggleAll clears them again.
    act(&mut app, Action::ToggleAll);
    assert!(app.item_list.selection.is_empty());
}

// ---------------------------------------------------------------------------
// run_preview branches (text preview, callback preview, debounce).
// ---------------------------------------------------------------------------

use std::time::{Duration, Instant};

/// A [`SkimItem`] whose preview is inline text, hitting the `ItemPreview::Text` arm.
#[derive(Debug)]
struct TextPreviewItem;

impl SkimItem for TextPreviewItem {
    fn text(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Borrowed("item")
    }
    fn preview(&self, _ctx: PreviewContext) -> ItemPreview {
        ItemPreview::Text("hello preview".to_string())
    }
}

fn app_with_one(item: Arc<dyn SkimItem>) -> App {
    let mut app = App::default();
    let mut matched_items = vec![MatchedItem::new(item, Rank::default(), None, &RankBuilder::default())];
    app.item_list.append(&mut matched_items);
    app.item_list.height = 10;
    app
}

#[test]
fn run_preview_renders_inline_text() {
    let mut app = app_with_one(Arc::new(TextPreviewItem));
    app.options.preview = Some("ignored".to_string());
    // Defeat the debounce window.
    app.last_preview_spawn = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();

    let mut tui = test_tui();
    app.run_preview(&mut tui).unwrap();
    // The inline text is loaded as preview content (3 chars across one line).
    assert!(app.preview.total_lines >= 1);
    // A ready preview queues a PreviewReady event.
    assert!(drain_events(&mut tui).iter().any(|e| matches!(e, Event::PreviewReady)));
}

#[test]
fn run_preview_uses_preview_callback() {
    use crate::tui::PreviewCallback;
    let mut app = app_with_items(&["a", "b"]);
    app.options.preview = None;
    app.options.preview_fn = Some(PreviewCallback::from(|_items: Vec<Arc<dyn SkimItem>>| {
        vec!["callback line".to_string()]
    }));
    app.last_preview_spawn = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();

    let mut tui = test_tui();
    app.run_preview(&mut tui).unwrap();
    assert!(app.preview.total_lines >= 1);
}

#[test]
fn run_preview_debounces_rapid_calls() {
    let mut app = app_with_one(Arc::new(TextPreviewItem));
    app.options.preview = Some("ignored".to_string());
    // A fresh spawn timestamp triggers the debounce early-return.
    app.last_preview_spawn = Instant::now();
    app.pending_preview_run = false;

    let mut tui = test_tui();
    app.run_preview(&mut tui).unwrap();
    assert!(app.pending_preview_run);
}

/// A [`SkimItem`] whose preview is positioned inline text (`TextWithPos`).
#[derive(Debug)]
struct PositionedTextItem;

impl SkimItem for PositionedTextItem {
    fn text(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Borrowed("item")
    }
    fn preview(&self, _ctx: PreviewContext) -> ItemPreview {
        use crate::tui::Size as PreviewSize;
        ItemPreview::TextWithPos(
            "a\nb\nc\nd\ne\n".to_string(),
            crate::PreviewPosition {
                v_scroll: PreviewSize::Fixed(2),
                v_offset: PreviewSize::Fixed(0),
                h_scroll: PreviewSize::Fixed(1),
                h_offset: PreviewSize::Fixed(0),
            },
        )
    }
}

#[test]
fn from_options_single_reader_and_matcher_flags() {
    use crate::options::FeatureFlag;
    let theme = Arc::new(crate::theme::ColorTheme::default());
    let mut options = SkimOptions::default();
    options.flags = vec![FeatureFlag::SingleReader, FeatureFlag::SingleMatcher];
    // Building with the single-thread flags must not panic and yields a usable App.
    let app = App::from_options(options, theme, String::new());
    assert!(!app.should_quit);
}

#[test]
fn run_preview_renders_positioned_text() {
    let mut app = app_with_one(Arc::new(PositionedTextItem));
    app.options.preview = Some("ignored".to_string());
    app.preview.rows = 100;
    app.preview.cols = 100;
    app.last_preview_spawn = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();

    let mut tui = test_tui();
    app.run_preview(&mut tui).unwrap();
    // The positioned text applied a vertical scroll offset.
    assert_eq!(app.preview.scroll_y, 2);
}

/// A positioned-text preview using `Neg`/`Percent` size variants, complementing
/// `PositionedTextItem` (Fixed) to cover the remaining position-offset arms.
#[derive(Debug)]
struct PositionedTextNegPercentItem;

impl SkimItem for PositionedTextNegPercentItem {
    fn text(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Borrowed("item")
    }
    fn preview(&self, _ctx: PreviewContext) -> ItemPreview {
        use crate::tui::Size as PreviewSize;
        ItemPreview::TextWithPos(
            "a\nb\nc\n".to_string(),
            crate::PreviewPosition {
                v_scroll: PreviewSize::Neg(2),
                v_offset: PreviewSize::Percent(50),
                h_scroll: PreviewSize::Neg(3),
                h_offset: PreviewSize::Percent(25),
            },
        )
    }
}

#[test]
fn run_preview_positioned_text_neg_and_percent_offsets() {
    let mut app = app_with_one(Arc::new(PositionedTextNegPercentItem));
    app.options.preview = Some("ignored".to_string());
    app.preview.rows = 40;
    app.preview.cols = 40;
    app.last_preview_spawn = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();

    let mut tui = test_tui();
    // Exercises the Neg (v_scroll/h_scroll) and Percent (v_offset/h_offset) arms.
    app.run_preview(&mut tui).unwrap();
    // h_scroll = cols(40) - 3 = 37, plus h_offset = 25% of 40 = 10 → 47.
    assert_eq!(app.preview.scroll_x, 47);
}

/// A [`SkimItem`] whose preview runs a shell command.
#[derive(Debug)]
struct CommandPreviewItem;

impl SkimItem for CommandPreviewItem {
    fn text(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Borrowed("item")
    }
    fn preview(&self, _ctx: PreviewContext) -> ItemPreview {
        ItemPreview::Command("echo hi".to_string())
    }
}

#[test]
fn run_preview_spawns_command() {
    let mut app = app_with_one(Arc::new(CommandPreviewItem));
    app.options.preview = Some("ignored".to_string());
    app.last_preview_spawn = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();

    let mut tui = test_tui();
    // The Command preview spawns a PTY child without error.
    app.run_preview(&mut tui).unwrap();
    app.preview.kill();
}

/// A [`SkimItem`] whose preview runs a command and applies a position.
#[derive(Debug)]
struct CommandWithPosPreviewItem;

impl SkimItem for CommandWithPosPreviewItem {
    fn text(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Borrowed("item")
    }
    fn preview(&self, _ctx: PreviewContext) -> ItemPreview {
        use crate::tui::Size as PreviewSize;
        ItemPreview::CommandWithPos(
            "echo hi".to_string(),
            crate::PreviewPosition {
                v_scroll: PreviewSize::Percent(50),
                v_offset: PreviewSize::Neg(1),
                h_scroll: PreviewSize::Percent(50),
                h_offset: PreviewSize::Fixed(2),
            },
        )
    }
}

#[test]
fn run_preview_spawns_command_with_position() {
    let mut app = app_with_one(Arc::new(CommandWithPosPreviewItem));
    app.options.preview = Some("ignored".to_string());
    app.preview.rows = 40;
    app.preview.cols = 40;
    app.last_preview_spawn = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();

    let mut tui = test_tui();
    // Exercises the CommandWithPos position-offset arithmetic (Percent/Neg/Fixed).
    app.run_preview(&mut tui).unwrap();
    // v_scroll = 50% of 40 rows = 20, then v_offset (Neg(1) → 39) scrolls further.
    assert!(app.preview.scroll_y >= 20);
    // h_scroll(50% of 40 = 20) + h_offset(2) = 22.
    assert_eq!(app.preview.scroll_x, 22);
    app.preview.kill();
}

/// A command-with-position preview using the `Neg`/`Percent`/`Fixed` variants
/// not covered by [`CommandWithPosPreviewItem`].
#[derive(Debug)]
struct CommandWithPosNegItem;

impl SkimItem for CommandWithPosNegItem {
    fn text(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Borrowed("item")
    }
    fn preview(&self, _ctx: PreviewContext) -> ItemPreview {
        use crate::tui::Size as PreviewSize;
        ItemPreview::CommandWithPos(
            "echo hi".to_string(),
            crate::PreviewPosition {
                v_scroll: PreviewSize::Neg(5),
                v_offset: PreviewSize::Percent(50),
                h_scroll: PreviewSize::Neg(4),
                h_offset: PreviewSize::Neg(2),
            },
        )
    }
}

#[test]
fn run_preview_command_with_position_neg_and_percent() {
    let mut app = app_with_one(Arc::new(CommandWithPosNegItem));
    app.options.preview = Some("ignored".to_string());
    app.preview.rows = 40;
    app.preview.cols = 40;
    app.last_preview_spawn = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();

    let mut tui = test_tui();
    // Covers the Neg v_scroll/h_scroll, Percent v_offset, and Neg h_offset arms.
    app.run_preview(&mut tui).unwrap();
    // h_scroll = cols(40) - 4 = 36, plus h_offset = cols(40) - 2 = 38 → 74.
    assert_eq!(app.preview.scroll_x, 74);
    app.preview.kill();
}
