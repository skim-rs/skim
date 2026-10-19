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
