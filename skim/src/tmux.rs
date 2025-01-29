use std::{
    borrow::Cow,
    env,
    fs::File,
    io::{BufRead as _, BufReader, IsTerminal as _, Write as _},
    process::{Command, Stdio},
    sync::Arc,
    thread,
};

use nix::{sys::stat, unistd::mkfifo};
use rand::{distr::Alphanumeric, Rng};
use tuikit::key::Key;
use which::which;

use crate::{event::Event, SkimItem, SkimOptions, SkimOutput};

#[derive(Debug, PartialEq, Eq)]
enum TmuxWindowDir {
    Center,
    Top,
    Bottom,
    Left,
    Right,
}

impl From<&str> for TmuxWindowDir {
    fn from(value: &str) -> Self {
        use TmuxWindowDir::*;
        match value {
            "center" => Center,
            "top" => Top,
            "bottom" => Bottom,
            "left" => Left,
            "right" => Right,
            _ => Center,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct TmuxOptions<'a> {
    width: &'a str,
    height: &'a str,
    x: &'a str,
    y: &'a str,
}

struct SkimTmuxOutput {
    line: String,
}

impl SkimItem for SkimTmuxOutput {
    fn text(&self) -> Cow<'_, str> {
        Cow::from(&self.line)
    }
}

impl<'a> From<&'a String> for TmuxOptions<'a> {
    fn from(value: &'a String) -> Self {
        let (raw_dir, size) = value.split_once(",").unwrap_or((value, "50%"));
        let dir = TmuxWindowDir::from(raw_dir);
        let (height, width) = if let Some((lhs, rhs)) = size.split_once(",") {
            match dir {
                TmuxWindowDir::Center | TmuxWindowDir::Left | TmuxWindowDir::Right => (rhs, lhs),
                TmuxWindowDir::Top | TmuxWindowDir::Bottom => (lhs, rhs),
            }
        } else {
            match dir {
                TmuxWindowDir::Left | TmuxWindowDir::Right => ("100%", size),
                TmuxWindowDir::Top | TmuxWindowDir::Bottom => (size, "100%"),
                TmuxWindowDir::Center => (size, size),
            }
        };

        let (x, y) = match dir {
            TmuxWindowDir::Center => ("C", "C"),
            TmuxWindowDir::Top => ("C", "0%"),
            TmuxWindowDir::Bottom => ("C", "100%"),
            TmuxWindowDir::Left => ("0%", "C"),
            TmuxWindowDir::Right => ("100%", "C"),
        };

        Self { height, width, x, y }
    }
}

/// Run skim in a tmux popup
///
/// This will extract the tmux options, then build a new sk command
/// without them and send it to tmux in a popup.
pub fn run_with(opts: &SkimOptions) -> Option<SkimOutput> {
    // Create temp dir for downstream output
    let temp_dir_name = format!(
        "sk-tmux-{}",
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

    let t_tmp_stdin = temp_dir.join("stdin");
    let stdin_handle = if has_piped_input {
        debug!("Reading stdin and piping to file");

        mkfifo(&tmp_stdin, stat::Mode::S_IRWXU)
            .unwrap_or_else(|e| panic!("Failed to create stdin pipe {}: {}", tmp_stdin.clone().display(), e));
        Some(thread::spawn(move || {
            let mut stdin_writer = File::create(&t_tmp_stdin)
                .unwrap_or_else(|e| panic!("Failed to open stdin pipe {}: {}", t_tmp_stdin.clone().display(), e));

            loop {
                let mut buf = vec![];
                match stdin_reader.read_until(line_ending, &mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        debug!("Read {n} bytes from stdin");
                        stdin_writer.write_all(&buf).unwrap();
                    }
                    Err(e) => panic!("Failed to read from stdin: {}", e),
                }
            }
        }))
    } else {
        None
    };

    // Build args to send to downstream sk invocation
    let mut tmux_shell_cmd = String::new();
    let mut prev_is_tmux_flag = false;
    // We keep argv[0] to use in the popup's command
    for arg in std::env::args() {
        debug!("Got arg {}", arg);
        if prev_is_tmux_flag {
            prev_is_tmux_flag = false;
            if !arg.starts_with("-") {
                continue;
            }
        }
        if arg == "--tmux" {
            debug!("Found tmux arg, skipping this and the next");
            prev_is_tmux_flag = true;
            continue;
        } else if arg.starts_with("--tmux") {
            debug!("Found equal tmux arg, skipping");
            continue;
        }
        push_quoted_arg(&mut tmux_shell_cmd, &arg);
    }
    if has_piped_input {
        tmux_shell_cmd.push_str(&format!(" <{}", tmp_stdin.display()));
    }
    tmux_shell_cmd.push_str(&format!(" >{}", tmp_stdout.display()));

    debug!("build cmd {}", &tmux_shell_cmd);

    // Run downstream sk in tmux
    let raw_tmux_opts = &opts.tmux.clone().unwrap();
    let tmux_opts = TmuxOptions::from(raw_tmux_opts);
    let mut tmux_cmd = Command::new(which("tmux").unwrap_or_else(|e| panic!("Failed to find tmux in path: {}", e)));

    tmux_cmd
        .arg("display-popup")
        .arg("-E")
        .args(["-d", std::env::current_dir().unwrap().to_str().unwrap()])
        .args(["-h", tmux_opts.height])
        .args(["-w", tmux_opts.width])
        .args(["-x", tmux_opts.x])
        .args(["-y", tmux_opts.y]);

    for (name, value) in std::env::vars() {
        if name.starts_with("SKIM") || name == "PATH" || name.starts_with("RUST") {
            debug!("adding {} = {} to the command's env", name, value);
            tmux_cmd.args(["-e", &format!("{}={}", name, value)]);
        }
    }

    tmux_cmd.arg(tmux_shell_cmd);

    debug!("tmux command: {:?}", tmux_cmd);

    let status = tmux_cmd
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("Tmux invocation failed with {}", e));

    if let Some(h) = stdin_handle {
        h.join().unwrap_or(());
    }

    let output_ending = if opts.print0 { "\0" } else { "\n" };
    let mut stdout_bytes = std::fs::read_to_string(tmp_stdout).unwrap_or_default();
    stdout_bytes.pop();
    let mut stdout = stdout_bytes.split(output_ending);
    let _ = std::fs::remove_dir_all(temp_dir);

    let query_str = if opts.print_query && status.success() {
        stdout.next().expect("Not enough lines to unpack in downstream result")
    } else {
        ""
    };

    let command_str = if opts.print_cmd && status.success() {
        stdout.next().expect("Not enough lines to unpack in downstream result")
    } else {
        ""
    };

    let mut output_lines: Vec<Arc<dyn SkimItem>> = vec![];
    for line in stdout {
        debug!("Adding output line: {}", line);
        output_lines.push(Arc::new(SkimTmuxOutput { line: line.to_string() }));
    }

    let is_abort = !status.success();
    let final_event = match is_abort {
        true => Event::EvActAbort,
        false => Event::EvActAccept(None), // if --expect or --bind accept(key) are used,
                                           // the key is technically returned in the selected_items
    };

    let skim_output = SkimOutput {
        final_event,
        is_abort,
        final_key: Key::Null,
        query: query_str.to_string(),
        cmd: command_str.to_string(),
        selected_items: output_lines,
    };
    Some(skim_output)
}

fn push_quoted_arg(args_str: &mut String, arg: &str) {
    use shell_quote::{Bash, Fish, Quote as _, Sh, Zsh};
    let shell_path = env::var("SHELL").unwrap_or(String::from("/bin/sh"));
    let shell = shell_path.rsplit_once('/').unwrap_or(("", "sh")).1;
    let quoted_arg: Vec<u8> = match shell {
        "zsh" => Zsh::quote(arg),
        "bash" => Bash::quote(arg),
        "fish" => Fish::quote(arg),
        _ => Sh::quote(arg),
    };
    args_str.push_str(&format!(
        " {}",
        String::from_utf8(quoted_arg).expect("Failed to parse quoted arg as utf8, this should not happen")
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(input: &str, height: &str, width: &str, x: &str, y: &str) {
        assert_eq!(
            TmuxOptions::from(&String::from(input)),
            TmuxOptions { height, width, x, y }
        )
    }

    #[test]
    fn tmux_options_default() {
        check("", "50%", "50%", "C", "C");
    }
    #[test]
    fn tmux_options_center() {
        let (x, y) = ("C", "C");
        check("center", "50%", "50%", x, y);
        check("center,10", "10", "10", x, y);
        check("center,10,20", "20", "10", x, y);
        check("center,10%,20", "20", "10%", x, y);
        check("center,10%,20%", "20%", "10%", x, y);
    }
    #[test]
    fn tmux_options_top() {
        let (x, y) = ("C", "0%");
        check("top", "50%", "100%", x, y);
        check("top,10", "10", "100%", x, y);
        check("top,10,20", "10", "20", x, y);
        check("top,10%,20", "10%", "20", x, y);
        check("top,10%,20%", "10%", "20%", x, y);
    }
    #[test]
    fn tmux_options_bottom() {
        let (x, y) = ("C", "100%");
        check("bottom", "50%", "100%", x, y);
        check("bottom,10", "10", "100%", x, y);
        check("bottom,10,20", "10", "20", x, y);
        check("bottom,10%,20", "10%", "20", x, y);
        check("bottom,10%,20%", "10%", "20%", x, y);
    }
    #[test]
    fn tmux_options_left() {
        let (x, y) = ("0%", "C");
        check("left", "100%", "50%", x, y);
        check("left,10", "100%", "10", x, y);
        check("left,10,20", "20", "10", x, y);
        check("left,10%,20", "20", "10%", x, y);
        check("left,10%,20%", "20%", "10%", x, y);
    }
    #[test]
    fn tmux_options_right() {
        let (x, y) = ("100%", "C");
        check("right", "100%", "50%", x, y);
        check("right,10", "100%", "10", x, y);
        check("right,10,20", "20", "10", x, y);
        check("right,10%,20", "20", "10%", x, y);
        check("right,10%,20%", "20%", "10%", x, y);
    }
}
