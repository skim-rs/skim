use std::{
    borrow::Cow,
    io::{BufRead as _, BufReader, BufWriter, IsTerminal as _, Write as _},
    sync::Arc,
    thread,
};

use rand::{distributions::Alphanumeric, Rng};
use tmux_interface::{StdIO, Tmux};
use tuikit::key::Key;

use crate::{event::Event, SkimItem, SkimOptions, SkimOutput};

#[derive(Debug)]
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

#[derive(Debug)]
pub struct TmuxOptions<'a> {
    pub width: &'a str,
    pub height: &'a str,
    pub x: &'a str,
    pub y: &'a str,
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

pub fn run_with(opts: &SkimOptions) -> Option<SkimOutput> {
    // Create temp dir for downstream output
    let temp_dir_name = format!(
        "sk-tmux-{}",
        &rand::thread_rng()
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

    let stdin_handle = if has_piped_input {
        debug!("Reading stdin and piping to file");

        let stdin_f = std::fs::File::create(tmp_stdin.clone())
            .unwrap_or_else(|e| panic!("Failed to create stdin file {}: {}", tmp_stdin.clone().display(), e));
        let mut stdin_writer = BufWriter::new(stdin_f);
        Some(thread::spawn(move || loop {
            let mut buf = vec![];
            match stdin_reader.read_until(line_ending, &mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    debug!("Read {n} bytes from stdin");
                    stdin_writer.write_all(&buf).unwrap();
                }
                Err(e) => panic!("Failed to read from stdin: {}", e),
            }
        }))
    } else {
        None
    };

    // Build args to send to downstream sk invocation
    let mut tmux_shell_cmd = String::new();
    let mut prev_is_tmux_flag = false;
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
        tmux_shell_cmd.push_str(&format!(" {arg}"));
    }
    if has_piped_input {
        tmux_shell_cmd.push_str(&format!(" <{}", tmp_stdin.display()));
    }
    tmux_shell_cmd.push_str(&format!(" >{}", tmp_stdout.display()));

    debug!("build cmd {}", &tmux_shell_cmd);

    // Run downstream sk in tmux
    let raw_tmux_opts = &opts.tmux.clone().unwrap();
    let tmux_opts = TmuxOptions::from(raw_tmux_opts);
    let tmux_cmd = tmux_interface::commands::tmux_command::TmuxCommand::new()
        .name("popup")
        .push_flag("-E")
        .push_option("-h", tmux_opts.height)
        .push_option("-w", tmux_opts.width)
        .push_option("-x", tmux_opts.x)
        .push_option("-y", tmux_opts.y)
        .push_param(tmux_shell_cmd)
        .to_owned();

    let out = Tmux::with_command(tmux_cmd).stdin(Some(StdIO::Inherit)).output();

    let _ = std::fs::remove_dir_all(temp_dir);

    debug!("Tmux returned {:?}", out);

    let status = out.expect("Failed to run command in popup").status();

    if let Some(h) = stdin_handle {
        h.join().unwrap_or(());
    }

    let output_ending = if opts.print0 { "\0" } else { "\n" };
    let stdout_bytes = std::fs::read_to_string(tmp_stdout).unwrap_or_default();
    let mut stdout = stdout_bytes.split(output_ending);

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

    let accept_key = if !opts.expect.is_empty() && status.success() {
        Some(
            stdout
                .next()
                .expect("Not enough lines to unpack in downstream result")
                .to_string(),
        )
    } else {
        None
    };

    let mut selected_items: Vec<Arc<dyn SkimItem>> = vec![];
    for line in stdout {
        selected_items.push(Arc::new(SkimTmuxOutput { line: line.to_string() }));
    }

    let is_abort = !status.success();
    let final_event = match is_abort {
        true => Event::EvActAbort,
        false => Event::EvActAccept(accept_key),
    };

    let skim_output = SkimOutput {
        final_event,
        is_abort,
        final_key: Key::Null,
        query: query_str.to_string(),
        cmd: command_str.to_string(),
        selected_items,
    };
    Some(skim_output)
}
