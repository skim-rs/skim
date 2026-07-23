//! Zellij-backed end-to-end test harness.
//!
//! This is a drop-in replacement for the old tmux-based harness. It drives a
//! real `sk` process running inside a [Zellij](https://zellij.dev) pane and
//! observes it exactly like a user would: by sending keystrokes and reading the
//! rendered screen back.
//!
//! Unlike tmux, Zellij has no detached-server model, so the harness spawns a
//! Zellij client attached to an in-process pseudo-terminal (via `portable-pty`,
//! which uses `openpty` on Unix and ConPTY on Windows). The harness is
//! cross-platform (Linux, macOS and Windows). A few details are load-bearing for
//! the non-Linux runners:
//! - the pane's shell is resolved to an absolute `bash` path (the Zellij
//!   server's own environment may not have `bash` on `PATH`);
//! - `ZELLIJ_SOCKET_DIR` is forced to a short path so the session's unix socket
//!   path stays under the OS cap (macOS's default `$TMPDIR` is too long — see
//!   [`zellij_socket_dir`]);
//! - the drain thread answers the client's cursor-position report (`ESC[6n`),
//!   which the Windows ConPTY client blocks on to learn the terminal size;
//! - [`ZellijController::wait_ready`] nudges the client's terminal size until the
//!   server hands the pane a non-zero geometry to render into.
//!
//! The pure-harness e2e tests (`interactive.rs`) therefore run on all three
//! platforms; the tests that additionally install POSIX mock binaries
//! (`execute.rs`, `popup.rs`, `listen.rs`) stay `#![cfg(unix)]` for that reason,
//! not the multiplexer's.
//!
//! The public surface (`ZellijController`, `Keys`, `wait`, `sk`, the `sk_test!`
//! DSL and the `line!`/`keys!`/`out!` helpers) mirrors the previous tmux
//! harness so existing tests port over unchanged.

use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{BufReader, ErrorKind, Read, Result, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use rand::RngExt as _;
use rand::distr::Alphanumeric;
use tempfile::{NamedTempFile, TempDir, tempdir};
use which::which;

use crate::common::{SK, SKIM_ENV_REMOVES, SKIM_SHELL_ENV_CLEAR};

/// Build the minimal Zellij config used for every test session.
///
/// - startup tips / release notes off: they spawn a floating plugin pane that
///   steals focus, so keys and screen dumps would target the wrong pane.
/// - pane frames / mouse off: keep the captured pane free of decorations.
/// - kitty keyboard protocol off: with it on, `sk` negotiates the CSI-u key
///   encoding and ignores the legacy escape sequences the harness injects, so
///   arrow keys (Up/Down/…) would silently do nothing.
/// - `bash` as the default shell: the tests drive it with POSIX shell syntax
///   (`echo -n -e`, `printf`, pipelines). Bash ships with the macOS and Windows
///   CI runners as well, keeping the harness cross-platform. We resolve it to an
///   absolute path via `which` and hand that to Zellij: the Zellij *server*
///   spawns the pane's shell from its own environment, which on the Windows and
///   macOS runners does not necessarily have `bash` on `PATH`, so a bare
///   `default_shell "bash"` would leave the pane with no shell and nothing to
///   render. Backslashes are forward-slashed so the path survives KDL's string
///   escaping on Windows.
fn zellij_config() -> String {
    let shell = which("bash")
        .ok()
        .and_then(|p| p.to_str().map(|s| s.replace('\\', "/")))
        .unwrap_or_else(|| "bash".to_string());
    format!(
        r#"show_startup_tips false
show_release_notes false
pane_frames false
mouse_mode false
session_serialization false
support_kitty_keyboard_protocol false
default_shell "{shell}"
"#
    )
}

/// Directory Zellij should place its per-session unix sockets in, exported as
/// `ZELLIJ_SOCKET_DIR` on every `zellij` invocation.
///
/// A unix-domain socket path is length-capped by the OS (~104 bytes on macOS,
/// 108 on Linux), and Zellij's socket path is `<this dir>/<protocol-version>/
/// <session name>`. Zellij's default base is `$TMPDIR/zellij-<uid>`, and on the
/// macOS runners `$TMPDIR` is a long `/var/folders/…` path that leaves ~0 bytes
/// for the session name — Zellij then rejects *every* session with "session name
/// must be less than 0 characters" (zellij-org/zellij#4211) and the client exits
/// before it attaches. Forcing a short base (`/tmp`) keeps the whole path well
/// under the cap on Linux and macOS alike.
fn zellij_socket_dir() -> Result<std::path::PathBuf> {
    #[cfg(unix)]
    let dir = std::path::PathBuf::from("/tmp/skim-zj");
    #[cfg(not(unix))]
    let dir = std::env::temp_dir().join("skim-zj");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Shell prompt installed once the pane comes up. It is deterministic (so the
/// harness can detect readiness) and always contains a literal `$` regardless
/// of the user, matching the assumptions the ported tests make about a returned
/// shell prompt.
const PROMPT: &str = "skim$ ";

/// Build the shell command that runs `sk` with `opts`, clears the `SKIM_*`
/// environment inline, and atomically writes its selection to `outfile` (via a
/// `.part` rename) so readers never observe a half-written result file.
pub fn sk(outfile: &str, opts: &[&str]) -> String {
    format!(
        "{}{} {} > {}.part; mv {}.part {}",
        SKIM_SHELL_ENV_CLEAR,
        SK,
        opts.join(" "),
        outfile,
        outfile,
        outfile
    )
}

/// Longest a single `zellij` CLI invocation may take before it is killed. Well
/// above a healthy call (tens of ms) but bounded, so a wedged Zellij server
/// (e.g. after a session was force-killed) surfaces as a retryable error
/// instead of blocking the test forever.
const ZELLIJ_CMD_TIMEOUT: Duration = Duration::from_secs(8);

/// Overall wall-clock budget for a [`wait`] loop. Generous enough for a slow
/// machine under load, bounded so a genuinely failing assertion (or a dead
/// session) fails in seconds rather than minutes.
const WAIT_BUDGET: Duration = Duration::from_secs(20);

/// Budget for the pane's *first* render (see [`ZellijController::wait_ready`]).
/// Larger than [`WAIT_BUDGET`]: on a cold macOS/Windows runner the Zellij server
/// extracts its bundled plugin assets and starts a shell on first use, which can
/// take considerably longer than a warm session's steady-state calls. Bounded so
/// a genuinely dead session still fails within a minute rather than hanging.
const FIRST_RENDER_BUDGET: Duration = Duration::from_secs(60);

/// Cap on how many bytes of the Zellij client's PTY output we retain for
/// diagnostics (the tail is the most useful part — recent errors, the shell
/// prompt, etc.).
const CLIENT_OUTPUT_CAP: usize = 64 * 1024;

/// Device Status Report requesting the cursor position (`ESC [ 6 n`). The Zellij
/// client emits this to probe the terminal; on the Windows ConPTY it *blocks*
/// until it receives the reply below (on Unix the size comes from the PTY ioctl,
/// so it never waits). We are the terminal, so the drain thread answers it.
const DSR_CPR_REQUEST: &[u8] = b"\x1b[6n";

/// Our reply to [`DSR_CPR_REQUEST`]: a Cursor Position Report placing the cursor
/// at the bottom-right of the 24x80 pane (`ESC [ 24 ; 80 R`), i.e. reporting a
/// 24x80 terminal to the client's size probe.
const CPR_REPLY: &[u8] = b"\x1b[24;80R";

/// How many times to (re)spawn the Zellij session before giving up. Zellij's
/// client/server handshake is occasionally racy at startup — the client dies
/// with "Received empty unknown from server" and the session never renders. This
/// is rare on Linux but frequent on the cold macOS/Windows runners, so a fresh
/// session is spawned instead of failing the test outright.
const SESSION_SPAWN_ATTEMPTS: usize = 4;

/// Poll `pred` until it succeeds or [`WAIT_BUDGET`] elapses. On timeout the most
/// recent error returned by `pred` is surfaced, so a persistent failure keeps its
/// diagnostic cause.
pub fn wait<F, T>(pred: F) -> Result<T>
where
    F: Fn() -> Result<T>,
{
    let deadline = Instant::now() + WAIT_BUDGET;
    loop {
        match pred() {
            Ok(t) => return Ok(t),
            Err(e) => {
                if Instant::now() >= deadline {
                    return Err(e);
                }
            }
        }
        sleep(Duration::from_millis(10));
    }
}

/// Run a `Command`, returning its captured stdout/stderr. The child is killed and
/// a `TimedOut` error returned if it does not finish within `timeout`. Output
/// pipes are drained on threads so a chatty process cannot deadlock on a full
/// pipe, and the child/readers are always torn down before returning — including
/// on a `try_wait` failure.
///
/// The exit status is intentionally *not* turned into an error: some `zellij
/// action` calls exit non-zero in transient states (e.g. while an inline `sk`
/// tears its viewport down) yet still return usable output, so callers rely on
/// [`ZellijController::action`]'s content checks instead.
fn output_with_timeout(mut cmd: Command, timeout: Duration) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let mut out = child.stdout.take().expect("piped stdout");
    let mut err = child.stderr.take().expect("piped stderr");
    let out_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out.read_to_end(&mut buf);
        buf
    });
    let err_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = err.read_to_end(&mut buf);
        buf
    });

    let deadline = Instant::now() + timeout;
    // Some(status) => the child exited; None => it was killed for timing out.
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => sleep(Duration::from_millis(20)),
            Err(e) => {
                // Can't poll the child; don't leave it or the reader threads dangling.
                let _ = child.kill();
                let _ = child.wait();
                let _ = out_handle.join();
                let _ = err_handle.join();
                return Err(e);
            }
        }
    };

    let stdout = out_handle.join().unwrap_or_default();
    let stderr = err_handle.join().unwrap_or_default();
    match status {
        None => Err(std::io::Error::new(ErrorKind::TimedOut, "zellij command timed out")),
        Some(_) => Ok((stdout, stderr)),
    }
}

/// A keystroke (or run of characters) to inject into the pane. Each variant
/// encodes to the raw terminal bytes a real terminal would send (see
/// [`Keys::encode`]): literal text, a single char, a modified key
/// (`Ctrl`/`Alt`), or a named special key.
pub enum Keys<'a> {
    Str(&'a str),
    Key(char),
    Ctrl(&'a Keys<'a>),
    Alt(&'a Keys<'a>),
    Enter,
    Tab,
    BTab,
    Left,
    Right,
    BSpace,
    Up,
    Down,
    Escape,
}

impl Display for Keys<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        use Keys::*;
        match self {
            Str(s) => write!(f, "{}", s),
            Key(c) => write!(f, "{}", c),
            Ctrl(k) => write!(f, "C-{}", k),
            Alt(k) => write!(f, "M-{}", k),
            Enter => write!(f, "Enter"),
            Tab => write!(f, "Tab"),
            BTab => write!(f, "BTab"),
            Left => write!(f, "Left"),
            Right => write!(f, "Right"),
            BSpace => write!(f, "BSpace"),
            Up => write!(f, "Up"),
            Down => write!(f, "Down"),
            Escape => write!(f, "Escape"),
        }
    }
}

impl Keys<'_> {
    /// The raw bytes a terminal would deliver for this key, appended to `out`.
    fn encode(&self, out: &mut Vec<u8>) {
        use Keys::*;
        match self {
            Str(s) => out.extend_from_slice(s.as_bytes()),
            Key(c) => {
                let mut buf = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
            Ctrl(inner) => {
                let mut inner_bytes = Vec::new();
                inner.encode(&mut inner_bytes);
                // Ctrl masks the low 5 bits of the (upper-cased) ASCII byte.
                if let Some(&b) = inner_bytes.first() {
                    out.push(b.to_ascii_uppercase() & 0x1f);
                }
            }
            Alt(inner) => {
                out.push(0x1b);
                inner.encode(out);
            }
            Enter => out.push(b'\r'),
            Tab => out.push(b'\t'),
            BTab => out.extend_from_slice(&[0x1b, b'[', b'Z']),
            Left => out.extend_from_slice(&[0x1b, b'[', b'D']),
            Right => out.extend_from_slice(&[0x1b, b'[', b'C']),
            BSpace => out.push(0x7f),
            Up => out.extend_from_slice(&[0x1b, b'[', b'A']),
            Down => out.extend_from_slice(&[0x1b, b'[', b'B']),
            Escape => out.push(0x1b),
        }
    }
}

/// Drives a single `sk` process inside its own Zellij session.
///
/// The `window` field is retained (holding the Zellij session name) so the
/// public shape matches the previous tmux controller.
pub struct ZellijController {
    pub window: String,
    pub tempdir: TempDir,
    pub outfile: Option<String>,
    // The client process and its master PTY must stay alive for the session's
    // lifetime: dropping the master hangs up the pane and tears the session
    // down early.
    master: Box<dyn MasterPty + Send>,
    child: Option<Box<dyn portable_pty::Child + Send + Sync>>,
    // Rolling tail of the Zellij client's PTY output, captured by the drain
    // thread. Purely for diagnostics: when the pane fails to come up on a runner
    // we can't reproduce locally, its tail (shell errors, plugin failures) is
    // surfaced in the timeout error.
    client_output: Arc<Mutex<Vec<u8>>>,
    // Set by the drain thread when the client's PTY reaches EOF, i.e. the Zellij
    // client process has exited. Lets `wait_ready` abort a dead session fast
    // (and respawn) instead of waiting out the whole render budget.
    client_exited: Arc<AtomicBool>,
}

fn io_err<E: std::fmt::Display>(e: E) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

impl ZellijController {
    fn zellij_bin() -> std::path::PathBuf {
        which("zellij").expect("Please install zellij (>= 0.44) to $PATH")
    }

    /// Run `zellij <args>` with no session targeting and return stdout lines.
    pub fn run(args: &[&str]) -> Result<Vec<String>> {
        let mut cmd = Command::new(Self::zellij_bin());
        cmd.env("ZELLIJ_SOCKET_DIR", zellij_socket_dir()?).args(args);
        let (stdout, _) = output_with_timeout(cmd, ZELLIJ_CMD_TIMEOUT)?;
        Ok(String::from_utf8_lossy(&stdout).lines().map(str::to_string).collect())
    }

    /// Run `zellij --session <this> action <args>` and return stdout as a String.
    ///
    /// Returns an error while the session/pane is not yet available (or if the
    /// call is killed for taking too long) so callers wrapped in [`wait`] retry
    /// instead of observing a bogus "no session" message as pane content or
    /// hanging on a wedged server.
    fn action(&self, args: &[&str]) -> Result<String> {
        let mut cmd = Command::new(Self::zellij_bin());
        cmd.env("ZELLIJ_SOCKET_DIR", zellij_socket_dir()?)
            .args(["--session", &self.window, "action"])
            .args(args);
        let (stdout_bytes, stderr_bytes) = output_with_timeout(cmd, ZELLIJ_CMD_TIMEOUT)?;
        let stdout = String::from_utf8_lossy(&stdout_bytes).to_string();
        let stderr = String::from_utf8_lossy(&stderr_bytes);
        if stderr.contains("not found") || stdout.contains("no active session") {
            return Err(std::io::Error::new(ErrorKind::NotConnected, "zellij session not ready"));
        }
        Ok(stdout)
    }

    /// Inject raw terminal input bytes into the focused pane.
    fn write_bytes(&self, bytes: &[u8]) -> Result<()> {
        let mut args: Vec<String> = vec!["write".to_string()];
        args.extend(bytes.iter().map(|b| b.to_string()));
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        self.action(&refs)?;
        Ok(())
    }

    /// Create a controller running its own Zellij session named after `name`
    /// (plus a random suffix for uniqueness), with the pane's shell brought to a
    /// clean, deterministic prompt. The session is torn down on drop.
    ///
    /// Zellij's startup handshake is occasionally racy (see
    /// [`SESSION_SPAWN_ATTEMPTS`]); a session that dies before it renders is torn
    /// down and a fresh one is spawned, up to that many attempts.
    pub fn new_named(name: &str) -> Result<Self> {
        // Keep session names short: Zellij caps them (~36 chars) and they also
        // count against the unix socket path budget (see `zellij_socket_dir`).
        // Drop non-alphanumerics (Zellij rejects most punctuation) and truncate;
        // the random suffix added in `spawn_once` keeps them unique regardless.
        let sanitized: String = name.chars().filter(char::is_ascii_alphanumeric).take(10).collect();

        let mut last_err: Option<std::io::Error> = None;
        for _ in 0..SESSION_SPAWN_ATTEMPTS {
            match Self::spawn_once(&sanitized) {
                Ok(controller) => return Ok(controller),
                Err(e) => last_err = Some(e),
            }
            // The failed attempt's controller has been dropped here, tearing the
            // dead session down before the next attempt spawns a fresh one.
        }
        Err(last_err.unwrap_or_else(|| std::io::Error::new(ErrorKind::Other, "failed to start zellij session")))
    }

    /// Spawn a single Zellij session + client and bring it to a ready prompt.
    /// Returns the wired-up controller on success; on failure the transient
    /// controller is dropped (cleaning up the session) and the error is returned
    /// so [`new_named`](Self::new_named) can retry with a fresh session.
    fn spawn_once(sanitized: &str) -> Result<Self> {
        let suffix: String = rand::rng().sample_iter(&Alphanumeric).take(6).map(char::from).collect();
        // Kept short on purpose — see `new_named` and `zellij_socket_dir`.
        let session = if sanitized.is_empty() {
            format!("sk_{suffix}")
        } else {
            format!("sk_{sanitized}_{suffix}")
        };

        let tempdir = tempdir()?;
        let config = tempdir.path().join("zellij.kdl");
        std::fs::write(&config, zellij_config())?;

        let pty = native_pty_system();
        let pair = pty
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(io_err)?;

        let mut cmd = CommandBuilder::new(Self::zellij_bin());
        cmd.args([
            "--config",
            config.to_str().expect("config path is not valid UTF-8"),
            "--session",
            &session,
            "attach",
            "--create",
            &session,
        ]);
        // Match the env hygiene the shell wrapper applies inline, so nothing
        // leaks in from the outer test environment via the session server.
        for var in SKIM_ENV_REMOVES {
            cmd.env_remove(var);
        }
        // Keep the session's unix socket path short (see `zellij_socket_dir`).
        cmd.env("ZELLIJ_SOCKET_DIR", zellij_socket_dir()?);
        if let Ok(dir) = std::env::current_dir() {
            cmd.cwd(dir);
        }

        let child = pair.slave.spawn_command(cmd).map_err(io_err)?;
        // The child keeps its own handles to the slave; drop ours.
        drop(pair.slave);

        // Continuously drain the client's output or its PTY buffer fills and the
        // Zellij client blocks (freezing rendering and input handling). We keep a
        // bounded tail of that output for diagnostics (see `client_output`).
        let mut reader = pair.master.try_clone_reader().map_err(io_err)?;
        let mut writer = pair.master.take_writer().map_err(io_err)?;
        let client_output = Arc::new(Mutex::new(Vec::new()));
        let client_exited = Arc::new(AtomicBool::new(false));
        let sink = Arc::clone(&client_output);
        let exited = Arc::clone(&client_exited);
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            // Rolling window so a DSR request split across two reads is still
            // matched. Only needs to span one marker's worth of bytes.
            let mut window: Vec<u8> = Vec::new();
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let chunk = &buf[..n];
                if let Ok(mut tail) = sink.lock() {
                    tail.extend_from_slice(chunk);
                    let overflow = tail.len().saturating_sub(CLIENT_OUTPUT_CAP);
                    if overflow > 0 {
                        tail.drain(0..overflow);
                    }
                }
                // Reply to every cursor-position report the client requests, or
                // it blocks forever on the Windows ConPTY and never renders.
                window.extend_from_slice(chunk);
                while let Some(pos) = window.windows(DSR_CPR_REQUEST.len()).position(|w| w == DSR_CPR_REQUEST) {
                    let _ = writer.write_all(CPR_REPLY);
                    let _ = writer.flush();
                    window.drain(0..pos + DSR_CPR_REQUEST.len());
                }
                // Retain a marker-minus-one tail in case the next request
                // straddles the read boundary.
                let keep = DSR_CPR_REQUEST.len().saturating_sub(1);
                if window.len() > keep {
                    let cut = window.len() - keep;
                    window.drain(0..cut);
                }
            }
            // PTY EOF: the Zellij client has exited (cleanly or by dying).
            exited.store(true, Ordering::SeqCst);
        });

        let controller = Self {
            window: session,
            tempdir,
            outfile: None,
            master: pair.master,
            child: Some(child),
            client_output,
            client_exited,
        };
        controller.wait_ready()?;
        Ok(controller)
    }

    /// Like [`new_named`](Self::new_named) but with a random session name.
    pub fn new() -> Result<Self> {
        let name: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();
        Self::new_named(&name)
    }

    /// Bytes we want the pane sized to for the tests (mirrors the initial
    /// `openpty`). A screen dump reflects the *pane* geometry, which the Zellij
    /// server derives from the attached client's terminal size.
    const ROWS: u16 = 24;
    const COLS: u16 = 80;

    /// Ask the client PTY to report `cols`×[`ROWS`]. Resizing (even to the same
    /// size) makes `portable-pty` deliver a `SIGWINCH`/ConPTY resize to the
    /// attached Zellij client, which then (re-)reports its geometry to the
    /// server. On the macOS and Windows runners the client's *initial* size can
    /// be lost, leaving the pane at zero size with nothing to render; nudging it
    /// forces a non-zero geometry so the pane actually draws.
    fn resize_client(&self, cols: u16) {
        let _ = self.master.resize(PtySize {
            rows: Self::ROWS,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
    }

    /// A best-effort, human-readable tail of what the Zellij client has printed
    /// to its PTY. Used to explain first-render timeouts on runners we can't
    /// reproduce locally.
    fn client_output_tail(&self) -> String {
        self.client_output
            .lock()
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default()
    }

    /// Block until the session server and pane are up and a clean, deterministic
    /// shell prompt is showing.
    fn wait_ready(&self) -> Result<()> {
        // Session server + pane are up once a screen dump comes back non-empty.
        // Give this its own (longer) budget for cold-start runners, and on each
        // empty poll nudge the client's terminal size so the server always has a
        // non-zero pane geometry to render into.
        let deadline = Instant::now() + FIRST_RENDER_BUDGET;
        let mut wide = false;
        loop {
            if let Ok(dump) = self.action(&["dump-screen"]) {
                if !dump.trim().is_empty() {
                    break;
                }
            }
            // A dead client will never render; fail fast so `new_named` respawns.
            if self.client_exited.load(Ordering::SeqCst) {
                return Err(std::io::Error::new(
                    ErrorKind::ConnectionReset,
                    format!(
                        "zellij client exited before the pane rendered. output tail:\n{}",
                        self.client_output_tail()
                    ),
                ));
            }
            if Instant::now() >= deadline {
                return Err(std::io::Error::new(
                    ErrorKind::WouldBlock,
                    format!(
                        "pane not rendered within {:?}. zellij client output tail:\n{}",
                        FIRST_RENDER_BUDGET,
                        self.client_output_tail()
                    ),
                ));
            }
            // Toggle the width so every poll delivers a fresh resize event.
            self.resize_client(if wide { Self::COLS + 1 } else { Self::COLS });
            wide = !wide;
            sleep(Duration::from_millis(100));
        }
        // Settle on the canonical size before the tests read the pane.
        self.resize_client(Self::COLS);

        // Install a deterministic prompt and clear the screen. The keys are
        // buffered by the pane PTY even if bash is still starting up. Re-send on
        // each retry in case the very first keystrokes raced shell startup.
        let deadline = Instant::now() + FIRST_RENDER_BUDGET;
        loop {
            self.write_bytes(format!("unset PROMPT_COMMAND HISTFILE HISTCONTROL; PS1='{PROMPT}'; clear\r").as_bytes())?;
            let lines = self.capture()?;
            if lines.first().is_some_and(|l| l.starts_with(PROMPT.trim_end()))
                && !lines.iter().any(|l| l.contains("PROMPT_COMMAND"))
            {
                return Ok(());
            }
            if self.client_exited.load(Ordering::SeqCst) {
                return Err(std::io::Error::new(
                    ErrorKind::ConnectionReset,
                    format!(
                        "zellij client exited before the prompt came up. output tail:\n{}",
                        self.client_output_tail()
                    ),
                ));
            }
            if Instant::now() >= deadline {
                return Err(std::io::Error::new(
                    ErrorKind::WouldBlock,
                    format!(
                        "shell prompt did not come up within {:?}. zellij client output tail:\n{}",
                        FIRST_RENDER_BUDGET,
                        self.client_output_tail()
                    ),
                ));
            }
            sleep(Duration::from_millis(50));
        }
    }

    /// Send a sequence of keystrokes to the pane, each encoded to the raw
    /// terminal bytes a real terminal would deliver.
    pub fn send_keys(&self, keys: &[Keys]) -> std::io::Result<()> {
        print!("typing `");
        for key in keys {
            let mut bytes = Vec::new();
            key.encode(&mut bytes);
            self.write_bytes(&bytes)?;
            print!("{}", key);
        }
        println!("`");
        Ok(())
    }

    /// Allocate a fresh temp file path inside this controller's tempdir.
    pub fn tempfile(&self) -> Result<String> {
        Ok(NamedTempFile::new_in(&self.tempdir)?
            .path()
            .to_str()
            .ok_or_else(|| std::io::Error::new(ErrorKind::InvalidData, "temp file path is not valid UTF-8"))?
            .to_string())
    }

    /// Screen contents of the pane, most-recent (bottom) line first.
    ///
    /// The reversed ordering mirrors the old tmux harness so existing tests
    /// index the same way: `capture()[0]` is the bottom line, where `sk` draws
    /// its query prompt in the default (bottom-anchored) layout.
    pub fn capture(&self) -> Result<Vec<String>> {
        let dump = wait(|| self.action(&["dump-screen"]))?;
        Ok(Self::to_lines(&dump))
    }

    /// Like [`capture`], but preserves ANSI styling (colors / attributes).
    pub fn capture_colored(&self) -> Result<Vec<String>> {
        let dump = wait(|| self.action(&["dump-screen", "--ansi"]))?;
        Ok(Self::to_lines(&dump))
    }

    fn to_lines(dump: &str) -> Vec<String> {
        dump.trim()
            .split('\n')
            .map(str::to_string)
            .rev()
            .collect::<Vec<String>>()
    }

    /// Poll [`capture`](Self::capture) until `pred` accepts the screen contents,
    /// erroring with the last captured screen if the budget elapses first.
    pub fn until<F>(&self, pred: F) -> std::io::Result<()>
    where
        F: Fn(&[String]) -> bool,
    {
        match wait(|| {
            let lines = self.capture()?;
            if pred(&lines) {
                return Ok(true);
            }
            Err(std::io::Error::other("pred not matched"))
        }) {
            Ok(true) => Ok(()),
            Ok(false) => Err(std::io::Error::other(self.capture()?.join("\n"))),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                self.capture()?.join("\n"),
            )),
        }
    }

    /// Capture skim output without ANSI sequences
    pub fn output(&self) -> Result<Vec<String>> {
        if let Some(ref outfile) = self.outfile {
            self.output_from(outfile)
        } else {
            Err(std::io::Error::new(
                ErrorKind::NotFound,
                "You need to use start_sk to get an outfile",
            ))
        }
    }

    /// Capture skim output from explicit outfile path
    pub fn output_from(&self, outfile: &str) -> Result<Vec<String>> {
        wait(|| {
            if Path::new(&outfile).exists() {
                Ok(())
            } else {
                Err(std::io::Error::new(ErrorKind::NotFound, "outfile does not exist yet"))
            }
        })?;
        let mut string_lines = String::new();
        BufReader::new(File::open(outfile)?).read_to_string(&mut string_lines)?;

        let str_lines = string_lines.trim();
        Ok(str_lines
            .split("\n")
            .map(|s| s.to_string())
            .collect::<Vec<String>>()
            .into_iter()
            .collect())
    }

    /// Launch `sk` in the pane with `opts`, optionally piping `stdin_cmd` into
    /// it, recording the result file so [`output`](Self::output) can read it.
    /// Returns the path of that output file.
    pub fn start_sk(&mut self, stdin_cmd: Option<&str>, opts: &[&str]) -> Result<String> {
        let outfile = self.tempfile()?;
        let sk_cmd = sk(&outfile, opts);
        let cmd = match stdin_cmd {
            Some(s) => format!("{} | {}", s, sk_cmd),
            None => sk_cmd,
        };
        println!("--- starting up sk ---");
        self.send_keys(&[Keys::Str(&cmd), Keys::Enter])?;
        println!("--- sk is running  ---");
        self.outfile = Some(outfile.clone());
        Ok(outfile)
    }
}

impl Drop for ZellijController {
    fn drop(&mut self) {
        // Kill the client first, then remove the (now dead) session.
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
        }
        let _ = Self::run(&["delete-session", &self.window, "--force"]);
    }
}
// ============================================================================
// sk_test! - Macro for writing compact tmux-based integration tests
// ============================================================================
//
// USAGE GUIDE
// -----------
//
// 1. INPUT SYNTAX:
//    - Echo string:  "a\\nb\\nc"      -> Runs: echo -n -e 'a\nb\nc'
//    - Command:      @cmd "seq 1 100" -> Runs: seq 1 100 (pipe to sk)
//
// 2. DSL SYNTAX (Only syntax supported):
//
// sk_test!(test_name, "input", &["--opts"], {
//   @capture[0] eq(">");                     // Wait until capture[0] == ">"
//   @capture[1] trim().starts_with("3/3");   // Wait until capture[1].trim().starts_with("3/3")
//   @capture[-1] eq("foo");                  // Wait until last line == "foo"
//   @capture[*] contains("bar");             // Wait until any line contains "bar"
//   @output[0] eq("result");                 // Wait until output[0] == "result"
//   @output[-1] eq("last");                  // Wait until output[-1] (last line) == "last"
//   @output[*] starts_with("prefix");        // Wait until any output line starts with "prefix"
//   @capture_colored[0] contains("\x1b");    // Wait until colored capture contains ANSI
//   @lines |l| (l.len() > 5);                // Complex assertion with closure
//   @keys Enter, Tab;                        // Send multiple keys
//   @dbg;                                    // Debug print current capture
// });
//
// NOTE: All methods use wait() for consistent retry behavior. Any ZellijController
//       method that takes no args and returns Result<Vec<String>> can be used:
//       capture, output, capture_colored, etc.
//
// EXAMPLES
// --------
//
// Example 1: Simple test with echo input (all methods wait/retry)
//   sk_test!(simple, "a\\nb\\nc", &[], {
//     @capture[0] eq(">");        // Waits until condition is met
//     @keys Enter;
//     @output[0] eq("a");          // Waits until output is available
//   });
//
// Example 2: Using command input with @cmd
//   sk_test!(with_seq, @cmd "seq 1 10", &["--bind", "'ctrl-t:toggle-all'"], {
//     @capture[0] eq(">");
//     @keys Ctrl(&Key('t'));
//     @capture[2] eq(">>1");
//   });
//
// Example 3: Complex closures with @lines
//   sk_test!(complex, "apple\\nbanana", &[], {
//     @lines |l| (l.len() > 4);
//     @keys Str("ana");
//     @lines |l| (l.iter().any(|x| x.contains("banana")));
//   });
//
// Example 4: Method chaining
//   sk_test!(chaining, "  foo  \\n  bar  ", &[], {
//     @capture[2] trim().eq("foo");
//     @keys Enter;
//     @output[0] trim().eq("foo");
//   });
//
// Example 5: Using wildcards and negative indices
//   sk_test!(wildcards, "apple\\nbanana\\ncherry", &[], {
//     @capture[*] contains("3/3");           // Any line contains "3/3"
//     @capture[-1] starts_with(">");         // Last line starts with ">"
//     @keys Str("ana");
//     @capture[*] contains("banana");        // Any line contains "banana"
//     @keys Enter;
//     @output[0] eq("banana");               // First output line
//     @output[-1] eq("banana");              // Last output line
//     @output[*] starts_with("b");           // Any output line starts with "b"
//   });
//
// Example 6: New array syntax test
//   sk_test!(new_syntax_test, "foo\\nbar\\nbaz", &[], {
//     @capture[0] starts_with(">");
//     @capture[1] contains("3/3");
//     @keys Enter;
//     @output[0] eq("foo");
//     @output[- 1] eq("foo");
//   });
//
// Example 7: Wildcard syntax test
//   sk_test!(wildcard_syntax_test, "apple\\nbanana\\ncherry", &[], {
//     @capture[*] contains("3/3");
//     @keys Str("ana");
//     @capture[*] contains("banana");
//     @keys Enter;
//     @output[*] eq("banana");
//   });
//
// Example 8: Comprehensive example showing all features
//   sk_test!(comprehensive_example, "foo\\nbar\\nbaz\\nqux", &[], {
//     // Positive index with simple method
//     @capture[0] starts_with(">");
//
//     // Positive index with method chain
//     @capture[1] trim().contains("4/4");
//
//     // Wildcard - check if any line matches
//     @capture[*] contains("foo");
//
//     // Send keys
//     @keys Str("ba");
//
//     // Negative index - last line
//     @capture[- 1] contains("bar");
//
//     // Select first match
//     @keys Enter;
//
//     // Output assertions
//     @output[0] eq("bar");           // First output line
//     @output[- 1] eq("bar");         // Last output line
//     @output[*] starts_with("b");    // Any output line starts with "b"
//   });
//
// Example 9: Using capture_colored for ANSI escape sequences
//   sk_test!(ansi_test, @cmd "echo -e '\\x1b[31mred\\x1b[0m'", &["--ansi"], {
//     @capture[*] contains("red");
//     @capture_colored[*] contains("\x1b[31m");  // Check for ANSI codes
//     @keys Enter;
//   });
//
// DSL COMMAND REFERENCE
// ---------------------
// @METHOD[N] method_chain     Wait until METHOD[N].method_chain is true (N = line number)
// @METHOD[-N] method_chain    Wait until METHOD[-N].method_chain is true (negative index)
// @METHOD[*] method_chain     Wait until any line matches (uses .iter().any())
//   where METHOD is any ZellijController method returning Result<Vec<String>>:
//     - capture: Wait until condition is true
//     - output: Wait until condition is true
//     - capture_colored: Wait until condition is true on colored capture
//   All methods use wait() for consistent retry behavior
// @lines |l| (expr)           Call tmux.until(|l| expr)? with closure
// @keys key1, key2            Send keys (automatically adds ?)
// @dbg                        Debug print current capture
//
// NOTES
// -----
// - The `tmux` variable is implicitly available in DSL blocks
// - All variants automatically handle Result propagation and Ok(()) return
// - DSL closures must be wrapped in parentheses: |l| (expr)
// - Method chains support any String/&str method: eq(), starts_with(), contains(), trim(), etc.
// - You can chain methods: trim().starts_with("foo")
// - Negative indices work like Python: -1 is last element, -2 is second-to-last, etc.
// - ALL methods use wait() with retry logic - no immediate assertions
// - wait() retries every 10ms for up to 10 seconds before timing out
//
#[allow(unused_macros)]
macro_rules! sk_test {
    // Standard variant with echo input: explicit variable name with block
    ($name:tt, $input:expr, $options:expr, $tmux:ident => $content:block) => {
      #[test]
      #[allow(unused_variables)]
      fn $name() -> std::io::Result<()> {
        let mut $tmux = crate::common::zellij::ZellijController::new()?;
        $tmux.start_sk(Some(&format!("echo -n -e '{}'", $input)), $options)?;

        $content

        Ok(())
      }
    };

    // Standard variant with arbitrary command: use @cmd marker
    ($name:tt, @cmd $cmd:expr, $options:expr, $tmux:ident => $content:block) => {
      #[test]
      #[allow(unused_variables)]
      fn $name() -> std::io::Result<()> {
        let mut $tmux = crate::common::zellij::ZellijController::new()?;
        $tmux.start_sk(Some($cmd), $options)?;

        $content

        Ok(())
      }
    };

    // DSL variant with echo input
    ($name:tt, $input:expr, $options:expr, { $($content:tt)* }) => {
      #[test]
      #[allow(unused_variables)]
      fn $name() -> std::io::Result<()> {
        let mut tmux = crate::common::zellij::ZellijController::new_named(stringify!($name))?;
        tmux.start_sk(Some(&format!("echo -n -e '{}'", $input)), $options)?;

        sk_test!(@expand tmux; $($content)*);

        Ok(())
      }
    };

    // DSL variant with arbitrary command: use @cmd marker
    ($name:tt, @cmd $cmd:expr, $options:expr, { $($content:tt)* }) => {
      #[test]
      #[allow(unused_variables)]
      fn $name() -> std::io::Result<()> {
        let mut tmux = crate::common::zellij::ZellijController::new_named(stringify!($name))?;
        tmux.start_sk(Some($cmd), $options)?;

        sk_test!(@expand tmux; $($content)*);

        Ok(())
      }
    };

    // Token processing rules
    (@expand $tmux:ident; ) => {};

    // Generic method patterns - works with any ZellijController method
    // @method[*] - check if any line matches (uses .iter().any())
    (@expand $tmux:ident; @ $method:ident [ * ] $($rest:tt)*) => {
        sk_test!(@method_any_collect $tmux, $method, [] ; $($rest)*);
    };

    // @method[-idx] for negative index - supports arbitrary method chains (must come before positive)
    (@expand $tmux:ident; @ $method:ident [ - $idx:literal ] $($rest:tt)*) => {
        sk_test!(@method_neg_collect $tmux, $method, $idx, [] ; $($rest)*);
    };

    // @method[idx] for positive index - supports arbitrary method chains
    (@expand $tmux:ident; @ $method:ident [ $idx:literal ] $($rest:tt)*) => {
        sk_test!(@method_pos_collect $tmux, $method, $idx, [] ; $($rest)*);
    };

    // Collect tokens until semicolon for positive index - dispatches to wait or assert
    (@method_pos_collect $tmux:ident, $method:ident, $idx:expr, [$($methods:tt)*] ; ; $($rest:tt)*) => {
        sk_test!(@method_pos_dispatch $tmux, $method, $idx, [$($methods)*]);
        sk_test!(@expand $tmux; $($rest)*);
    };
    (@method_pos_collect $tmux:ident, $method:ident, $idx:expr, [$($methods:tt)*] ; $next:tt $($rest:tt)*) => {
        sk_test!(@method_pos_collect $tmux, $method, $idx, [$($methods)* $next] ; $($rest)*);
    };

    // Dispatch for positive index - all methods use wait()
    (@method_pos_dispatch $tmux:ident, $method:ident, $idx:expr, [$($methods:tt)*]) => {
        {
            if crate::common::zellij::wait(|| {
                let lines = $tmux.$method()?;
                if lines.len() > $idx && lines[$idx].$($methods)* {
                    Ok(true)
                } else {
                    Err(std::io::Error::new(std::io::ErrorKind::Other, "condition not met"))
                }
            }).is_err() {
                let lines = $tmux.$method().unwrap_or_default();
                let actual = if lines.len() > $idx { &lines[$idx] } else { "<no line>" };
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("Timed out waiting for {}[{}].{}, got: {}", stringify!($method), $idx, stringify!($($methods)*), actual)
                ));
            }
        }
    };

    // Collect tokens until semicolon for negative index - dispatches to wait or assert
    (@method_neg_collect $tmux:ident, $method:ident, $idx:expr, [$($methods:tt)*] ; ; $($rest:tt)*) => {
        sk_test!(@method_neg_dispatch $tmux, $method, $idx, [$($methods)*]);
        sk_test!(@expand $tmux; $($rest)*);
    };
    (@method_neg_collect $tmux:ident, $method:ident, $idx:expr, [$($methods:tt)*] ; $next:tt $($rest:tt)*) => {
        sk_test!(@method_neg_collect $tmux, $method, $idx, [$($methods)* $next] ; $($rest)*);
    };

    // Dispatch for negative index - all methods use wait()
    (@method_neg_dispatch $tmux:ident, $method:ident, $idx:expr, [$($methods:tt)*]) => {
        {
            if crate::common::zellij::wait(|| {
                let lines = $tmux.$method()?;
                if $idx > 0 && lines.len() >= $idx {
                    let actual_idx = lines.len() - $idx;
                    if lines[actual_idx].$($methods)* {
                        Ok(true)
                    } else {
                        Err(std::io::Error::new(std::io::ErrorKind::Other, "condition not met"))
                    }
                } else {
                    Err(std::io::Error::new(std::io::ErrorKind::Other, "not enough lines"))
                }
            }).is_err() {
                let lines = $tmux.$method().unwrap_or_default();
                let actual_idx = lines.len().saturating_sub($idx);
                let actual = if $idx > 0 && lines.len() >= $idx { &lines[actual_idx] } else { "<no line>" };
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("Timed out waiting for {}[-{}].{}, got: {}", stringify!($method), $idx, stringify!($($methods)*), actual)
                ));
            }
        }
    };

    // Collect tokens until semicolon for wildcard [*] - dispatches to wait or assert
    (@method_any_collect $tmux:ident, $method:ident, [$($methods:tt)*] ; ; $($rest:tt)*) => {
        sk_test!(@method_any_dispatch $tmux, $method, [$($methods)*]);
        sk_test!(@expand $tmux; $($rest)*);
    };
    (@method_any_collect $tmux:ident, $method:ident, [$($methods:tt)*] ; $next:tt $($rest:tt)*) => {
        sk_test!(@method_any_collect $tmux, $method, [$($methods)* $next] ; $($rest)*);
    };

    // Dispatch for wildcard - all methods use wait()
    (@method_any_dispatch $tmux:ident, $method:ident, [$($methods:tt)*]) => {
        {
            if crate::common::zellij::wait(|| {
                let lines = $tmux.$method()?;
                if lines.iter().any(|line| line.$($methods)*) {
                    Ok(true)
                } else {
                    Err(std::io::Error::new(std::io::ErrorKind::Other, "condition not met"))
                }
            }).is_err() {
                let lines = $tmux.$method().unwrap_or_default();
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("Timed out waiting for {}[*] any line matching .{}, got: {:?}", stringify!($method), stringify!($($methods)*), lines)
                ));
            }
        }
    };

    // @lines command for tmux.until with closure
    (@expand $tmux:ident; @ lines | $param:ident | ( $($body:tt)* ) ; $($rest:tt)*) => {
        $tmux.until(|$param| $($body)*)?;
        sk_test!(@expand $tmux; $($rest)*);
    };

    // @keys command for send_keys - supports any number of keys
    (@expand $tmux:ident; @ keys $($key:expr),+ ; $($rest:tt)*) => {
        send_keys!($tmux, $($key),+)?;
        sk_test!(@expand $tmux; $($rest)*);
    };

    // @dbg command for debug printing
    (@expand $tmux:ident; @ dbg ; $($rest:tt)*) => {
        match $tmux.capture() {
            Ok(lines) => println!("DBG: capture: {:?}", lines),
            Err(e) => println!("DBG: capture failed: {}", e),
        }
        match $tmux.output() {
            Ok(lines) => println!("DBG: output: {:?}", lines),
            Err(e) => println!("DBG: output failed: {}", e),
        }
        sk_test!(@expand $tmux; $($rest)*);
    };

    // Pass through regular Rust statements that access tmux (catch-all, must be last)
    (@expand $tmux:ident; $stmt:stmt ; $($rest:tt)*) => {
        #[allow(redundant_semicolons)]
        {
            $stmt;
            sk_test!(@expand $tmux; $($rest)*);
        }
    };

}

#[allow(unused_macros)]
macro_rules! assert_line {
    ($tmux:ident, $line_nr:literal $($expression:tt)+) => {
      {
      if $tmux.until(|l| l.len() > $line_nr && l[$line_nr] $($expression)+).is_err() {
          let lines = $tmux.capture().unwrap_or_default();
          let actual = if lines.len() > $line_nr { &lines[$line_nr] } else { "<no line>" };
          Err(std::io::Error::new(
              std::io::ErrorKind::TimedOut,
              format!(
                  "Timed out waiting for condition on line {}, got {} but expected it to {}",
                  $line_nr,
                  actual,
                  stringify!($($expression)+)
              ),
          ))
        } else {
          Ok(())
        }
      }?
    };
}

#[allow(unused_macros)]
macro_rules! send_keys {
    ($tmux:ident, $($key:expr),+) => {
      $tmux.send_keys(&[$($key),+])
    };
}

#[allow(unused_macros)]
macro_rules! assert_output_line {
    ($tmux:ident, $line_nr:literal $($expression:tt)+) => {
        let output = $tmux.output()?;
        println!("Output: {output:?}");
        assert!(output[$line_nr] $($expression)+, "Timed out waiting for condition on output line {}, expected it to {}", $line_nr, stringify!($($expression)+));
    };
}

// Ultra-short aliases for compact test writing
// Usage: line!(t, 0 == ">") instead of assert_line!(t, 0 == ">")
#[allow(unused_macros)]
macro_rules! line {
    ($tmux:ident, $line_nr:literal $($expression:tt)+) => {
        assert_line!($tmux, $line_nr $($expression)+)
    };
}

#[allow(unused_macros)]
macro_rules! keys {
    ($tmux:ident, $($key:expr),+) => {
        send_keys!($tmux, $($key),+)
    };
}

#[allow(unused_macros)]
macro_rules! out {
    ($tmux:ident, $line_nr:literal $($expression:tt)+) => {
        assert_output_line!($tmux, $line_nr $($expression)+)
    };
}
