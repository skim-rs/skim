use std::{
    fmt::{Display, Formatter},
    fs::File,
    io::{BufReader, Error, ErrorKind, Read, Result},
    path::Path,
    process::Command,
    thread::sleep,
    time::Duration,
};

use rand::distr::{Alphanumeric, SampleString as _};
use tempfile::{tempdir, NamedTempFile, TempDir};
use which::which;

pub static SK: &str = "SKIM_DEFAULT_OPTIONS= SKIM_DEFAULT_COMMAND= cargo run --package skim --release --";

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
    for _ in 1..2000 {
        if let Ok(t) = pred() {
            return Ok(t);
        }
        sleep(Duration::from_millis(50));
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
        }
    }
}

pub struct TmuxController {
    window: String,
    pub tempdir: TempDir,
}

impl TmuxController {
    pub fn run(args: &[&str]) -> Result<Vec<String>> {
        println!("Running {:?}", args);
        let output = Command::new(which("tmux").expect("Please install tmux to $PATH"))
            .args(args)
            .output()?
            .stdout
            .split(|c| *c == b'\n')
            .map(|bytes| String::from_utf8(bytes.to_vec()).expect("Failed to parse bytes as UTF8 string"))
            .collect::<Vec<String>>();
        sleep(Duration::from_millis(50));
        Ok(output[0..output.len() - 1].to_vec())
    }

    pub fn new() -> Result<Self> {
        let unset_cmd = "unset SKIM_DEFAULT_COMMAND SKIM_DEFAULT_OPTIONS PS1 PROMPT_COMMAND";

        let shell_cmd = "bash --rcfile None";

        let name = Alphanumeric.sample_string(&mut rand::rng(), 16);

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
        })
    }

    pub fn send_keys(&self, keys: &[Keys]) -> Result<()> {
        for key in keys {
            Self::run(&["send-keys", "-t", &self.window, &key.to_string()])?;
        }
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
            Self::run(&["capture-pane", "-b", &self.window, "-t", &format!("{}.0", self.window)])?;
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

    pub fn output(&self, outfile: &str) -> Result<Vec<String>> {
        wait(|| {
            if Path::new(outfile).exists() {
                Ok(())
            } else {
                Err(Error::new(ErrorKind::NotFound, "oufile does not exist yet"))
            }
        })?;
        let mut string_lines = String::new();
        println!("{}", Path::new(outfile).exists());
        println!("Reading file {outfile}");
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

    pub fn start_sk(&self, stdin_cmd: Option<&str>, opts: &[&str]) -> Result<String> {
        let outfile = self.tempfile()?;
        let sk_cmd = sk(&outfile, opts);
        let cmd = match stdin_cmd {
            Some(s) => format!("{} | {}", s, sk_cmd),
            None => sk_cmd,
        };
        self.send_keys(&[Keys::Str(&cmd), Keys::Enter])?;
        Ok(outfile)
    }
}

impl Drop for TmuxController {
    fn drop(&mut self) {
        let _ = Self::run(&["kill-window", "-t", &self.window]);
    }
}
