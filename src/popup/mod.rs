//! Tmux & Zellij integration utilities.
//!
//! This module provides functionality for running skim within tmux/zellij panes,
//! allowing skim to be used as a tmux popup or split pane.

mod tmux;
mod zellij;

use std::{
    borrow::Cow,
    fmt::Write as FmtWrite,
    io::{BufRead as _, BufReader, BufWriter, IsTerminal as _, Write as _},
    process::ExitStatus,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use nix::sys::stat::Mode;
use nix::unistd::mkfifo;
use rand::{RngExt as _, distr::Alphanumeric};

use crate::{
    Rank, SkimItem, SkimOptions, SkimOutput,
    item::{MatchedItem, RankBuilder},
    tui::{Event, event::Action},
};

use tmux::TmuxPopup;
use zellij::ZellijPopup;

#[derive(Debug, PartialEq, Eq)]
enum PopupWindowDir {
    Center,
    Top,
    Bottom,
    Left,
    Right,
}

impl From<&str> for PopupWindowDir {
    fn from(value: &str) -> Self {
        use PopupWindowDir::{Bottom, Center, Left, Right, Top};
        match value {
            "top" => Top,
            "bottom" => Bottom,
            "left" => Left,
            "right" => Right,
            _ => Center, // includes "center" and all unknown values
        }
    }
}

trait SkimPopup {
    fn from_options(options: &SkimOptions) -> Box<dyn SkimPopup>
    where
        Self: Sized;
    fn add_env(&mut self, key: &str, value: &str);
    fn run_and_wait(&mut self, command: &str) -> std::io::Result<ExitStatus>;
}

struct SkimPopupOutput {
    line: String,
}

impl SkimItem for SkimPopupOutput {
    fn text(&self) -> Cow<'_, str> {
        Cow::from(&self.line)
    }
}

/// Returns whether or not a compatible multiplexer is running
#[must_use]
pub fn check_env() -> bool {
    tmux::is_available() || zellij::is_available()
}

/// Run skim in a tmux popup
///
/// This will extract the tmux options, then build a new sk command
/// without them and send it to tmux in a popup.
///
/// # Panics
///
/// Panics if the temporary directory for IPC cannot be created.
#[allow(clippy::too_many_lines)]
pub fn run_with(opts: &SkimOptions) -> Option<SkimOutput> {
    // Create temp dir for downstream output
    let temp_dir_name = format!(
        "sk-popup-{}",
        &rand::rng()
            .sample_iter(&Alphanumeric)
            .take(8)
            .map(char::from)
            .collect::<String>(),
    );
    let temp_dir = std::env::temp_dir().join(&temp_dir_name);
    std::fs::create_dir(&temp_dir)
        .unwrap_or_else(|e| panic!("Failed to create temp dir {}: {}", temp_dir.display(), e));

    debug!("Created temp dir {}", temp_dir.display());
    let tmp_stdout = temp_dir.join("stdout");
    let tmp_stdin = temp_dir.join("stdin");

    let has_piped_input = !std::io::stdin().is_terminal();
    let mut stdin_reader = BufReader::new(std::io::stdin());
    let line_ending = if opts.read0 { b'\0' } else { b'\n' };

    let stop_reading = Arc::new(AtomicBool::new(false));
    let _stdin_handle = if has_piped_input {
        debug!("Reading stdin and piping to fifo");

        // Create a named pipe (FIFO)
        // This allows the nested skim to continuously read as data arrives
        let stdin_path_str = tmp_stdin
            .to_str()
            .unwrap_or_else(|| panic!("Failed to convert stdin path to string"));
        mkfifo(stdin_path_str, Mode::S_IRUSR | Mode::S_IWUSR)
            .unwrap_or_else(|e| panic!("Failed to create fifo {}: {}", tmp_stdin.display(), e));

        let tmp_stdin_clone = tmp_stdin.clone();
        let stop_flag = Arc::clone(&stop_reading);
        Some(thread::spawn(move || {
            debug!("Opening fifo for writing (may block until reader starts)");
            let stdin_f = std::fs::File::create(tmp_stdin_clone.clone())
                .unwrap_or_else(|e| panic!("Failed to open fifo {}: {}", tmp_stdin_clone.display(), e));
            debug!("Fifo opened for writing");
            let mut stdin_writer = BufWriter::new(stdin_f);
            loop {
                // Check if we should stop reading
                if stop_flag.load(Ordering::Relaxed) {
                    debug!("Stop signal received, exiting stdin reader thread");
                    break;
                }

                let mut buf = vec![];
                match stdin_reader.read_until(line_ending, &mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        debug!("Read {n} bytes from stdin");
                        stdin_writer.write_all(&buf).unwrap();
                    }
                    Err(e) => panic!("Failed to read from stdin: {e}"),
                }
            }
            // Ensure all buffered data is written to the file
            let _ = stdin_writer.flush();
        }))
    } else {
        None
    };

    // Build args to send to downstream sk invocation
    let mut stripped_shell_cmd = String::new();
    let mut prev_is_popup = false;
    let mut prev_is_output_format_flag = false;
    // We keep argv[0] to use in the popup's command
    for arg in std::env::args() {
        debug!("Got arg {arg}");
        if prev_is_popup {
            prev_is_popup = false;
            if !arg.starts_with('-') {
                continue;
            }
        } else if prev_is_output_format_flag {
            prev_is_output_format_flag = false;
            continue;
        }
        if arg == "--tmux" || arg == "--popup" {
            debug!("Found popup arg, skipping this and the next");
            prev_is_popup = true;
            continue;
        } else if arg.starts_with("--tmux") || arg.starts_with("--popup") {
            debug!("Found equal popup arg, skipping");
            continue;
        } else if arg == "--output-format" {
            debug!("Found output format arg, skipping this and the next");
            prev_is_output_format_flag = true;
            continue;
        } else if arg.starts_with("--output-format") {
            debug!("Found equal output format arg, skipping");
            continue;
        }
        push_quoted_arg(&mut stripped_shell_cmd, &arg);
    }
    // Always add all --print-xxx flags to the child sk command so that the output
    // is fully structured and can be parsed unconditionally below, regardless of
    // which flags the user originally passed.
    for flag in &[
        "--print-query",
        "--print-cmd",
        "--print-header",
        "--print-current",
        "--print-score",
    ] {
        let _ = write!(stripped_shell_cmd, " {flag}");
    }

    if has_piped_input {
        let _ = write!(stripped_shell_cmd, " <{}", tmp_stdin.display());
    }
    let _ = write!(stripped_shell_cmd, " >{}", tmp_stdout.display());

    debug!("build cmd {}", &stripped_shell_cmd);

    // Run downstream sk in tmux
    let mut popup: Box<dyn SkimPopup> = if zellij::is_available() {
        ZellijPopup::from_options(opts)
    } else if tmux::is_available() {
        TmuxPopup::from_options(opts)
    } else {
        panic!("You shouldn't have been able to get here");
    };

    for (name, value) in std::env::vars() {
        if name.starts_with("SKIM") || name == "PATH" || name.starts_with("RUST") {
            let value = sanitize_value(value);
            debug!("adding {name} = {value} to the command's env");
            popup.add_env(&name, &value);
        }
    }

    let status = popup
        .run_and_wait(&stripped_shell_cmd)
        .unwrap_or_else(|e| panic!("Popup invocation of {stripped_shell_cmd} failed with {e}"));

    // Signal the stdin thread to stop and wait for it to exit
    stop_reading.store(true, Ordering::Relaxed);

    let output_ending = if opts.print0 { "\0" } else { "\n" };
    let mut stdout_bytes = std::fs::read_to_string(tmp_stdout).unwrap_or_default();
    stdout_bytes.pop();
    let mut stdout = stdout_bytes.split(output_ending);
    let _ = std::fs::remove_dir_all(temp_dir);

    debug!("popup stdout: {stdout:?}");

    // The child sk process always runs with --print-query, --print-cmd, --print-header,
    // and --print-score, so we always read those lines unconditionally.
    let query_str = if status.success() {
        stdout.next().unwrap_or_default()
    } else {
        ""
    };

    let command_str = if status.success() {
        stdout.next().unwrap_or_default()
    } else {
        ""
    };

    let header = if status.success() {
        stdout.next().unwrap_or_default()
    } else {
        ""
    }
    .to_string();

    let current: Option<MatchedItem> = if status.success() {
        let line = stdout.next().unwrap_or_default();
        if line.is_empty() {
            None
        } else {
            Some(MatchedItem::new(
                Arc::new(SkimPopupOutput { line: line.to_string() }),
                Rank::default(),
                None,
                &RankBuilder::default(),
            ))
        }
    } else {
        None
    };

    let mut output_lines: Vec<MatchedItem> = vec![];
    while let Some(line) = stdout.next() {
        debug!("Adding output line: {line}");
        // --print-score is always enabled in the child, so every item is followed by its score.
        let score: i32 = stdout.next().unwrap_or_default().parse().unwrap_or_default();
        let rank = Rank {
            score,
            ..Default::default()
        };
        let item = MatchedItem::new(
            Arc::new(SkimPopupOutput { line: line.to_string() }),
            rank,
            None,
            &RankBuilder::default(),
        );
        output_lines.push(item);
    }

    let is_abort = !status.success();
    let final_event = if is_abort {
        Event::Action(Action::Abort)
    } else {
        Event::Action(Action::Accept(None))
    };

    let skim_output = SkimOutput {
        final_event,
        is_abort,
        final_key: KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        // Note: In poup mode, the actual final key is not available since skim runs in a separate
        // popup process. Only the output text is captured. Use --expect with --bind to capture
        // specific accept keys in the output if needed.
        query: query_str.to_string(),
        cmd: command_str.to_string(),
        selected_items: output_lines,
        current,
        header,
    };
    Some(skim_output)
}

fn push_quoted_arg(args_str: &mut String, arg: &str) {
    use shell_quote::{Quote as _, Sh};
    let _ = write!(
        args_str,
        " {}",
        String::from_utf8(Sh::quote(arg)).expect("Failed to parse quoted arg as utf8, this should not happen")
    );
}

fn sanitize_value(value: String) -> String {
    if !value.ends_with(';') {
        return value;
    }

    let mut value = value.clone();
    value.replace_range(value.len() - 1.., "\\;");
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── PopupWindowDir::from ──────────────────────────────────────────────────

    #[test]
    fn popup_window_dir_known_values() {
        assert_eq!(PopupWindowDir::from("center"), PopupWindowDir::Center);
        assert_eq!(PopupWindowDir::from("top"), PopupWindowDir::Top);
        assert_eq!(PopupWindowDir::from("bottom"), PopupWindowDir::Bottom);
        assert_eq!(PopupWindowDir::from("left"), PopupWindowDir::Left);
        assert_eq!(PopupWindowDir::from("right"), PopupWindowDir::Right);
    }

    #[test]
    fn popup_window_dir_unknown_falls_back_to_center() {
        assert_eq!(PopupWindowDir::from(""), PopupWindowDir::Center);
        assert_eq!(PopupWindowDir::from("foobar"), PopupWindowDir::Center);
        assert_eq!(PopupWindowDir::from("CENTER"), PopupWindowDir::Center); // case-sensitive
    }

    // ── sanitize_value ────────────────────────────────────────────────────────

    #[test]
    fn sanitize_value_no_semicolon() {
        assert_eq!(sanitize_value("hello".to_string()), "hello");
        assert_eq!(sanitize_value("foo=bar".to_string()), "foo=bar");
        assert_eq!(sanitize_value(String::new()), "");
    }

    #[test]
    fn sanitize_value_trailing_semicolon_is_escaped() {
        assert_eq!(sanitize_value("hello;".to_string()), "hello\\;");
        assert_eq!(sanitize_value(";".to_string()), "\\;");
    }

    #[test]
    fn sanitize_value_semicolon_in_middle_unchanged() {
        assert_eq!(sanitize_value("hel;lo".to_string()), "hel;lo");
        assert_eq!(sanitize_value("a;b;c".to_string()), "a;b;c");
    }

    // ── push_quoted_arg ───────────────────────────────────────────────────────
    // These tests mutate the SHELL env var. `#[serial]` ensures they never run
    // concurrently. `set_var`/`remove_var` are `unsafe fn` in Rust ≥ 1.81
    // (edition 2024); the SAFETY invariant holds because `#[serial]` serialises
    // access so no other thread reads the var while it is being written.

    #[test]
    #[serial_test::serial]
    fn push_quoted_arg_simple_word_sh() {
        // SAFETY: serialised by #[serial]; no concurrent reads of SHELL.
        unsafe { std::env::set_var("SHELL", "/bin/sh") };
        let mut s = String::new();
        push_quoted_arg(&mut s, "hello");
        assert_eq!(s, " hello");
        unsafe { std::env::remove_var("SHELL") };
    }

    #[test]
    #[serial_test::serial]
    fn push_quoted_arg_spaces_are_quoted() {
        // SAFETY: serialised by #[serial]; no concurrent reads of SHELL.
        unsafe { std::env::set_var("SHELL", "/bin/sh") };
        let mut s = String::new();
        push_quoted_arg(&mut s, "hello world");
        // The result must preserve both words and not be a bare unquoted string
        assert!(s.contains("hello"));
        assert!(s.contains("world"));
        assert_ne!(s.trim(), "hello world"); // must be quoted somehow
        unsafe { std::env::remove_var("SHELL") };
    }

    #[test]
    #[serial_test::serial]
    fn push_quoted_arg_appends_with_space_prefix() {
        // SAFETY: serialised by #[serial]; no concurrent reads of SHELL.
        unsafe { std::env::set_var("SHELL", "/bin/sh") };
        let mut s = String::from("sk");
        push_quoted_arg(&mut s, "--flag");
        assert!(s.starts_with("sk "));
        unsafe { std::env::remove_var("SHELL") };
    }

    #[test]
    #[serial_test::serial]
    fn push_quoted_arg_bash_shell() {
        // SAFETY: serialised by #[serial]; no concurrent reads of SHELL.
        unsafe { std::env::set_var("SHELL", "/usr/bin/bash") };
        let mut s = String::new();
        push_quoted_arg(&mut s, "simple");
        assert_eq!(s, " simple");
        unsafe { std::env::remove_var("SHELL") };
    }

    #[test]
    #[serial_test::serial]
    fn push_quoted_arg_zsh_shell() {
        // SAFETY: serialised by #[serial]; no concurrent reads of SHELL.
        unsafe { std::env::set_var("SHELL", "/bin/zsh") };
        let mut s = String::new();
        push_quoted_arg(&mut s, "simple");
        assert_eq!(s, " simple");
        unsafe { std::env::remove_var("SHELL") };
    }
}
