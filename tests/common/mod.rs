#[macro_use]
pub mod insta;
#[macro_use]
#[cfg(unix)]
pub mod tmux;

/// Raw binary path. Use `Command::new(SK)` to spawn directly; apply
/// `SKIM_ENV_REMOVES` via `.env_remove()` on the command when needed.
/// For shell-command strings (e.g. sent to tmux) prepend `SKIM_SHELL_ENV_CLEAR`.
#[cfg(all(unix, debug_assertions, coverage))]
pub static SK: &str = "./target/llvm-cov-target/debug/sk";
#[cfg(all(unix, debug_assertions, not(coverage)))]
pub static SK: &str = "./target/debug/sk";
#[cfg(all(unix, not(debug_assertions), coverage))]
pub static SK: &str = "./target/llvm-cov-target/release/sk";
#[cfg(all(unix, not(debug_assertions), not(coverage)))]
pub static SK: &str = "./target/release/sk";

#[cfg(all(windows, debug_assertions, coverage))]
pub static SK: &str = r".\target\llvm-cov-target\debug\sk.exe";
#[cfg(all(windows, debug_assertions, not(coverage)))]
pub static SK: &str = r".\target\debug\sk.exe";
#[cfg(all(windows, not(debug_assertions), coverage))]
pub static SK: &str = r".\target\llvm-cov-target\release\sk.exe";
#[cfg(all(windows, not(debug_assertions), not(coverage)))]
pub static SK: &str = r".\target\release\sk.exe";

/// Environment variables that sk tests must clear so `SKIM_DEFAULT_OPTIONS`
/// and friends don't leak in from the outer test environment.
pub const SKIM_ENV_REMOVES: &[&str] = &["SKIM_DEFAULT_OPTIONS", "SKIM_DEFAULT_COMMAND", "SKIM_OPTIONS_FILE"];

/// Shell-level env-clearing prefix for embedding sk in a shell command string
/// (e.g. commands sent to a tmux pane via `send-keys`).
#[cfg(unix)]
pub const SKIM_SHELL_ENV_CLEAR: &str = "SKIM_DEFAULT_OPTIONS= SKIM_DEFAULT_COMMAND= SKIM_OPTIONS_FILE= ";
#[cfg(windows)]
pub const SKIM_SHELL_ENV_CLEAR: &str = "";
