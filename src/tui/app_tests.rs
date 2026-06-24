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
