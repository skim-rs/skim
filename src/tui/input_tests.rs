use super::*;

fn status() -> StatusInfo {
    StatusInfo {
        total: 100,
        matched: 42,
        processed: 50,
        show_spinner: false,
        matcher_mode: String::new(),
        multi_selection: false,
        selected: 0,
        current_item_idx: 7,
        hscroll_offset: 3,
        start: None,
        inline_separator: " < ".to_string(),
    }
}

#[test]
fn left_title_without_spinner_has_padding() {
    let s = status();
    let title = s.left_title();
    // No spinner → two leading spaces, then matched/total.
    assert!(title.starts_with("  "));
    assert!(title.contains("42/100"));
}

#[test]
fn left_title_with_spinner_mode_progress_and_selection() {
    let mut s = status();
    s.show_spinner = true;
    s.start = Some(Instant::now());
    s.matcher_mode = "RE".to_string();
    s.multi_selection = true;
    s.selected = 4;
    let title = s.left_title();
    assert!(title.contains("42/100"));
    assert!(title.contains("/RE"));
    // processed != total → progress percentage shown.
    assert!(title.contains("(50%)"));
    assert!(title.contains("[4]"));
}

#[test]
fn inline_separator_switches_with_spinner() {
    let mut s = status();
    // Spinner off → raw separator.
    assert_eq!(s.inline_separator_or_spinner(), " < ");
    // Spinner on → a spinner glyph plus padding the width of the separator.
    s.show_spinner = true;
    s.start = Some(Instant::now());
    let out = s.inline_separator_or_spinner();
    assert_ne!(out, " < ");
    assert!(!out.is_empty());
}

#[test]
fn inline_status_includes_mode_progress_and_selection() {
    let mut s = status();
    s.show_spinner = true;
    s.matcher_mode = "RE".to_string();
    s.multi_selection = true;
    s.selected = 2;
    let out = s.inline_status();
    assert!(out.starts_with("42/100"));
    assert!(out.contains("/RE"));
    assert!(out.contains("(50%)"));
    assert!(out.contains("[2]"));
}

#[test]
fn right_title_shows_index_and_hscroll() {
    assert_eq!(status().right_title(), "7/3");
}

#[test]
fn input_word_navigation_and_deletion() {
    let mut input = Input::default();
    input.insert_str("foo bar baz");
    input.move_to_end();

    // Delete the trailing word into the returned string.
    let deleted = input.delete_backward_word();
    assert_eq!(deleted, "baz");
    assert_eq!(input.value, "foo bar ");

    // Move the cursor back over a word, then forward again.
    input.move_cursor_backward_word();
    input.move_cursor_forward_word();

    // delete_to_beginning empties everything before the cursor.
    input.move_to_end();
    let head = input.delete_to_beginning();
    assert_eq!(head, "foo bar ");
    assert_eq!(input.value, "");
}

#[test]
fn input_switch_mode_swaps_value_and_prompt() {
    let mut input = Input::default();
    input.insert_str("query");
    input.switch_mode();
    // After switching, the visible value is the (empty) alternate buffer.
    assert_eq!(input.value, "");
    input.switch_mode();
    assert_eq!(input.value, "query");
}

#[test]
fn forward_word_skips_leading_separators() {
    // Cursor before a run of non-word chars: forward-word skips them then the word.
    let mut input = Input::default();
    input.insert_str("  ..foo bar");
    input.move_cursor_to(0);
    input.move_cursor_forward_word();
    // Lands at the end of "foo".
    assert_eq!(input.value[..input.cursor_pos as usize].trim_start(), "..foo");
}

#[test]
fn backward_word_at_start_stays_at_zero() {
    let mut input = Input::default();
    input.insert_str("abc");
    input.move_cursor_to(0);
    input.move_cursor_backward_word();
    assert_eq!(input.cursor_pos, 0);
}

#[test]
fn delete_backward_word_at_start_is_empty() {
    let mut input = Input::default();
    input.insert_str("abc");
    input.move_cursor_to(0);
    assert_eq!(input.delete_backward_word(), "");
    assert_eq!(input.value, "abc");
}

#[test]
fn delete_backward_word_skips_trailing_punctuation() {
    // Cursor after punctuation: skip the non-word chars, then the word.
    let mut input = Input::default();
    input.insert_str("foo bar...");
    input.move_to_end();
    let deleted = input.delete_backward_word();
    // The "..." and "bar" are removed together.
    assert!(deleted.contains("bar"));
    assert_eq!(input.value, "foo ");
}

#[test]
fn delete_backward_to_whitespace_skips_trailing_spaces() {
    // Ctrl+W from after trailing whitespace removes the spaces and the word.
    let mut input = Input::default();
    input.insert_str("foo bar   ");
    input.move_to_end();
    let deleted = input.delete_backward_to_whitespace();
    assert!(deleted.contains("bar"));
    assert_eq!(input.value, "foo ");
}

#[test]
fn delete_backward_to_whitespace_at_start_is_empty() {
    let mut input = Input::default();
    input.insert_str("abc");
    input.move_cursor_to(0);
    assert_eq!(input.delete_backward_to_whitespace(), "");
}

#[test]
fn delete_forward_word_removes_next_word() {
    let mut input = Input::default();
    input.insert_str("foo bar");
    input.move_cursor_to(0);
    let deleted = input.delete_forward_word();
    assert!(deleted.contains("foo"));
}

#[test]
fn delete_forward_word_at_end_is_empty() {
    // Alt+D with the cursor already at the end of the buffer deletes nothing.
    let mut input = Input::default();
    input.insert_str("foo bar");
    input.move_to_end();
    assert_eq!(input.delete_forward_word(), "");
    assert_eq!(input.value, "foo bar");
}

#[test]
fn delete_forward_word_skips_leading_separators() {
    // Cursor sitting on a separator: forward-word delete skips the separator
    // run, then removes the following word (the leading non-word skip loop).
    let mut input = Input::default();
    input.insert_str("foo   bar");
    input.move_cursor_to(3); // on the first space of the run
    let deleted = input.delete_forward_word();
    assert_eq!(deleted, "   bar");
    assert_eq!(input.value, "foo");
}

#[test]
fn move_cursor_zero_offset_is_noop() {
    let mut input = Input::default();
    input.insert_str("abc");
    input.move_cursor_to(1);
    input.move_cursor(0);
    assert_eq!(input.cursor_pos, 1);
}

#[test]
fn delete_at_bounds_returns_none() {
    let mut input = Input::default();
    // Empty input → nothing to delete.
    assert!(input.delete(0).is_none());
    input.insert_str("ab");
    input.move_to_end();
    // Forward-delete at end → out of range → None.
    assert!(input.delete(0).is_none());
}

#[test]
fn input_render_writes_prompt_and_value() {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let mut input = Input::default();
    input.insert_str("hello");
    input.status_info = Some(status());
    let area = Rect::new(0, 0, 40, 3);
    let mut buf = Buffer::empty(area);
    input.render(area, &mut buf);
    let mut text = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            text.push_str(buf[(x, y)].symbol());
        }
    }
    assert!(text.contains("hello"));
}
