use super::*;

fn separator_buffer(display: &InfoDisplay, reverse: bool, border: BorderType, pattern: &str, width: u16) -> Buffer {
    let options = crate::options::SkimOptionsBuilder::default()
        .separator(pattern)
        .info(Info {
            display: display.clone(),
            separator: None,
        })
        .layout(if reverse {
            TuiLayout::Reverse
        } else {
            TuiLayout::Default
        })
        .border(border)
        .build()
        .unwrap();
    let mut input = Input::from_options(&options, Arc::new(ColorTheme::default()));
    input.status_info = Some(status());
    let height = if border.is_some() {
        3
    } else {
        1 + u16::from(*display != InfoDisplay::Inline)
    };
    let area = Rect::new(2, 3, width, height);
    let mut buf = Buffer::empty(area);
    input.render(area, &mut buf);
    buf
}

fn separator_row(buf: &Buffer, y: u16) -> String {
    (buf.area.x..buf.area.right()).map(|x| buf[(x, y)].symbol()).collect()
}

#[test]
fn separator_shares_status_row_in_both_directions() {
    for reverse in [false, true] {
        let buf = separator_buffer(&InfoDisplay::Default, reverse, BorderType::None, "─", 24);
        let y = if reverse { buf.area.bottom() - 1 } else { buf.area.y };
        assert_eq!(separator_row(&buf, y), "  42/100 ─────────── 7/3");
        assert_eq!(buf[(11, y)].fg, ColorTheme::default().separator.fg.unwrap());
        let prompt_y = if reverse { buf.area.y } else { buf.area.bottom() - 1 };
        assert!(separator_row(&buf, prompt_y).contains('>'));
    }
}

#[test]
fn separator_respects_packed_status_alignment() {
    let left = separator_buffer(&InfoDisplay::Left, false, BorderType::None, "-", 24);
    assert_eq!(separator_row(&left, left.area.y), "  42/100  7/3 ----------");
    let right = separator_buffer(&InfoDisplay::Right, false, BorderType::None, "-", 24);
    assert_eq!(separator_row(&right, right.area.y), "----------   42/100  7/3");
}

#[test]
fn separator_follows_inline_status_on_prompt_row() {
    let buf = separator_buffer(&InfoDisplay::Inline, false, BorderType::None, "─", 40);
    let row = separator_row(&buf, buf.area.y);
    assert!(row.contains("> "));
    assert!(row.contains("42/100"));
    assert!(row.contains(" ─"));
    assert!(row.ends_with("7/3"));
    assert_eq!(buf.area.height, 1);
}

#[test]
fn separator_gets_own_row_for_inline_right_and_hidden_info() {
    for display in [InfoDisplay::InlineRight, InfoDisplay::Hidden] {
        for reverse in [false, true] {
            let buf = separator_buffer(&display, reverse, BorderType::None, "─", 24);
            let y = if reverse { buf.area.bottom() - 1 } else { buf.area.y };
            assert_eq!(separator_row(&buf, y), "─".repeat(24));
            let prompt_y = if reverse { buf.area.y } else { buf.area.bottom() - 1 };
            assert!(separator_row(&buf, prompt_y).contains('>'));
        }
    }
}

#[test]
fn separator_reuses_facing_border_without_an_extra_row() {
    for reverse in [false, true] {
        for display in [InfoDisplay::Default, InfoDisplay::InlineRight, InfoDisplay::Hidden] {
            let buf = separator_buffer(&display, reverse, BorderType::Plain, "=", 24);
            let y = if reverse { buf.area.bottom() - 1 } else { buf.area.y };
            let row = separator_row(&buf, y);
            assert!(row.contains("==="));
            assert!(row.starts_with(if reverse { '└' } else { '┌' }));
            assert!(row.ends_with(if reverse { '┘' } else { '┐' }));
            assert_eq!(buf.area.height, 3);
        }
    }
}

#[test]
fn separator_repeats_strings_and_clips_wide_unicode() {
    for (pattern, expected) in [
        ("ab", "ababa"),
        ("界", "界 界  "),
        ("e\u{301}", "e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}"),
    ] {
        let buf = separator_buffer(&InfoDisplay::Hidden, true, BorderType::None, pattern, 5);
        assert_eq!(separator_row(&buf, buf.area.bottom() - 1), expected);
    }
}

#[test]
fn separator_adapts_to_resizing_without_rebuilding_input() {
    let options = crate::options::SkimOptionsBuilder::default()
        .separator("─")
        .info(Info {
            display: InfoDisplay::Hidden,
            separator: None,
        })
        .build()
        .unwrap();
    let mut input = Input::from_options(&options, Arc::new(ColorTheme::default()));
    for width in [40, 8, 60] {
        let area = Rect::new(0, 0, width, 2);
        let mut buf = Buffer::empty(area);
        input.render(area, &mut buf);
        assert_eq!(separator_row(&buf, 0), "─".repeat(usize::from(width)));
        assert!(separator_row(&buf, 1).contains('>'));
    }
}

#[test]
fn separator_handles_empty_zero_width_and_small_areas() {
    for pattern in ["", "\u{301}", "─"] {
        for display in [
            InfoDisplay::Default,
            InfoDisplay::Inline,
            InfoDisplay::InlineRight,
            InfoDisplay::Hidden,
        ] {
            for width in 0..4 {
                let _ = separator_buffer(&display, false, BorderType::None, pattern, width);
            }
        }
    }
}

#[test]
fn separator_color_does_not_change_status_color() {
    let options = crate::options::SkimOptionsBuilder::default()
        .separator("─")
        .border(BorderType::Plain)
        .color("separator:#666666,info:208,border:123:bold")
        .build()
        .unwrap();
    let mut input = Input::from_options(&options, Arc::new(ColorTheme::init_from_options(&options)));
    input.status_info = Some(status());
    let area = Rect::new(0, 0, 24, 3);
    let mut buf = Buffer::empty(area);
    input.render(area, &mut buf);
    assert_eq!(buf[(10, 0)].fg, Color::Rgb(102, 102, 102));
    assert!(!buf[(10, 0)].modifier.contains(Modifier::BOLD));
    assert_eq!(buf[(3, 0)].fg, Color::Indexed(208));
}

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
    assert_ne!(out, "");
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

#[test]
fn insert_str_leaves_cursor_at_end_for_multibyte() {
    // `cursor_pos` is a byte offset, so advancing it by char count left the cursor
    // inside the inserted text whenever a character was wider than one byte.
    let mut input = Input::default();
    input.insert_str("中文");
    assert_eq!(input.cursor_pos as usize, "中文".len());
}

#[test]
fn consecutive_insert_str_preserves_order_for_multibyte() {
    // Bracketed paste and IMEs deliver whole strings, so a wrong cursor position
    // made the next chunk land in the middle of the previous one.
    let mut input = Input::default();
    input.insert_str("中文");
    input.insert_str("测试");
    assert_eq!(input.value, "中文测试");
}

#[test]
fn insert_str_matches_repeated_insert_for_multibyte() {
    let mut by_str = Input::default();
    by_str.insert_str("中文");

    let mut by_char = Input::default();
    for c in "中文".chars() {
        by_char.insert(c);
    }

    assert_eq!(by_str.value, by_char.value);
    assert_eq!(by_str.cursor_pos, by_char.cursor_pos);
}
