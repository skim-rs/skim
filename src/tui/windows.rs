//! Windows-specific console control handler.
//!
//! On Windows, pressing Ctrl-C generates a `CTRL_C_EVENT` that by default
//! terminates the process immediately — before any destructors (and therefore
//! terminal cleanup) can run.  This module installs a custom handler via
//! `SetConsoleCtrlHandler` that performs terminal cleanup (disable mouse
//! capture, bracketed paste, alternate screen, raw mode) and then exits
//! with code 130 (the conventional "interrupted" exit code).

use super::backend::cleanup_terminal;

// Windows console control event constants.
const CTRL_C_EVENT: u32 = 0;
const CTRL_BREAK_EVENT: u32 = 1;
const CTRL_CLOSE_EVENT: u32 = 2;

unsafe extern "system" {
    fn SetConsoleCtrlHandler(handler_routine: Option<unsafe extern "system" fn(u32) -> i32>, add: i32) -> i32;
}

/// Console ctrl handler callback invoked by Windows on `CTRL_C_EVENT` (and
/// other control events like `CTRL_BREAK_EVENT`, `CTRL_CLOSE_EVENT`, etc.).
///
/// The handler cleans up the terminal state so that mouse capture, raw mode,
/// and the alternate screen are properly disabled, then exits with code 130.
///
/// Returning `TRUE` (1) tells Windows that the event has been handled and the
/// default handler (which would call `ExitProcess` without cleanup) should
/// **not** run.
unsafe extern "system" fn ctrl_c_handler(ctrl_type: u32) -> i32 {
    match ctrl_type {
        CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT => {
            let _ = cleanup_terminal();
            // 130 = 128 + SIGINT, the conventional exit code for Ctrl-C.
            std::process::exit(130);
        }
        _ => 0, // Let the next handler deal with it.
    }
}

/// Install our console ctrl handler so that `CTRL_C_EVENT` triggers terminal
/// cleanup instead of an abrupt process termination.
pub(crate) fn install_ctrl_c_handler() {
    // SAFETY: `SetConsoleCtrlHandler` with a valid handler and `TRUE` is a
    // well-defined Windows API call.
    unsafe {
        SetConsoleCtrlHandler(Some(ctrl_c_handler), 1);
    }
}

/// Remove our console ctrl handler, restoring default behaviour.
pub(crate) fn uninstall_ctrl_c_handler() {
    unsafe {
        SetConsoleCtrlHandler(Some(ctrl_c_handler), 0);
    }
}
