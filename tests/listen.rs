//! End-to-end coverage for `--listen` and `--remote`. Individual action behavior
//! is covered in `src/tui/app_tests.rs` without a terminal session.
#![cfg(all(unix, feature = "cli", feature = "listen"))]

#[allow(dead_code, missing_docs)]
#[macro_use]
mod common;

use common::SKIM_ENV_REMOVES;
use common::zellij::{Keys, ZellijController};
use rand::RngExt as _;
use std::io::{self, Write as _};
use std::process::{Command, Stdio};

fn send(name: &str, actions: &str) -> io::Result<()> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sk"));
    for var in SKIM_ENV_REMOVES {
        cmd.env_remove(var);
    }
    let mut child = cmd
        .args(["--remote", name])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    // Close stdin to finish the remote client, and reap it even if the write fails.
    let written = writeln!(child.stdin.take().expect("piped stdin"), "{actions}");
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "sk --remote {name} failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    written
}

fn setup(test: &str) -> io::Result<(ZellijController, String)> {
    let mut zellij = ZellijController::new_named(test)?;
    let name = format!("sk-listen-{:016x}", rand::rng().random::<u64>());
    zellij.start_sk(
        Some("printf 'a\\nb\\nc\\nd\\n'"),
        &["--listen", &name, "--sync", "--multi"],
    )?;
    // Wait for the initial selection, not just the prompt, before sending actions.
    sk_test!(@expand zellij;
        @capture[2]starts_with("> a");
    );
    Ok((zellij, name))
}

#[test]
fn listen_remote_actions() -> io::Result<()> {
    let (zellij, name) = setup("listen-actions")?;
    sk_test!(@expand zellij;
        send(&name, "reload(printf 'x\\ny\\nz\\n')")?;
        @capture[4]trim().eq("z");
    );

    // Select y, move back to x, then accept. The output must use the selection.
    send(&name, "up+select\ndown+accept(ctrl-a)")?;
    assert_eq!(zellij.output()?, ["ctrl-a", "y"]);
    Ok(())
}

#[test]
fn listen_remote_bindings() -> io::Result<()> {
    let (zellij, name) = setup("listen-bindings")?;
    sk_test!(@expand zellij;
        // The header confirms that both changes ran before we send terminal keys.
        send(&name, "bind(ctrl-x:up)+unbind(ctrl-u)+set-header(ready)")?;
        @capture[*]trim().eq("ready");
        @keys Keys::Ctrl(&Keys::Key('x')), Keys::Ctrl(&Keys::Key('x'));
        @capture[*]starts_with("> c");
        @keys Keys::Str("hello"), Keys::Ctrl(&Keys::Key('u')), Keys::Key('x');
        @capture[0]trim().eq("> hellox");
    );

    send(&name, "abort")?;
    assert_eq!(zellij.output()?, Vec::<String>::new());
    Ok(())
}
