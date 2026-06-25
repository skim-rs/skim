use super::*;
use std::io::Cursor;

/// Drain a receiver into the collected item texts.
///
/// Takes the receiver by value so it is dropped at the end, closing the
/// channel.
#[allow(clippy::needless_pass_by_value)]
fn drain(rx: SkimItemReceiver) -> Vec<String> {
    let mut out = Vec::new();
    while let Ok(batch) = rx.recv() {
        for item in batch {
            out.push(item.text().into_owned());
        }
    }
    out
}

#[test]
fn of_bufread_disables_items_matching_disable_pattern() {
    // `--disable-pattern` flags items whose text matches the regex; the reader
    // sets `disabled()` on them (they later become unselectable).
    let mut opts = crate::SkimOptions::default();
    opts.disable_pattern = Some(regex::Regex::new("foo").unwrap());
    let reader = SkimItemReader::new(SkimItemReaderOption::from_options(&opts));
    let rx = reader.of_bufread(Cursor::new("foo\nbar\n"));

    let mut disabled_by_text = std::collections::HashMap::new();
    while let Ok(batch) = rx.recv() {
        for item in batch {
            disabled_by_text.insert(item.text().into_owned(), item.disabled());
        }
    }
    assert_eq!(disabled_by_text.get("foo"), Some(&true));
    assert_eq!(disabled_by_text.get("bar"), Some(&false));
}

#[test]
fn of_bufread_reads_newline_separated_items() {
    let reader = SkimItemReader::default();
    let rx = reader.of_bufread(Cursor::new(b"a\nb\nc\n".to_vec()));
    assert_eq!(drain(rx), vec!["a", "b", "c"]);
}

#[test]
fn of_bufread_read0_splits_on_nul() {
    let opt = SkimItemReaderOption::default().read0(true).build();
    let reader = SkimItemReader::new(opt);
    let rx = reader.of_bufread(Cursor::new(b"a\0b\0c\0".to_vec()));
    assert_eq!(drain(rx), vec!["a", "b", "c"]);
}

#[test]
fn of_bufread_strips_ansi_when_enabled() {
    let opt = SkimItemReaderOption::default().ansi(true).build();
    let reader = SkimItemReader::new(opt);
    let rx = reader.of_bufread(Cursor::new(b"\x1b[31mred\x1b[0m\n".to_vec()));
    assert_eq!(drain(rx), vec!["red"]);
}

#[test]
fn of_bufread_with_custom_line_ending() {
    let opt = SkimItemReaderOption::default().line_ending(b';').build();
    let reader = SkimItemReader::new(opt);
    let rx = reader.of_bufread(Cursor::new(b"a;b;c;".to_vec()));
    assert_eq!(drain(rx), vec!["a", "b", "c"]);
}

#[test]
fn builder_setters_chain() {
    // Exercise the remaining builder setters; build() returns self.
    let opt = SkimItemReaderOption::default()
        .buf_size(4096)
        .ansi(false)
        .delimiter(Regex::new(r"\s+").unwrap())
        .with_nth(["1"].into_iter())
        .transform_fields(Vec::new())
        .nth(["2"].into_iter())
        .matching_fields(Vec::new())
        .show_error(true)
        .build();
    assert_eq!(opt.buf_size, 4096);
    assert!(opt.show_error);
}

#[test]
fn from_options_maps_read0_and_ansi() {
    let mut options = SkimOptions::default();
    options.read0 = true;
    options.ansi = true;
    let opt = SkimItemReaderOption::from_options(&options);
    assert_eq!(opt.line_ending, b'\0');
    assert!(opt.use_ansi_color);
}

#[test]
fn invoke_runs_a_command() {
    // The command runs through `shell_cmd` (`sh -c` / `cmd /c`); pick syntax that
    // emits two newline-separated lines on each platform. The reader strips a
    // trailing `\r`, so cmd.exe's CRLF output still yields bare "x"/"y".
    let cmd = if cfg!(windows) {
        "echo x& echo y"
    } else {
        "printf 'x\\ny\\n'"
    };
    let mut reader = SkimItemReader::default();
    let (rx, _tx) = reader.invoke(cmd, Arc::new(AtomicUsize::new(0)));
    assert_eq!(drain(rx), vec!["x", "y"]);
}

#[test]
fn invoke_with_show_error_redirects_stderr() {
    // show_error routes the child's stderr into the item stream. On cmd.exe the
    // redirect is written first so `echo` does not capture a trailing space.
    let cmd = if cfg!(windows) {
        "1>&2 echo oops"
    } else {
        "printf 'oops\\n' 1>&2"
    };
    let opt = SkimItemReaderOption::default().show_error(true).build();
    let mut reader = SkimItemReader::new(opt);
    let (rx, _tx) = reader.invoke(cmd, Arc::new(AtomicUsize::new(0)));
    assert_eq!(drain(rx), vec!["oops"]);
}

#[test]
fn read0_false_restores_newline_ending() {
    let opt = SkimItemReaderOption::default().read0(true).read0(false).build();
    assert_eq!(opt.line_ending, b'\n');
}

#[test]
fn option_setter_replaces_options() {
    // `SkimItemReader::option` swaps in a fresh option set.
    let reader = SkimItemReader::default().option(SkimItemReaderOption::default().read0(true).build());
    let rx = reader.of_bufread(Cursor::new(b"a\0b\0".to_vec()));
    assert_eq!(drain(rx), vec!["a", "b"]);
}

#[test]
fn thread_pool_setters() {
    // Both the chaining and the &mut variants accept a shared pool.
    let pool = default_thread_pool();
    let mut reader = SkimItemReader::default().with_thread_pool(pool.clone());
    reader.set_thread_pool(pool);
    let rx = reader.of_bufread(Cursor::new(b"x\ny\n".to_vec()));
    assert_eq!(drain(rx), vec!["x", "y"]);
}

#[test]
fn of_bufread_skips_invalid_utf8_lines() {
    // A line that is not valid UTF-8 is dropped; surrounding lines survive.
    let reader = SkimItemReader::default();
    let rx = reader.of_bufread(Cursor::new(b"ok\n\xff\xfe\nalso\n".to_vec()));
    assert_eq!(drain(rx), vec!["ok", "also"]);
}

#[test]
fn of_bufread_applies_with_nth_transform() {
    // with_nth selects the second whitespace field for display.
    let opt = SkimItemReaderOption::default().with_nth(["2"].into_iter()).build();
    let reader = SkimItemReader::new(opt);
    let rx = reader.of_bufread(Cursor::new(b"alpha beta gamma\n".to_vec()));
    // The selected field retains its trailing delimiter.
    assert_eq!(drain(rx), vec!["beta "]);
}
