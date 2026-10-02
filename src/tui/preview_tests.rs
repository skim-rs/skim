#[cfg(feature = "image")]
use image::{DynamicImage, RgbaImage};
#[cfg(feature = "image")]
use ratatui::layout::Size;
#[cfg(feature = "image")]
use ratatui_image::picker::Picker;

use super::{PREVIEW_MAX_BYTES, Preview, PreviewContent, read_bounded, update_plain_content};

#[cfg(feature = "image")]
fn image(width: u32, height: u32) -> DynamicImage {
    DynamicImage::ImageRgba8(RgbaImage::new(width, height))
}

#[cfg(feature = "image")]
#[test]
fn image_protocol_constrains_by_width() {
    let protocol = Preview::image_protocol(Some(&Picker::halfblocks()), image(400, 200), Size::new(20, 10))
        .expect("halfblocks protocol should be created");

    assert_eq!(protocol.size(), Size::new(20, 5));
}

#[cfg(feature = "image")]
#[test]
fn image_protocol_constrains_by_height() {
    let protocol = Preview::image_protocol(Some(&Picker::halfblocks()), image(200, 400), Size::new(20, 10))
        .expect("halfblocks protocol should be created");

    assert_eq!(protocol.size(), Size::new(10, 10));
}

#[cfg(feature = "image")]
#[test]
fn image_protocol_keeps_at_least_one_cell() {
    let protocol = Preview::image_protocol(Some(&Picker::halfblocks()), image(1000, 1), Size::new(1, 1))
        .expect("halfblocks protocol should be created");

    assert_eq!(protocol.size(), Size::new(1, 1));
}

#[cfg(feature = "image")]
#[test]
fn image_protocol_uses_halfblocks_picker_when_none_is_provided() {
    let protocol = Preview::image_protocol(None, image(400, 200), Size::new(20, 10))
        .expect("fallback halfblocks protocol should be created");

    assert_eq!(protocol.size(), Size::new(20, 5));
}

#[test]
fn content_loads_text_and_resets_scroll() {
    let mut p = Preview::default();
    p.scroll_x = 5;
    p.scroll_y = 5;
    p.content(b"line1\nline2\nline3\n").unwrap();
    assert_eq!(p.total_lines, 3);
    assert_eq!(p.scroll_x, 0);
    assert_eq!(p.scroll_y, 0);
    assert!(!p.is_loading());
}

#[test]
fn large_text_content_does_not_overflow_line_count() {
    let input = "x\n".repeat(70_000);
    let mut preview = Preview::default();
    preview.content(input.as_bytes()).unwrap();
    assert_eq!(preview.total_lines, 70_000);

    let content = preview.content.read().unwrap();
    let PreviewContent::Text(text) = &*content else {
        panic!("expected text preview");
    };
    let area = ratatui::layout::Rect::new(0, 0, 20, 5);
    let mut buffer = ratatui::buffer::Buffer::empty(area);
    assert_eq!(
        preview.render_text(ratatui::widgets::Block::new(), area, &mut buffer, text),
        70_000
    );
}

#[test]
fn bounded_reader_discards_output_after_limit() {
    let input = vec![b'x'; PREVIEW_MAX_BYTES + 4096];
    let output = read_bounded(std::io::Cursor::new(input));
    assert_eq!(output.len(), PREVIEW_MAX_BYTES);
}

fn preview_contains(preview: &Preview, expected: &str) -> bool {
    preview.content.read().is_ok_and(|content| match &*content {
        PreviewContent::Text(text) => text
            .lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content.as_ref().contains(expected))),
        _ => false,
    })
}

#[cfg(unix)]
#[test]
fn plain_preview_streams_before_command_exits() {
    use std::time::{Duration, Instant};

    use ratatui::backend::TestBackend;

    let mut preview = Preview::default();
    preview.pty = None;
    let mut tui =
        super::super::Tui::new_with_height_and_backend(TestBackend::new(20, 5), super::super::Size::Percent(100))
            .unwrap();
    preview.spawn(&mut tui, "printf streamed; sleep 30").unwrap();

    let started = Instant::now();
    let streamed_in_time = loop {
        if preview_contains(&preview, "streamed") {
            break true;
        }
        if started.elapsed() >= Duration::from_secs(2) {
            break false;
        }
        std::thread::sleep(Duration::from_millis(10));
    };

    preview.kill();
    preview.thread_handle.take().unwrap().join().unwrap();
    assert!(streamed_in_time, "preview output did not stream");
}

#[cfg(unix)]
#[test]
fn stale_plain_preview_cannot_replace_newer_streamed_output() {
    use std::time::{Duration, Instant};

    use ratatui::backend::TestBackend;

    let mut preview = Preview::default();
    preview.pty = None;
    let mut tui =
        super::super::Tui::new_with_height_and_backend(TestBackend::new(20, 5), super::super::Size::Percent(100))
            .unwrap();

    preview.spawn(&mut tui, "printf stale; sleep 30").unwrap();
    let stale_cancelled = preview.plain_cancelled.as_ref().unwrap().clone();
    let stale_thread = preview.thread_handle.take().unwrap();
    preview.spawn(&mut tui, "printf current; sleep 30").unwrap();

    let started = Instant::now();
    let current_streamed = loop {
        if preview_contains(&preview, "current") {
            break true;
        }
        if started.elapsed() >= Duration::from_secs(2) {
            break false;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    update_plain_content(&preview.content, &stale_cancelled, b"stale");
    let stale_write_was_ignored = preview_contains(&preview, "current") && !preview_contains(&preview, "stale");

    preview.kill();
    preview.thread_handle.take().unwrap().join().unwrap();
    stale_thread.join().unwrap();
    assert!(current_streamed, "new preview output did not stream");
    assert!(stale_write_was_ignored, "stale preview replaced newer output");
}

#[cfg(unix)]
#[test]
fn plain_preview_can_be_cancelled() {
    use std::time::{Duration, Instant};

    use ratatui::backend::TestBackend;

    let mut preview = Preview::default();
    preview.pty = None;
    let mut tui =
        super::super::Tui::new_with_height_and_backend(TestBackend::new(20, 5), super::super::Size::Percent(100))
            .unwrap();
    preview.spawn(&mut tui, "sleep 30").unwrap();

    let started = Instant::now();
    preview.kill();
    preview.thread_handle.take().unwrap().join().unwrap();
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn vertical_scroll_clamps_to_content() {
    let mut p = Preview::default();
    p.rows = 3;
    p.content(b"a\nb\nc\nd\ne\nf\n").unwrap();
    assert_eq!(p.total_lines, 6);
    p.scroll_down(100);
    // Cannot scroll past total_lines - (rows - 1) == 6 - 2 == 4.
    assert_eq!(p.scroll_y, 4);
    p.scroll_up(100);
    assert_eq!(p.scroll_y, 0);
}

#[test]
fn scroll_down_without_known_total_lines() {
    let mut p = Preview::default();
    p.total_lines = 0;
    p.scroll_down(5);
    assert_eq!(p.scroll_y, 5);
}

#[test]
fn horizontal_scroll() {
    let mut p = Preview::default();
    p.scroll_right(4);
    assert_eq!(p.scroll_x, 4);
    p.scroll_left(1);
    assert_eq!(p.scroll_x, 3);
    p.scroll_left(100);
    assert_eq!(p.scroll_x, 0);
}

#[test]
fn set_offset_is_one_indexed() {
    let mut p = Preview::default();
    p.set_offset(10);
    assert_eq!(p.scroll_y, 9);
    p.set_offset(0);
    assert_eq!(p.scroll_y, 0);
}

#[test]
fn page_up_and_down() {
    let mut p = Preview::default();
    p.rows = 10;
    p.content(b"x\n".repeat(50).as_slice()).unwrap();
    p.page_down();
    let after_down = p.scroll_y;
    assert!(after_down > 0);
    p.page_up();
    assert!(p.scroll_y < after_down);
}

#[test]
fn mark_ready_clears_loading() {
    let mut p = Preview::default();
    p.mark_ready();
    assert!(!p.is_loading());
}

#[test]
fn content_with_position_applies_offsets() {
    use crate::PreviewPosition;
    use crate::tui::Size as PreviewSize;
    let mut p = Preview::default();
    p.rows = 100;
    p.cols = 100;
    p.content(b"x\n".repeat(50).as_slice()).unwrap();
    let position = PreviewPosition {
        v_scroll: PreviewSize::Fixed(5),
        v_offset: PreviewSize::Fixed(2),
        h_scroll: PreviewSize::Fixed(3),
        h_offset: PreviewSize::Fixed(1),
    };
    p.content_with_position(b"x\n".repeat(50).as_slice(), position).unwrap();
    assert_eq!(p.scroll_y, 7);
    assert_eq!(p.scroll_x, 4);
}

#[test]
fn filter_and_respond_strips_query_sequences() {
    let mut writer: Box<dyn std::io::Write + Send> = Box::new(Vec::new());
    // A device-attributes query embedded in normal text.
    let data = b"abc\x1b[cdef";
    let filtered = Preview::filter_and_respond_to_queries(data, &mut writer);
    let text = String::from_utf8_lossy(&filtered);
    // The CSI query is consumed; surrounding text is preserved.
    assert!(text.contains("abc"));
    assert!(text.contains("def"));
    assert!(!text.contains('\x1b'));
}

#[test]
fn streaming_callback_publishes_before_completion_and_cancels() {
    use std::io::{ErrorKind, Write};
    use std::sync::{Mutex, mpsc};
    use std::time::{Duration, Instant};

    use ratatui::backend::TestBackend;
    use ratatui::style::Color;

    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Mutex::new(release_rx);
    let (result_tx, result_rx) = mpsc::channel();
    let main_thread = std::thread::current().id();
    let callback = super::PreviewCallback::streaming(move |_, _, mut writer| {
        assert_ne!(std::thread::current().id(), main_thread);
        writer.write_all(b"\x1b[31mfirst\x1b[0m").unwrap();
        writer.write_all(b"\nsecond").unwrap();
        release_rx.lock().unwrap().recv().unwrap();
        result_tx.send(writer.write_all(b"late").unwrap_err().kind()).unwrap();
    });
    let mut tui =
        super::super::Tui::new_with_height_and_backend(TestBackend::new(20, 5), super::super::Size::Percent(100))
            .unwrap();
    let mut preview = Preview::default();
    preview.spawn_callback(&mut tui, &callback, Vec::new(), None);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let PreviewContent::Text(text) = &*preview.content.read().unwrap()
            && text.lines.len() == 2
            && text.lines[0].to_string() == "first"
            && text.lines[1].to_string() == "second"
        {
            assert_eq!(text.lines[0].spans[0].style.fg, Some(Color::Red));
            break;
        }
        assert!(Instant::now() < deadline, "no incremental preview output");
        std::thread::yield_now();
    }
    assert!(preview.is_loading());
    preview.kill();
    preview.thread_handle.take().unwrap().join().unwrap();
    release_tx.send(()).unwrap();
    assert_eq!(
        result_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        ErrorKind::BrokenPipe
    );
    assert!(tui.event_rx.try_recv().is_err());
}

#[test]
fn streaming_callback_supports_buffering_and_legacy_calls() {
    use std::io::{BufWriter, Write};

    let callback = super::PreviewCallback::streaming(|current, _, writer| {
        assert!(current.is_none());
        let mut writer = BufWriter::new(writer);
        writer.write_all("hello\n世界".as_bytes()).unwrap();
        writer.flush().unwrap();
    });
    assert_eq!(callback(Vec::new()).join("\n"), "hello\n世界");
    let legacy = super::PreviewCallback::from(|_| vec!["a".to_string(), "b".to_string()]);
    assert_eq!(legacy(Vec::new()), vec!["a", "b"]);
}

#[test]
fn size_to_offset_resolves_each_variant() {
    let mut p = Preview::default();
    p.rows = 50;
    p.cols = 80;

    // Fixed maps straight through.
    assert_eq!(p.size_to_offset(super::super::Size::Fixed(7), true), 7);
    // Percent is relative to the matching dimension.
    assert_eq!(p.size_to_offset(super::super::Size::Percent(50), true), 25);
    assert_eq!(p.size_to_offset(super::super::Size::Percent(50), false), 40);
    // Neg subtracts from the matching dimension.
    assert_eq!(p.size_to_offset(super::super::Size::Neg(10), true), 40);
    assert_eq!(p.size_to_offset(super::super::Size::Neg(10), false), 70);
}

#[cfg(feature = "image")]
#[test]
fn set_image_picker_sets_and_clears() {
    let mut p = Preview::default();
    assert!(p.image_picker.is_none());
    p.set_image_picker(Some(Picker::halfblocks()));
    assert!(p.image_picker.is_some());
    p.set_image_picker(None);
    assert!(p.image_picker.is_none());
}
