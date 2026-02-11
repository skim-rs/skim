use std::{
    fmt::{Display, Formatter},
    fs::File,
    io::{BufReader, ErrorKind, Read, Result},
    path::Path,
    process::Command,
    thread::sleep,
    time::Duration,
};

use rand::RngExt as _;
use rand::distr::Alphanumeric;
use tempfile::{NamedTempFile, TempDir, tempdir};
use which::which;

use crate::common::SK;

pub fn sk(outfile: &str, opts: &[&str]) -> String {
    format!(
        "{} {} > {}.part; mv {}.part {}",
        SK,
        opts.join(" "),
        outfile,
        outfile,
        outfile
    )
}

pub fn wait<F, T>(pred: F) -> Result<T>
where
    F: Fn() -> Result<T>,
{
    for _ in 1..500 {
        if let Ok(t) = pred() {
            return Ok(t);
        }
        sleep(Duration::from_millis(10));
    }
    Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "wait timed out"))
}

pub enum Keys<'a> {
    /// Do not use, send multiple `Key`s instead
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

pub struct TmuxController {
    pub window: String,
    pub tempdir: TempDir,
    pub outfile: Option<String>,
}

impl Default for TmuxController {
    fn default() -> Self {
        Self {
            window: String::new(),
            tempdir: tempfile::tempdir().expect("Failed to create tempdir"),
            outfile: None,
        }
    }
}

impl TmuxController {
    pub fn run(args: &[&str]) -> Result<Vec<String>> {
        let output = Command::new(which("tmux").expect("Please install tmux to $PATH"))
            .args(args)
            .output()?
            .stdout
            .split(|c| *c == b'\n')
            .map(|bytes| String::from_utf8(bytes.to_vec()).expect("Failed to parse bytes as UTF8 string"))
            .collect::<Vec<String>>();
        Ok(output[0..output.len() - 1].to_vec())
    }

    pub fn new_named(name: &str) -> Result<Self> {
        let unset_cmd = "unset SKIM_DEFAULT_COMMAND SKIM_DEFAULT_OPTIONS PS1 PROMPT_COMMAND HISTFILE";

        let full_name = format!(
            "{name}-{}",
            rand::rng()
                .sample_iter(&Alphanumeric)
                .take(4)
                .map(char::from)
                .collect::<String>()
        );
        let shell_cmd = "bash --rcfile None";

        Self::run(&[
            "new-window",
            "-d",
            "-P",
            "-F",
            "#I",
            "-t",
            "skim_e2e:",
            "-n",
            &full_name,
            &format!("{}; {}", unset_cmd, shell_cmd),
        ])?;

        Self::run(&["set-window-option", "-t", &full_name, "pane-base-index", "0"])?;

        Ok(Self {
            window: format!("skim_e2e:{full_name}"),
            tempdir: tempdir()?,
            outfile: None,
        })
    }

    pub fn new() -> Result<Self> {
        let name: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();
        Self::new_named(&name)
    }

    pub fn send_keys(&self, keys: &[Keys]) -> std::io::Result<()> {
        print!("typing `");
        for key in keys {
            Self::run(&["send-keys", "-t", &self.window, &key.to_string()])?;
            print!("{}", key.to_string());
        }
        println!("`");
        Ok(())
    }

    pub fn tempfile(&self) -> Result<String> {
        Ok(NamedTempFile::new_in(&self.tempdir)?
            .path()
            .to_str()
            .unwrap()
            .to_string())
    }

    // Returns the lines in reverted order
    pub fn capture(&self) -> Result<Vec<String>> {
        let tempfile = wait(|| {
            let tempfile = self.tempfile()?;
            Self::run(&[
                "capture-pane",
                "-J",
                "-b",
                &self.window,
                "-t",
                &format!("{}.0", self.window),
            ])?;
            Self::run(&["save-buffer", "-b", &self.window, &tempfile])?;
            Ok(tempfile)
        })?;

        let mut string_lines = String::new();
        BufReader::new(File::open(tempfile)?).read_to_string(&mut string_lines)?;

        let str_lines = string_lines.trim();
        Ok(str_lines
            .split("\n")
            .map(|s| s.to_string())
            .collect::<Vec<String>>()
            .into_iter()
            .rev()
            .collect())
    }

    // Capture with ANSI escape sequences preserved (using -e flag)
    // Returns the lines in reverted order with ANSI codes
    pub fn capture_colored(&self) -> Result<Vec<String>> {
        let tempfile = wait(|| {
            let tempfile = self.tempfile()?;
            Self::run(&[
                "capture-pane",
                "-e",
                "-J",
                "-b",
                &self.window,
                "-t",
                &format!("{}.0", self.window),
            ])?;
            Self::run(&["save-buffer", "-b", &self.window, &tempfile])?;
            Ok(tempfile)
        })?;

        let mut string_lines = String::new();
        BufReader::new(File::open(tempfile)?).read_to_string(&mut string_lines)?;

        let str_lines = string_lines.trim();
        Ok(str_lines
            .split("\n")
            .map(|s| s.to_string())
            .collect::<Vec<String>>()
            .into_iter()
            .rev()
            .collect())
    }

    pub fn until<F>(&self, pred: F) -> std::io::Result<()>
    where
        F: Fn(&[String]) -> bool,
    {
        match wait(|| {
            let lines = self.capture()?;
            if pred(&lines) {
                return Ok(true);
            }
            Err(std::io::Error::new(ErrorKind::Other, "pred not matched"))
        }) {
            Ok(true) => Ok(()),
            Ok(false) => Err(std::io::Error::new(ErrorKind::Other, self.capture()?.join("\n"))),
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

impl Drop for TmuxController {
    fn drop(&mut self) {
        let _ = Self::run(&["kill-window", "-t", &self.window]);
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
// NOTE: All methods use wait() for consistent retry behavior. Any TmuxController
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
//   where METHOD is any TmuxController method returning Result<Vec<String>>:
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
        let mut $tmux = crate::common::tmux::TmuxController::new()?;
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
        let mut $tmux = crate::common::tmux::TmuxController::new()?;
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
        let mut tmux = crate::common::tmux::TmuxController::new_named(stringify!($name))?;
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
        let mut tmux = crate::common::tmux::TmuxController::new_named(stringify!($name))?;
        tmux.start_sk(Some($cmd), $options)?;

        sk_test!(@expand tmux; $($content)*);

        Ok(())
      }
    };

    // Token processing rules
    (@expand $tmux:ident; ) => {};

    // Generic method patterns - works with any TmuxController method
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
            if crate::common::tmux::wait(|| {
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
            if crate::common::tmux::wait(|| {
                let lines = $tmux.$method()?;
                if lines.len() >= $idx {
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
                let actual = if lines.len() >= $idx { &lines[actual_idx] } else { "<no line>" };
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
            if crate::common::tmux::wait(|| {
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
          Err(std::io::std::io::Error::new(std::io::std::io::ErrorKind::TimedOut, format!("Timed out waiting for condition on line {}, got {} but expected it to {}", $line_nr, actual, stringify!($($expression)+))))
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
