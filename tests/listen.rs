#[allow(dead_code)]
#[macro_use]
mod common;

use common::Keys::*;
use interprocess::{
    bound_util::RefWrite,
    local_socket::{GenericNamespaced, Stream, ToNsName, traits::Stream as _},
};
use rand::{Rng as _, distr::Alphabetic};
use std::io::{Result, Write as _};

use crate::common::TmuxController;

fn connect(name: &str) -> Result<Stream> {
    let ns_name = name.to_ns_name::<GenericNamespaced>().unwrap();
    Stream::connect(ns_name)
}
fn send(stream: &Stream, msg: &str) -> Result<()> {
    stream.as_write().write_all(format!("{msg}\n").as_bytes())
}

fn setup(name: &str, extra_args: &[&str]) -> Result<(TmuxController, Stream)> {
    let mut tmux = TmuxController::new_named(name)?;
    let socket_name = format!(
        "sk-test-{name}{}",
        rand::rng()
            .sample_iter(&Alphabetic)
            .take(4)
            .map(char::from)
            .collect::<String>()
    );
    tmux.start_sk(
        Some(&format!("echo -n -e '{}'", "a\\nb\\nc\\nd")),
        &[&["--listen", &socket_name], extra_args].concat(),
    )?;
    tmux.until(|l| l.len() > 0 && l[0].starts_with(">"))?;
    let stream = connect(&socket_name)?;
    Ok((tmux, stream))
}

#[test]
fn listen_up() -> std::io::Result<()> {
    let (tmux, stream) = setup("up", &[])?;
    sk_test!(@expand tmux;
        @capture[2]starts_with("> a");
        send(&stream,"Up(2)")? ;
        @capture[2]trim().starts_with("a");
        @capture[4]starts_with("> c");
    );
    Ok(())
}

#[test]
fn listen_down() -> std::io::Result<()> {
    let (tmux, stream) = setup("down", &[])?;
    sk_test!(@expand tmux;
        @capture[2]starts_with("> a");
        @keys Up, Up;
        @capture[2]trim().starts_with("a");
        @capture[4]starts_with("> c");
        send(&stream, "Down(2)")?;
        @capture[2]starts_with("> a");
    );
    Ok(())
}

#[test]
fn listen_abort() -> std::io::Result<()> {
    let (tmux, stream) = setup("abort", &[])?;
    sk_test!(@expand tmux;
        send(&stream, "Abort")?;
        @capture[0]trim().contains("$");
    );
    Ok(())
}

// Test Accept action - adapted to use "a" instead of "apple"
#[test]
fn listen_accept() -> std::io::Result<()> {
    let (tmux, stream) = setup("accept", &[])?;
    sk_test!(@expand tmux;
        @capture[2]starts_with("> a");
        send(&stream, "Accept(None)")?;
        @output[0]eq("a");
    );
    Ok(())
}

// Test Accept with key - adapted to use "a" instead of "apple"
#[test]
fn listen_accept_key() -> std::io::Result<()> {
    let (tmux, stream) = setup("accept_key", &[])?;
    sk_test!(@expand tmux;
        send(&stream, "Accept(Some(\"ctrl-a\"))")?;
        @output[0]eq("ctrl-a");
        @output[1]eq("a");
    );
    Ok(())
}

#[test]
fn listen_add_char() -> std::io::Result<()> {
    let (tmux, stream) = setup("add_char", &[])?;
    sk_test!(@expand tmux;
        send(&stream, "AddChar('a')")?;
        @capture[0]trim().eq("> a");
    );
    Ok(())
}

#[test]
fn listen_backward_char() -> std::io::Result<()> {
    let (tmux, stream) = setup("backward_char", &[])?;
    sk_test!(@expand tmux;
        @keys Str("hello");
        @capture[0]starts_with("> hello");
        send(&stream, "BackwardChar")?;
        @keys Key('|');
        @capture[0]trim().eq("> hell|o");
    );
    Ok(())
}

#[test]
fn listen_backward_delete_char() -> std::io::Result<()> {
    let (tmux, stream) = setup("backward_delete_char", &[])?;
    sk_test!(@expand tmux;
        @keys Str("test");
        @capture[0]trim().eq("> test");
        send(&stream, "BackwardDeleteChar")?;
        @capture[0]trim().eq("> tes");
    );
    Ok(())
}

#[test]
fn listen_backward_delete_char_eof() -> std::io::Result<()> {
    let (tmux, stream) = setup("backward_delete_char_eof", &[])?;
    sk_test!(@expand tmux;
        @keys Str("x");
        @capture[0]trim().eq("> x");
        send(&stream, "BackwardDeleteCharEof")?;
        @capture[0]trim().eq(">");
    );
    Ok(())
}

#[test]
fn listen_backward_kill_word() -> std::io::Result<()> {
    let (tmux, stream) = setup("backward_kill_word", &[])?;
    sk_test!(@expand tmux;
        @keys Str("hello world");
        @capture[0]trim().eq("> hello world");
        send(&stream, "BackwardKillWord")?;
        @capture[0]trim().eq("> hello");
    );
    Ok(())
}

#[test]
fn listen_backward_word() -> std::io::Result<()> {
    let (tmux, stream) = setup("backward_word", &[])?;
    sk_test!(@expand tmux;
        @keys Str("hello world");
        @capture[0]starts_with("> hello world");
        send(&stream, "BackwardWord")?;
        @keys Key('|');
        @capture[0]trim().eq("> hello |world");
    );
    Ok(())
}

#[test]
fn listen_end_of_line() -> std::io::Result<()> {
    let (tmux, stream) = setup("end_of_line", &[])?;
    sk_test!(@expand tmux;
        @keys Str("hello");
        @capture[0]trim().eq("> hello");
        send(&stream, "BeginningOfLine")?;
        send(&stream, "EndOfLine")?;
        @keys Key('X');
        @capture[0]trim().eq("> helloX");
    );
    Ok(())
}

#[test]
fn listen_first() -> std::io::Result<()> {
    let (tmux, stream) = setup("first", &[])?;
    sk_test!(@expand tmux;
        @keys Up, Up;
        @capture[4]starts_with("> c");
        send(&stream, "First")?;
        @capture[2]starts_with("> a");
    );
    Ok(())
}

#[test]
fn listen_forward_char() -> std::io::Result<()> {
    let (tmux, stream) = setup("forward_char", &[])?;
    sk_test!(@expand tmux;
        @keys Str("hello");
        @capture[0]trim().eq("> hello");
        send(&stream, "BeginningOfLine")?;
        send(&stream, "ForwardChar")?;
        @keys Key('X');
        @capture[0]trim().eq("> hXello");
    );
    Ok(())
}

#[test]
fn listen_forward_word() -> std::io::Result<()> {
    let (tmux, stream) = setup("forward_word", &[])?;
    sk_test!(@expand tmux;
        @keys Str("hello world");
        @capture[0]trim().eq("> hello world");
        send(&stream, "BeginningOfLine")?;
        send(&stream, "ForwardWord")?;
        @keys Key('X');
        @capture[0]trim().eq("> helloX world");
    );
    Ok(())
}

#[test]
fn listen_kill_line() -> std::io::Result<()> {
    let (tmux, stream) = setup("kill_line", &[])?;
    sk_test!(@expand tmux;
        @keys Str("hello world");
        @capture[0]trim().eq("> hello world");
        send(&stream, "BackwardWord")?;
        send(&stream, "KillLine")?;
        @capture[0]trim().eq("> hello");
    );
    Ok(())
}

#[test]
fn listen_kill_word() -> std::io::Result<()> {
    let (tmux, stream) = setup("kill_word", &[])?;
    sk_test!(@expand tmux;
        @keys Str("hello world");
        @capture[0]trim().eq("> hello world");
        send(&stream, "BeginningOfLine")?;
        send(&stream, "KillWord")?;
        @capture[0]trim().eq(">  world");
    );
    Ok(())
}

#[test]
fn listen_last() -> std::io::Result<()> {
    let (tmux, stream) = setup("last", &[])?;
    sk_test!(@expand tmux;
        @capture[2]starts_with("> a");
        @capture[5]trim().eq("d");
        send(&stream, "Last")?;
        @capture[5]starts_with("> d");
    );
    Ok(())
}

// Test Reload action with command
#[test]
fn listen_reload_cmd() -> std::io::Result<()> {
    let (tmux, stream) = setup("reload_cmd", &[])?;
    sk_test!(@expand tmux;
        @capture[1]trim().contains("4/4");
        send(&stream, "Reload(Some(\"printf 'x\\\\ny\\\\nz'\"))")?;
        @capture[2]starts_with("> x");
    );
    Ok(())
}

#[test]
fn listen_select_all() -> std::io::Result<()> {
    let (tmux, stream) = setup("select_all", &["-m"])?;
    sk_test!(@expand tmux;
        send(&stream, "SelectAll")?;
        @capture[2]trim().eq(">>a");
        @capture[3]trim().eq(">b");
        @capture[4]trim().eq(">c");
    );
    Ok(())
}

#[test]
fn listen_select_row() -> std::io::Result<()> {
    let (tmux, stream) = setup("select_row", &["-m"])?;
    sk_test!(@expand tmux;
        @capture[2]starts_with("> a");
        send(&stream, "SelectRow(2)")?;
        @capture[2]trim().eq("> a");
        @capture[4]trim().eq(">c");
    );
    Ok(())
}

#[test]
fn listen_select() -> std::io::Result<()> {
    let (tmux, stream) = setup("select", &["-m"])?;
    sk_test!(@expand tmux;
        send(&stream, "Select")?;
        @capture[2]starts_with(">>");
    );
    Ok(())
}

#[test]
fn listen_toggle() -> std::io::Result<()> {
    let (tmux, stream) = setup("toggle", &["-m"])?;
    sk_test!(@expand tmux;
        send(&stream, "Toggle")?;
        @capture[2]starts_with(">>a");
        send(&stream, "Toggle")?;
        @capture[2]starts_with("> a");
    );
    Ok(())
}

#[test]
fn listen_toggle_all() -> std::io::Result<()> {
    let (tmux, stream) = setup("toggle_all", &["-m"])?;
    sk_test!(@expand tmux;
        send(&stream, "ToggleAll")?;
        @capture[2]starts_with(">>a");
        @capture[3]trim().eq(">b");
        @capture[4]trim().eq(">c");
    );
    Ok(())
}

#[test]
fn listen_toggle_in() -> std::io::Result<()> {
    let (tmux, stream) = setup("toggle_in", &["-m"])?;
    sk_test!(@expand tmux;
        @keys Up;
        @capture[2]trim().starts_with("a");
        @capture[3]starts_with("> b");
        send(&stream, "ToggleIn")?;
        @capture[2]starts_with("> a");
        @capture[3]trim().starts_with(">b");
    );
    Ok(())
}

#[test]
fn listen_toggle_out() -> std::io::Result<()> {
    let (tmux, stream) = setup("toggle_out", &["-m"])?;
    sk_test!(@expand tmux;
        @capture[2]starts_with("> a");
        @capture[3]trim().starts_with("b");
        send(&stream, "ToggleOut")?;
        @capture[2]trim().starts_with(">a");
        @capture[3]starts_with("> b");
    );
    Ok(())
}

// Test Top action (alias for First)
#[test]
fn listen_top() -> std::io::Result<()> {
    let (tmux, stream) = setup("top", &[])?;
    sk_test!(@expand tmux;
        @keys Up, Up;
        @capture[4]starts_with("> c");
        send(&stream, "Top")?;
        @capture[2]starts_with("> a");
    );
    Ok(())
}

#[test]
fn listen_unix_line_discard() -> std::io::Result<()> {
    let (tmux, stream) = setup("unix_line_discard", &[])?;
    sk_test!(@expand tmux;
        @keys Str("hello world");
        @capture[0]trim().eq("> hello world");
        send(&stream, "UnixLineDiscard")?;
        @capture[0]trim().eq(">");
    );
    Ok(())
}

#[test]
fn listen_unix_word_rubout() -> std::io::Result<()> {
    let (tmux, stream) = setup("unix_word_rubout", &[])?;
    sk_test!(@expand tmux;
        @keys Str("hello world");
        @capture[0]trim().eq("> hello world");
        send(&stream, "UnixWordRubout")?;
        @capture[0]trim().eq("> hello");
    );
    Ok(())
}

#[test]
fn listen_yank() -> std::io::Result<()> {
    let (tmux, stream) = setup("yank", &[])?;
    sk_test!(@expand tmux;
        @keys Str("hello");
        @capture[0]trim().eq("> hello");
        send(&stream, "BackwardKillWord")?;
        @capture[0]trim().eq(">");
        send(&stream, "Yank")?;
        @capture[0]trim().eq("> hello");
    );
    Ok(())
}
