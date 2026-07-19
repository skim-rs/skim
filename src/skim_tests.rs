use super::*;
use ratatui::backend::TestBackend;
use std::time::{Duration, Instant};

/// Spin until `cond` holds or a short timeout elapses.
fn wait_until(mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while !cond() && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Build a `Skim<TestBackend>` pre-loaded with `items`, started, with its
/// reader drained — mirroring the snapshot test harness setup.
fn started_skim_with(options: SkimOptions, items: &[&str]) -> Skim<TestBackend> {
    let (tx, rx) = crate::prelude::unbounded();
    let batch: Vec<Arc<dyn SkimItem>> = items
        .iter()
        .map(|s| Arc::new(s.to_string()) as Arc<dyn SkimItem>)
        .collect();
    tx.send(batch).unwrap();
    drop(tx); // close the channel so the reader finishes

    let backend = TestBackend::new(40, 10);
    let tui = Tui::new_with_height_and_backend(backend, Size::Percent(100)).unwrap();
    let mut skim = Skim::<TestBackend>::init(options, Some(rx)).unwrap();
    skim.init_tui_with(tui);
    skim.start();
    skim
}

fn started_skim(items: &[&str]) -> Skim<TestBackend> {
    started_skim_with(SkimOptions::default().build(), items)
}

#[test]
fn accessors_expose_app_and_tui() {
    let mut skim = started_skim(&["alpha", "beta"]);

    // Immutable and mutable app/tui accessors.
    assert!(!skim.app().should_quit);
    skim.app_mut().input.value = "x".to_string();
    assert_eq!(skim.app().input.value, "x");

    // The TUI accessors and combined borrow do not panic once initialized.
    let _ = skim.tui_ref();
    let _ = skim.tui_mut();
    let (_app, _tui) = skim.app_and_tui();

    // The event sender is cloneable while the TUI is live.
    let _sender = skim.event_sender();

    // Final event defaults to Quit and skim has not been asked to quit yet.
    assert!(matches!(skim.final_event(), Event::Quit));
    assert!(!skim.should_quit());
}

#[test]
fn should_enter_is_false_in_filter_mode() {
    let mut options = SkimOptions::default();
    options.filter = Some(String::new());
    let options = options.build();
    let mut skim = started_skim_with(options, &["a", "b", "c"]);
    // Filter mode processes everything synchronously and never enters the TUI.
    assert!(!skim.should_enter());
    assert_eq!(skim.app().item_list.items.len(), 3);
}

#[test]
fn should_enter_is_false_for_select_1_single_match() {
    let mut options = SkimOptions::default();
    options.select_1 = true;
    let options = options.build();
    // A single matching item satisfies select-1, so skim exits early.
    let mut skim = started_skim_with(options, &["only"]);
    assert!(!skim.should_enter());
}

#[test]
fn should_enter_is_true_in_sync_mode_with_matches() {
    let mut options = SkimOptions::default();
    options.sync = true;
    let options = options.build();
    // Sync mode waits for all items, then enters the TUI to display them.
    let mut skim = started_skim_with(options, &["a", "b"]);
    assert!(skim.should_enter());
}

#[test]
fn should_enter_is_true_for_exit_0_with_matches() {
    let mut options = SkimOptions::default();
    options.exit_0 = true;
    let options = options.build();
    // exit-0 only bails when nothing matches; here items match so we enter.
    let mut skim = started_skim_with(options, &["a", "b"]);
    assert!(skim.should_enter());
}

#[test]
fn output_collects_results_and_marks_abort() {
    let mut skim = started_skim(&["a", "b"]);
    wait_until(|| skim.check_reader());
    wait_until(|| skim.matcher_stopped());
    let output = skim.output();
    // The default final_event (Quit) is treated as an abort.
    assert!(output.is_abort);
    // Non-interactive, no cmd_query → the command is the initial command.
    assert_eq!(output.cmd, "");
}

#[test]
fn nested_accept_actions_are_reported_as_accepts() {
    for binding in ["start:first,first:accept", "start:if-query-empty(accept)"] {
        let mut options = SkimOptions::default();
        options.bind = vec![binding.to_string()];
        let mut skim = started_skim_with(options.build(), &["a"]);

        tokio::runtime::Runtime::new().unwrap().block_on(skim.run()).unwrap();

        assert!(matches!(skim.final_event(), Event::Action(Action::Accept(None))));
        assert!(!skim.output().is_abort, "binding `{binding}` was reported as an abort");
    }
}

#[test]
fn output_uses_input_as_cmd_in_interactive_mode() {
    let mut options = SkimOptions::default();
    options.interactive = true;
    let options = options.build();
    let mut skim = started_skim_with(options, &["a"]);
    skim.app_mut().input.value = "typed".to_string();
    wait_until(|| skim.check_reader());
    let output = skim.output();
    assert_eq!(output.cmd, "typed");
    assert_eq!(output.query, "typed");
}

#[test]
fn output_uses_cmd_query_when_set() {
    let mut options = SkimOptions::default();
    options.cmd_query = Some("preset".to_string());
    let options = options.build();
    let mut skim = started_skim_with(options, &["a"]);
    wait_until(|| skim.check_reader());
    let output = skim.output();
    assert_eq!(output.cmd, "preset");
}

#[test]
fn multi_selection_flows_into_output_and_serializes() {
    // multi-select -> SkimOutput -> CLI serialization. The matcher only fills the
    // visible list on render, so we populate it directly and exercise selection +
    // output + serialization. (The reader -> matcher half is covered by
    // `reader_and_matcher_complete`.)
    use crate::Rank;
    use crate::item::{MatchedItem, RankBuilder};

    let mut options = SkimOptions::default();
    options.multi = true;
    let options = options.build();
    let mut skim = started_skim_with(options, &["a", "b", "c"]);
    wait_until(|| skim.check_reader());
    wait_until(|| skim.matcher_stopped());

    let rb = RankBuilder::default();
    let mk = |t: &str| MatchedItem::new(Arc::new(t.to_string()) as Arc<dyn SkimItem>, Rank::default(), None, &rb);
    skim.app_mut().item_list.append(&mut vec![mk("a"), mk("b"), mk("c")]);
    // Toggle "a" and "c" into the selection, as BTab would.
    skim.app_mut().item_list.toggle_at(0);
    skim.app_mut().item_list.toggle_at(2);

    let output = skim.output();
    let texts: Vec<String> = output.selected_items.iter().map(|i| i.output().into_owned()).collect();
    assert_eq!(texts, vec!["a", "c"]);

    // --print0 serialization of the multi-selection.
    let mut print0 = SkimOptions::default();
    print0.print0 = true;
    let bin = crate::BinOptions::from_opts(&print0);
    let mut buf = Vec::new();
    output.write_output(&mut buf, &bin).unwrap();
    assert_eq!(String::from_utf8(buf).unwrap(), "a\0c\0");
}

#[test]
fn reader_and_matcher_complete() {
    let mut skim = started_skim(&["one", "two", "three"]);

    // The reader drains and reports done via check_reader.
    wait_until(|| skim.check_reader());
    assert!(skim.reader_done());

    // After the reader finishes, the matcher eventually stops.
    wait_until(|| skim.matcher_stopped());
    assert!(skim.matcher_stopped());
}

#[test]
fn try_flush_render_emits_render_when_due() {
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};
    let mut skim = started_skim(&["a"]);

    // Mark a render as needed and age the frame-rate gate so it is due.
    skim.app.needs_render.store(true, Ordering::Relaxed);
    let now = Instant::now();
    skim.app.last_render_timer = now.checked_sub(Duration::from_secs(1)).unwrap_or(now);
    skim.try_flush_render();

    // The render flag is cleared and a Render event is queued on the TUI.
    assert!(!skim.app.needs_render.load(Ordering::Relaxed));
    let mut saw_render = false;
    while let Ok(ev) = skim.tui.as_mut().unwrap().event_rx.try_recv() {
        if matches!(ev, Event::Render) {
            saw_render = true;
        }
    }
    assert!(saw_render);
}

#[test]
fn try_flush_render_noop_when_not_needed() {
    use std::sync::atomic::Ordering;
    let mut skim = started_skim(&["a"]);
    // No render requested → nothing happens, no panic.
    skim.app.needs_render.store(false, Ordering::Relaxed);
    skim.try_flush_render();
    assert!(!skim.app.needs_render.load(Ordering::Relaxed));
}
