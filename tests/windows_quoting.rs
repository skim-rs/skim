#![allow(missing_docs, clippy::pedantic)]
#![cfg(windows)]

#[allow(dead_code)]
mod common;

use std::fs;
use std::time::{Duration, Instant};

use skim::prelude::*;
use skim::tui::event::Action;

const CHILD_MARKER: &str = "--skim-quoting-child";

// The test binary also serves as an argv recorder. Arguments after `--` are
// libtest filters, so the child can receive arbitrary item text without a shell script.
#[test]
fn capture_quoted_arguments() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let Some(marker) = args.iter().position(|arg| arg == CHILD_MARKER) else {
        return Ok(());
    };
    fs::write(&args[marker + 1], serde_json::to_vec(&args[marker + 2..])?)?;
    Ok(())
}

#[test]
fn command_expansion_rejects_windows_line_breaks_before_execution() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let invoked = dir.path().join("shell-invoked.txt");
    for input in [
        b"ok\r\necho injected\0".as_slice(),
        b"ok\recho injected\0".as_slice(),
        b"ok\necho injected\0".as_slice(),
    ] {
        let value = std::str::from_utf8(&input[..input.len() - 1])?;
        let options = SkimOptionsBuilder::default().read0(true).multi(true).build()?;
        let mut harness = common::insta::enter_bytes(input, options)?;
        assert_eq!(harness.skim.app().item_list.selected().unwrap().output(), value);
        harness.action(Action::SelectAll)?;
        harness.skim.app_mut().input.value = value.into();

        for placeholder in ["{}", "{+}", "{q}", "{cq}", "{1..}", "{+1..}"] {
            // If CMD starts at all, the first command writes the marker file.
            let command = format!("echo invoked > \"{}\" & echo {placeholder}", invoked.display());
            for action in [Action::ExecuteSilent(command.clone()), Action::Execute(command)] {
                let error = harness
                    .action(action)
                    .expect_err("CR/LF must be rejected before invoking CMD");
                assert!(
                    error
                        .to_string()
                        .contains("CMD command expansion cannot contain CR or LF")
                );
                assert!(!invoked.exists(), "CMD must not run any part of the rejected command");
            }
        }
    }
    Ok(())
}

#[test]
fn issue_1194_execute_silent_preserves_windows_arguments() -> Result<(), Box<dyn std::error::Error>> {
    // Include spaces in the output path to check CMD argument boundaries too.
    let dir = tempfile::tempdir()?;
    let output = dir.path().join("quoted arguments.json");
    let executable = std::env::current_exe()?;
    let options = SkimOptionsBuilder::default().multi(true).build()?;
    let mut harness = common::insta::enter_items(["hello world", "foo bar"], options)?;

    harness.action(Action::SelectAll)?;
    harness.type_str("he wo")?;
    assert_eq!(harness.skim.app().item_list.selected().unwrap().text(), "hello world");

    // `@` prevents CMD /c from stripping the executable's opening quote.
    let command = format!(
        "@\"{}\" --exact capture_quoted_arguments -- {CHILD_MARKER} \"{}\" {{+}} {{}} {{q}}",
        executable.display(),
        output.display(),
    );
    harness.action(Action::ExecuteSilent(command))?;

    let deadline = Instant::now() + Duration::from_secs(10);
    let actual: Vec<String> = loop {
        if let Ok(bytes) = fs::read(&output) {
            if let Ok(args) = serde_json::from_slice(&bytes) {
                break args;
            }
        }
        assert!(Instant::now() < deadline, "execute-silent did not record its arguments");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(actual, ["hello world", "foo bar", "hello world", "he wo"]);
    Ok(())
}
