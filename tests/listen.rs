#[allow(dead_code)]
#[macro_use]
mod common;

// use common::Keys::*;
// use interprocess::{
//     bound_util::RefWrite,
//     local_socket::{GenericNamespaced, Stream, ToNsName, traits::Stream as _},
// };
// use std::io::{Result, Write as _};

// fn connect(name: &str) -> Result<Stream> {
//     let ns_name = name.to_ns_name::<GenericNamespaced>().unwrap();
//     Stream::connect(ns_name)
// }
// fn send(stream: &Stream, msg: &str) -> Result<()> {
//     stream.as_write().write_all(format!("{msg}\n").as_bytes())
// }

// sk_test!(listen_up, "a\\nb\\nc\\nd", &["--listen", "sk-test-up"], {
//     @capture[0] starts_with(">");
//     @capture[2] starts_with("> a");
//     let stream = connect("sk-test-up")?;
//     send(&stream, "Up(2)")?;
//     @capture[2] trim().starts_with("a");
//     @capture[4] starts_with("> c");
// });
//
// sk_test!(listen_down, "a\\nb\\nc\\nd", &["--listen", "sk-test-down"], {
//     @capture[0] starts_with(">");
//     @capture[2] starts_with("> a");
//     let stream = connect("sk-test-down")?;
//     @keys Up, Up;
//     @capture[2] trim().starts_with("a");
//     @capture[4] starts_with("> c");
//     send(&stream, "Down(2)")?;
//     @capture[2] starts_with("> a");
// });
//
// // Test Abort action
// sk_test!(listen_abort, "a\\nb\\nc", &["--listen", "sk-test-abort"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-abort")?;
//     send(&stream, "Abort")?;
//     @capture[0] trim().contains("$");
// });
//
// // Test Accept action
// sk_test!(listen_accept, "apple\\nbanana\\ncherry", &["--listen", "sk-test-accept"], {
//     @capture[0] starts_with(">");
//     @capture[2] starts_with("> apple");
//     let stream = connect("sk-test-accept")?;
//     send(&stream, "Accept(None)")?;
//     @output[0] eq("apple");
// });
//
// // Test Accept with key
// sk_test!(listen_accept_key, "apple\\nbanana", &["--listen", "sk-test-accept-key"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-accept-key")?;
//     send(&stream, "Accept(Some(\"ctrl-a\"))")?;
//     @output[0] eq("ctrl-a");
//     @output[1] eq("apple");
// });
//
// // Test AddChar action
// sk_test!(listen_add_char, "apple\\napricot\\nbanana", &["--listen", "sk-test-add-char"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-add-char")?;
//     send(&stream, "AddChar('a')")?;
//     @capture[0] trim().eq("> a");
// });
//
// // Test AppendAndSelect action
// sk_test!(listen_append_select, "a\\nb\\nc", &["--listen", "sk-test-append-select", "-m"], {
//     @capture[0] starts_with(">");
//     @keys Key('d');
//     @capture[0] starts_with("> d");
//     @capture[2] starts_with("> a");
//     let stream = connect("sk-test-append-select")?;
//     send(&stream, "AppendAndSelect")?;
//     @capture[2] starts_with(">>d");
// });
//
// // Test BackwardChar action
// sk_test!(listen_backward_char, "test", &["--listen", "sk-test-backward-char"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-backward-char")?;
//     @keys Str("hello");
//     @capture[0] starts_with("> hello");
//     send(&stream, "BackwardChar")?;
//     @keys Key('|');
//     @capture[0] trim().eq("> hell|o");
// });
//
// // Test BackwardDeleteChar action
// sk_test!(listen_backward_delete_char, "apple\\nbanana", &["--listen", "sk-test-backward-delete-char"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-backward-delete-char")?;
//     @keys Str("test");
//     @capture[0] trim().eq("> test");
//     send(&stream, "BackwardDeleteChar")?;
//     @capture[0] trim().eq("> tes");
// });
//
// // Test BackwardDeleteCharEof action
// sk_test!(listen_backward_delete_char_eof, "a\\nb", &["--listen", "sk-test-backward-delete-char-eof"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-backward-delete-char-eof")?;
//     @keys Str("x");
//     @capture[0] trim().eq("> x");
//     send(&stream, "BackwardDeleteCharEof")?;
//     @capture[0] trim().eq(">");
// });
//
// // Test BackwardKillWord action
// sk_test!(listen_backward_kill_word, "test", &["--listen", "sk-test-backward-kill-word"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-backward-kill-word")?;
//     @keys Str("hello world");
//     @capture[0] trim().eq("> hello world");
//     send(&stream, "BackwardKillWord")?;
//     @capture[0] trim().eq("> hello");
// });
//
// // Test BackwardWord action
// sk_test!(listen_backward_word, "test", &["--listen", "sk-test-backward-word"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-backward-word")?;
//     @keys Str("hello world");
//     @capture[0] starts_with("> hello world");
//     send(&stream, "BackwardWord")?;
//     @keys Key('|');
//     @capture[0] trim().eq("> hello |world");
// });
//
// // Test BeginningOfLine action
// sk_test!(listen_beginning_of_line, "test", &["--listen", "sk-test-beginning-of-line"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-beginning-of-line")?;
//     @keys Str("hello");
//     @capture[0] starts_with("> hello");
//     send(&stream, "BeginningOfLine")?;
//     @keys Key('|');
//     @capture[0] trim().eq("> |hello");
// });
//
// // Test ClearScreen action
// sk_test!(listen_clear_screen, "a\\nb\\nc", &["--listen", "sk-test-clear-screen"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-clear-screen")?;
//     send(&stream, "ClearScreen")?;
//     @capture[0] starts_with(">");
// });
//
// // Test DeleteChar action
// sk_test!(listen_delete_char, "test", &["--listen", "sk-test-delete-char"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-delete-char")?;
//     @keys Str("hello");
//     @capture[0] trim().eq("> hello");
//     send(&stream, "BeginningOfLine")?;
//     send(&stream, "DeleteChar")?;
//     @capture[0] trim().eq("> ello");
// });
//
// // Test DeleteCharEof action
// sk_test!(listen_delete_char_eof, "a\\nb", &["--listen", "sk-test-delete-char-eof"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-delete-char-eof")?;
//     @keys Str("x");
//     @capture[0] starts_with("> x");
//     send(&stream, "BeginningOfLine")?;
//     send(&stream, "DeleteCharEof")?;
//     @capture[0] trim().eq(">");
// });
//
// // Test DeselectAll action
// sk_test!(listen_deselect_all, "a\\nb\\nc", &["--listen", "sk-test-deselect-all", "-m"], {
//     @capture[0] starts_with(">");
//     @capture[2] starts_with("> a");
//     let stream = connect("sk-test-deselect-all")?;
//     send(&stream, "Toggle")?;
//     @capture[2] starts_with(">>a");
//     send(&stream, "DeselectAll")?;
//     @capture[2] starts_with("> a");
// });
//
// // Test EndOfLine action
// sk_test!(listen_end_of_line, "test", &["--listen", "sk-test-end-of-line"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-end-of-line")?;
//     @keys Str("hello");
//     @capture[0] trim().eq("> hello");
//     send(&stream, "BeginningOfLine")?;
//     send(&stream, "EndOfLine")?;
//     @keys Key('X');
//     @capture[0] trim().eq("> helloX");
// });
//
// // Test Execute action
// sk_test!(listen_execute, "a\\nb\\nc", &["--listen", "sk-test-execute"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-execute")?;
//     send(&stream, "Execute(\"echo test\")")?;
//     // Execute runs command, skim should continue
//     @capture[0] starts_with(">");
// });
//
// // Test ExecuteSilent action
// sk_test!(listen_execute_silent, "a\\nb\\nc", &["--listen", "sk-test-execute-silent"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-execute-silent")?;
//     send(&stream, "ExecuteSilent(\"echo test\")")?;
//     @capture[0] starts_with(">");
// });
//
// // Test First action
// sk_test!(listen_first, "a\\nb\\nc\\nd", &["--listen", "sk-test-first"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-first")?;
//     @keys Up, Up;
//     @capture[4] starts_with("> c");
//     send(&stream, "First")?;
//     @capture[2] starts_with("> a");
// });
//
// // Test ForwardChar action
// sk_test!(listen_forward_char, "test", &["--listen", "sk-test-forward-char"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-forward-char")?;
//     @keys Str("hello");
//     @capture[0] trim().eq("> hello");
//     send(&stream, "BeginningOfLine")?;
//     send(&stream, "ForwardChar")?;
//     @keys Key('X');
//     @capture[0] trim().eq("> hXello");
// });
//
// // Test ForwardWord action
// sk_test!(listen_forward_word, "test", &["--listen", "sk-test-forward-word"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-forward-word")?;
//     @keys Str("hello world");
//     @capture[0] trim().eq("> hello world");
//     send(&stream, "BeginningOfLine")?;
//     send(&stream, "ForwardWord")?;
//     @keys Key('X');
//     @capture[0] trim().eq("> helloX world");
// });
//
// // Test Ignore action
// sk_test!(listen_ignore, "a\\nb\\nc", &["--listen", "sk-test-ignore"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-ignore")?;
//     send(&stream, "Ignore")?;
//     @capture[0] starts_with(">");
// });
//
// // Test KillLine action
// sk_test!(listen_kill_line, "test", &["--listen", "sk-test-kill-line"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-kill-line")?;
//     @keys Str("hello world");
//     @capture[0] trim().eq("> hello world");
//     send(&stream, "BackwardWord")?;
//     send(&stream, "KillLine")?;
//     @capture[0] trim().eq("> hello");
// });
//
// // Test KillWord action
// sk_test!(listen_kill_word, "test", &["--listen", "sk-test-kill-word"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-kill-word")?;
//     @keys Str("hello world");
//     @capture[0] trim().eq("> hello world");
//     send(&stream, "BeginningOfLine")?;
//     send(&stream, "KillWord")?;
//     @capture[0] trim().eq(">  world");
// });
//
// // Test Last action
// sk_test!(listen_last, "a\\nb\\nc\\nd", &["--listen", "sk-test-last"], {
//     @capture[0] starts_with(">");
//     @capture[2] starts_with("> a");
//     @capture[5] trim().eq("d");
//     let stream = connect("sk-test-last")?;
//     send(&stream, "Last")?;
//     @capture[5] starts_with("> d");
// });
//
// // Test NextHistory action
// sk_test!(listen_next_history, "a\\nb", &["--listen", "sk-test-next-history", "--history=/tmp/sk-history-next"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-next-history")?;
//     send(&stream, "NextHistory")?;
//     @capture[0] starts_with(">");
// });
//
// // Test HalfPageDown action
// sk_test!(listen_half_page_down, "1\\n2\\n3\\n4\\n5\\n6\\n7\\n8\\n9\\n10", &["--listen", "sk-test-half-page-down"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-half-page-down")?;
//     send(&stream, "HalfPageDown(1)")?;
//     @capture[*] contains(">");
// });
//
// // Test HalfPageUp action
// sk_test!(listen_half_page_up, "1\\n2\\n3\\n4\\n5\\n6\\n7\\n8\\n9\\n10", &["--listen", "sk-test-half-page-up"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-half-page-up")?;
//     send(&stream, "Last")?;
//     send(&stream, "HalfPageUp(1)")?;
//     @capture[*] contains(">");
// });
//
// // Test PageDown action
// sk_test!(listen_page_down, "1\\n2\\n3\\n4\\n5\\n6\\n7\\n8\\n9\\n10", &["--listen", "sk-test-page-down"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-page-down")?;
//     send(&stream, "PageDown(1)")?;
//     @capture[*] contains(">");
// });
//
// // Test PageUp action
// sk_test!(listen_page_up, "1\\n2\\n3\\n4\\n5\\n6\\n7\\n8\\n9\\n10", &["--listen", "sk-test-page-up"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-page-up")?;
//     send(&stream, "Last")?;
//     send(&stream, "PageUp(1)")?;
//     @capture[*] contains(">");
// });
//
// // Test Reload action with command
// sk_test!(listen_reload_cmd, "a\\nb\\nc", &["--listen", "sk-test-reload-cmd"], {
//     @capture[0] starts_with(">");
//     @capture[1] trim().contains("3/3");
//     let stream = connect("sk-test-reload-cmd")?;
//     send(&stream, "Reload(Some(\"printf 'x\\\\ny\\\\nz'\"))")?;
//     @capture[2] starts_with("> x");
// });
//
// // Test SelectAll action
// sk_test!(listen_select_all, "a\\nb\\nc", &["--listen", "sk-test-select-all", "-m"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-select-all")?;
//     send(&stream, "SelectAll")?;
//     @capture[2] trim().eq(">>a");
//     @capture[3] trim().eq(">b");
//     @capture[4] trim().eq(">c");
// });
//
// // Test SelectRow action
// sk_test!(listen_select_row, "a\\nb\\nc\\nd", &["--listen", "sk-test-select-row", "-m"], {
//     @capture[0] starts_with(">");
//     @capture[2] starts_with("> a");
//     let stream = connect("sk-test-select-row")?;
//     send(&stream, "SelectRow(2)")?;
//     @capture[2] trim().eq("> a");
//     @capture[4] trim().eq(">c");
// });
//
// // Test Select action
// sk_test!(listen_select, "a\\nb\\nc", &["--listen", "sk-test-select", "-m"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-select")?;
//     send(&stream, "Select")?;
//     @capture[2] starts_with(">>");
// });
//
// // Test Toggle action
// sk_test!(listen_toggle, "a\\nb\\nc", &["--listen", "sk-test-toggle", "-m"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-toggle")?;
//     send(&stream, "Toggle")?;
//     @capture[2] starts_with(">>a");
//     send(&stream, "Toggle")?;
//     @capture[2] starts_with("> a");
// });
//
// // Test ToggleAll action
// sk_test!(listen_toggle_all, "a\\nb\\nc", &["--listen", "sk-test-toggle-all", "-m"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-toggle-all")?;
//     send(&stream, "ToggleAll")?;
//     @capture[2] starts_with(">>a");
//     @capture[3] trim().eq(">b");
//     @capture[4] trim().eq(">c");
// });
//
// // Test ToggleIn action
// sk_test!(listen_toggle_in, "a\\nb\\nc\\nd", &["--listen", "sk-test-toggle-in", "-m"], {
//     @capture[0] starts_with(">");
//     @keys Up;
//     @capture[2] trim().starts_with("a");
//     @capture[3] starts_with("> b");
//     let stream = connect("sk-test-toggle-in")?;
//     send(&stream, "ToggleIn")?;
//     @capture[2] starts_with("> a");
//     @capture[3] trim().starts_with(">b");
// });
//
// // Test ToggleOut action
// sk_test!(listen_toggle_out, "a\\nb\\nc\\nd", &["--listen", "sk-test-toggle-out", "-m"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-toggle-out")?;
//     @capture[2] starts_with("> a");
//     @capture[3] trim().starts_with("b");
//     send(&stream, "ToggleOut")?;
//     @capture[2] trim().starts_with(">a");
//     @capture[3] starts_with("> b");
// });
//
// // Test Top action (alias for First)
// sk_test!(listen_top, "a\\nb\\nc\\nd", &["--listen", "sk-test-top"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-top")?;
//     @keys Up, Up;
//     @capture[4] starts_with("> c");
//     send(&stream, "Top")?;
//     @capture[2] starts_with("> a");
// });
//
// // Test UnixLineDiscard action
// sk_test!(listen_unix_line_discard, "test", &["--listen", "sk-test-unix-line-discard"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-unix-line-discard")?;
//     @keys Str("hello world");
//     @capture[0] trim().eq("> hello world");
//     send(&stream, "UnixLineDiscard")?;
//     @capture[0] trim().eq(">");
// });
//
// // Test UnixWordRubout action
// sk_test!(listen_unix_word_rubout, "test", &["--listen", "sk-test-unix-word-rubout"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-unix-word-rubout")?;
//     @keys Str("hello world");
//     @capture[0] trim().eq("> hello world");
//     send(&stream, "UnixWordRubout")?;
//     @capture[0] trim().eq("> hello");
// });
//
// // Test Yank action
// sk_test!(listen_yank, "test", &["--listen", "sk-test-yank"], {
//     @capture[0] starts_with(">");
//     let stream = connect("sk-test-yank")?;
//     @keys Str("hello");
//     @capture[0] trim().eq("> hello");
//     send(&stream, "BackwardKillWord")?;
//     @capture[0] trim().eq(">");
//     send(&stream, "Yank")?;
//     @capture[0] trim().eq("> hello");
// });

#[cfg(not(target_os = "linux"))]
sk_test!(listen_vanilla, "", &["--listen", "sk-test-listen-vanilla"], {
    std::thread::sleep(std::time::Duration::from_millis(2000));
    @dbg;
    @capture[0] starts_with(">");
});
