use std::{
    fmt::{Display, Formatter},
    fs::File,
    io::{BufReader, Error, ErrorKind, Read, Result},
    path::Path,
    process::Command,
    thread::sleep,
    time::Duration,
};

use rand::Rng;
use rand::distr::Alphanumeric;
use tempfile::{NamedTempFile, TempDir, tempdir};
use which::which;

#[cfg(debug_assertions)]
pub static SK: &str = "SKIM_DEFAULT_OPTIONS= SKIM_DEFAULT_COMMAND= ../target/debug/sk";
#[cfg(not(debug_assertions))]
pub static SK: &str = "SKIM_DEFAULT_OPTIONS= SKIM_DEFAULT_COMMAND= ../target/release/sk";

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

fn wait<F, T>(pred: F) -> Result<T>
where
    F: Fn() -> Result<T>,
{
    for _ in 1..1000 {
        if let Ok(t) = pred() {
            return Ok(t);
        }
        sleep(Duration::from_millis(10));
    }
    Err(Error::new(ErrorKind::TimedOut, "wait timed out"))
}

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
        }
    }
}

pub struct TmuxController {
    window: String,
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

    pub fn new() -> Result<Self> {
        let unset_cmd = "unset SKIM_DEFAULT_COMMAND SKIM_DEFAULT_OPTIONS PS1 PROMPT_COMMAND HISTFILE";

        let shell_cmd = "bash --rcfile None";

        let name: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();

        Self::run(&[
            "new-window",
            "-d",
            "-P",
            "-F",
            "#I",
            "-n",
            &name,
            &format!("{}; {}", unset_cmd, shell_cmd),
        ])?;

        Self::run(&["set-window-option", "-t", &name, "pane-base-index", "0"])?;

        Ok(Self {
            window: name,
            tempdir: tempdir()?,
            outfile: None,
        })
    }

    pub fn send_keys(&self, keys: &[Keys]) -> Result<()> {
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

    pub fn until<F>(&self, pred: F) -> Result<()>
    where
        F: Fn(&[String]) -> bool,
    {
        match wait(|| {
            let lines = self.capture()?;
            if pred(&lines) {
                return Ok(true);
            }
            Err(Error::new(ErrorKind::Other, "pred not matched"))
        }) {
            Ok(true) => Ok(()),
            Ok(false) => Err(Error::new(ErrorKind::Other, self.capture()?.join("\n"))),
            _ => Err(Error::new(ErrorKind::TimedOut, self.capture()?.join("\n"))),
        }
    }

    /// Capture skim output without ANSI sequences
    pub fn output(&self) -> Result<Vec<String>> {
        if let Some(ref outfile) = self.outfile {
            self.output_from(outfile)
        } else {
            Err(Error::new(
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
                Err(Error::new(ErrorKind::NotFound, "outfile does not exist yet"))
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
            .rev()
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
// 2. TEST STYLES:
//    - @dsl { ... }    -> Ultra-compact DSL (recommended for most tests)
//    - tmux => { ... } -> Standard syntax (use for complex closures)
//
// DSL SYNTAX (Recommended)
// -------------------------
// sk_test!(test_name, "input", &["--opts"], @dsl {
//   @ line 0 == ">";                    // Assert line equals
//   @ line 1 != "text";                 // Assert line not equals
//   @ line 2 contains("substr");        // Assert line contains
//   @ line 3 starts_with("prefix");     // Assert line starts with
//   @ line 4 ends_with("suffix");       // Assert line ends with
//   @ lines |l| (l.len() > 5);          // Complex assertion with closure
//   @ keys Enter, Tab;                  // Send multiple keys
//   @ out 0 == "result";                // Assert output line
// });
//
// STANDARD SYNTAX (For complex tests)
// ------------------------------------
// sk_test!(test_name, @cmd "seq 1 100", &["--opts"], tmux => {
//   tmux.until(|l| l.len() > 10)?;
//   tmux.send_keys(&[Ctrl(&Key('f'))])?;
//   tmux.until(|l| l.iter().any(|x| x.contains("test")))?;
// });
//
// EXAMPLES
// --------
//
// Example 1: Simple test with echo input
//   sk_test!(simple, "a\\nb\\nc", &[], @dsl {
//     @ line 0 == ">";
//     @ keys Enter;
//     @ out 0 == "a";
//   });
//
// Example 2: Using command input with @cmd
//   sk_test!(with_seq, @cmd "seq 1 10", &["--bind", "'ctrl-t:toggle-all'"], @dsl {
//     @ line 0 == ">";
//     @ keys Ctrl(&Key('t'));
//     @ line 2 == ">>1";
//   });
//
// Example 3: Complex closures with @ lines
//   sk_test!(complex, "apple\\nbanana", &[], @dsl {
//     @ lines |l| (l.len() > 4);
//     @ keys Str("ana");
//     @ lines |l| (l.iter().any(|x| x.contains("banana")));
//   });
//
// Example 4: Standard syntax for very complex logic
//   sk_test!(advanced, @cmd "seq 1 100", &[], tmux => {
//     tmux.until(|l| l.len() > 10)?;
//     tmux.send_keys(&[Up, Up, Down])?;
//     tmux.until(|l| {
//       l.len() > 5 && l[2].starts_with(">") && l[2].contains("5")
//     })?;
//   });
//
// DSL COMMAND REFERENCE
// ---------------------
// @ line N == "text"        Assert line N equals text
// @ line N != "text"        Assert line N not equals text
// @ line N contains("x")    Assert line N contains substring
// @ line N starts_with("x") Assert line N starts with text
// @ line N ends_with("x")   Assert line N ends with text
// @ lines |l| (expr)        Call tmux.until(|l| expr)? with closure
// @ keys key1, key2         Send keys (automatically adds ?)
// @ out N == "text"         Assert output line N equals text
// @ out N != "text"         Assert output line N not equals text
// @ out N contains("x")     Assert output line N contains text
//
// NOTES
// -----
// - The `tmux` variable is implicitly available in DSL blocks
// - All variants automatically handle Result propagation and Ok(()) return
// - DSL closures must be wrapped in parentheses: |l| (expr)
// - Use standard syntax when you need complex multi-line closures
//
macro_rules! sk_test {
    // Standard variant with echo input: explicit variable name with block
    ($name:tt, $input:expr, $options:expr, $tmux:ident => $content:block) => {
      #[test]
      #[allow(unused_variables)]
      fn $name() -> Result<()> {
        let mut $tmux = TmuxController::new()?;
        $tmux.start_sk(Some(&format!("echo -n -e '{}'", $input)), $options)?;

        $content

        Ok(())
      }
    };

    // Standard variant with arbitrary command: use @cmd marker
    ($name:tt, @cmd $cmd:expr, $options:expr, $tmux:ident => $content:block) => {
      #[test]
      #[allow(unused_variables)]
      fn $name() -> Result<()> {
        let mut $tmux = TmuxController::new()?;
        $tmux.start_sk(Some($cmd), $options)?;

        $content

        Ok(())
      }
    };

    // DSL variant with echo input
    ($name:tt, $input:expr, $options:expr, @dsl { $($content:tt)* }) => {
      #[test]
      #[allow(unused_variables)]
      fn $name() -> Result<()> {
        let mut tmux = TmuxController::new()?;
        tmux.start_sk(Some(&format!("echo -n -e '{}'", $input)), $options)?;

        sk_test!(@expand tmux; $($content)*);

        Ok(())
      }
    };

    // DSL variant with arbitrary command: use @cmd marker
    ($name:tt, @cmd $cmd:expr, $options:expr, @dsl { $($content:tt)* }) => {
      #[test]
      #[allow(unused_variables)]
      fn $name() -> Result<()> {
        let mut tmux = TmuxController::new()?;
        tmux.start_sk(Some($cmd), $options)?;

        sk_test!(@expand tmux; $($content)*);

        Ok(())
      }
    };

    // Token processing rules
    (@expand $tmux:ident; ) => {};

    // @line command for assert_line with == operator
    (@expand $tmux:ident; @ line $line_nr:literal == $val:expr ; $($rest:tt)*) => {
        assert_line!($tmux, $line_nr == $val);
        sk_test!(@expand $tmux; $($rest)*);
    };

    // @line command for assert_line with != operator
    (@expand $tmux:ident; @ line $line_nr:literal != $val:expr ; $($rest:tt)*) => {
        assert_line!($tmux, $line_nr != $val);
        sk_test!(@expand $tmux; $($rest)*);
    };

    // @line command for assert_line with contains()
    (@expand $tmux:ident; @ line $line_nr:literal contains( $val:expr ) ; $($rest:tt)*) => {
        assert_line!($tmux, $line_nr .contains($val));
        sk_test!(@expand $tmux; $($rest)*);
    };

    // @line command for assert_line with starts_with()
    (@expand $tmux:ident; @ line $line_nr:literal starts_with( $val:expr ) ; $($rest:tt)*) => {
        assert_line!($tmux, $line_nr .starts_with($val));
        sk_test!(@expand $tmux; $($rest)*);
    };

    // @line command for assert_line with ends_with()
    (@expand $tmux:ident; @ line $line_nr:literal ends_with( $val:expr ) ; $($rest:tt)*) => {
        assert_line!($tmux, $line_nr .ends_with($val));
        sk_test!(@expand $tmux; $($rest)*);
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

    // @out command for assert_output_line with == operator
    (@expand $tmux:ident; @ out $line_nr:literal == $val:expr ; $($rest:tt)*) => {
        assert_output_line!($tmux, $line_nr == $val);
        sk_test!(@expand $tmux; $($rest)*);
    };

    // @out command for assert_output_line with != operator
    (@expand $tmux:ident; @ out $line_nr:literal != $val:expr ; $($rest:tt)*) => {
        assert_output_line!($tmux, $line_nr != $val);
        sk_test!(@expand $tmux; $($rest)*);
    };

    // @out command for assert_output_line with contains()
    (@expand $tmux:ident; @ out $line_nr:literal contains( $val:expr ) ; $($rest:tt)*) => {
        assert_output_line!($tmux, $line_nr .contains($val));
        sk_test!(@expand $tmux; $($rest)*);
    };

    // Pass through regular Rust statements that access tmux (still use semicolon)
    (@expand $tmux:ident; $stmt:stmt; $($rest:tt)*) => {
        $stmt;
        sk_test!(@expand $tmux; $($rest)*);
    };
}

#[allow(unused_macros)]
macro_rules! assert_line {
    ($tmux:ident, $line_nr:literal $($expression:tt)+) => {
      {
      if $tmux.until(|l| l.len() > $line_nr && l[$line_nr] $($expression)+).is_err() {
          let lines = $tmux.capture().unwrap_or_default();
          let actual = if lines.len() > $line_nr { &lines[$line_nr] } else { "<no line>" };
          Err(std::io::Error::new(std::io::ErrorKind::TimedOut, format!("Timed out waiting for condition on line {}, got {} but expected it to {}", $line_nr, actual, stringify!($($expression)+))))
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
