//! Command-line interface for skim fuzzy finder.
//!
//! This binary provides the `sk` command-line tool for fuzzy finding and filtering.
#![cfg_attr(coverage, allow(unused_features), feature(coverage_attribute))]

extern crate clap;
extern crate env_logger;
extern crate log;
extern crate shlex;
extern crate skim;

use color_eyre::Result;
use color_eyre::eyre::eyre;
use interprocess::bound_util::RefWrite;
use interprocess::local_socket::ToNsName as _;
use interprocess::local_socket::traits::Stream as _;
use log::trace;
use skim::binds::parse_action_chain;
use skim::reader::CommandCollector;
use std::fs::File;
use std::io;
use std::io::{BufReader, BufWriter, IsTerminal, Write};

use skim::prelude::*;

fn init_logger(opts: &SkimOptions) {
    let target = if let Some(ref log_file) = opts.log_file.as_ref().or(std::env::var("SKIM_LOG_FILE").ok().as_ref()) {
        env_logger::Target::Pipe(Box::new(File::create(log_file).expect("Failed to create log file")))
    } else {
        env_logger::Target::Stdout
    };

    let env_var = "SKIM_LOG";

    let format = |buf: &mut env_logger::fmt::Formatter, record: &log::Record<'_>| {
        writeln!(
            buf,
            "[{} {} {} ({}:{})] [{}/{:?}] {}",
            buf.timestamp_nanos(),
            record.level().as_str(),
            record.module_path().unwrap_or("sk"),
            record.file().unwrap_or_default(),
            record.line().unwrap_or_default(),
            std::thread::current().name().unwrap_or("?"),
            std::thread::current().id(),
            record.args()
        )
    };

    if let Some(level) = opts.log_level {
        env_logger::builder()
            .filter_level(level)
            .parse_env(env_var)
            .target(target)
            .format(format)
            .init();
    } else {
        env_logger::builder()
            .parse_env(env_var)
            .target(target)
            .format(format)
            .init();
    }
}

//------------------------------------------------------------------------------
fn main() -> Result<()> {
    let mut opts = SkimOptions::from_env().unwrap_or_else(|e| {
        e.exit();
    });
    color_eyre::install()?;
    init_logger(&opts);

    // Build the options after setting the log target
    opts = opts.build();
    trace!("Command line: {:?}", std::env::args());

    // Shell completion scripts
    if let Some(shell) = opts.shell {
        // Generate completion script directly to stdout
        skim::shell::generate_completions(&shell, &mut std::io::stdout());
        if opts.shell_bindings {
            skim::shell::generate_key_bindings(&shell, &mut std::io::stdout())?;
        }
        return Ok(());
    }
    // Man page
    if opts.man {
        crate::manpage::generate(&mut std::io::stdout())?;
        return Ok(());
    }

    if let Some(remote) = opts.remote {
        let ns_name = remote
            .to_ns_name::<interprocess::local_socket::GenericNamespaced>()
            .unwrap();
        let stream = interprocess::local_socket::Stream::connect(ns_name)?;
        let mut action_chain = String::new();
        loop {
            action_chain.clear();
            let len = std::io::stdin().read_line(&mut action_chain)?;
            log::debug!("Got line {} from stdin", action_chain.trim());
            if len == 0 {
                break;
            }
            let actions = parse_action_chain(action_chain.trim())?;
            for act in actions {
                stream
                    .as_write()
                    .write_all(format!("{}\n", ron::ser::to_string(&act)?).as_bytes())?;
                log::debug!("Sent action {act:?} to listener");
            }
        }
        return Ok(());
    }

    match sk_main(opts) {
        Ok(exit_code) => std::process::exit(exit_code),
        Err(err) => match err.downcast_ref::<clap::error::Error>() {
            Some(e) => e.exit(),
            None => Err(eyre!(err)),
        },
    }
}

/// Returns `None` if the popup should not open, otherwise run the popup and return the result
#[cfg(unix)]
#[allow(clippy::option_option)]
fn check_and_run_popup(opts: &SkimOptions) -> Option<Option<SkimOutput>> {
    if opts.popup.is_some() && popup::check_env() {
        Some(crate::popup::run_with(opts))
    } else {
        None
    }
}
#[cfg(not(unix))]
#[allow(clippy::option_option)]
fn check_and_run_popup(_opts: &SkimOptions) -> Option<Option<SkimOutput>> {
    None
}

fn sk_main(mut opts: SkimOptions) -> Result<i32> {
    let reader_opts = SkimItemReaderOption::from_options(&opts);
    let cmd_collector = Rc::new(RefCell::new(SkimItemReader::new(reader_opts)));
    opts.cmd_collector = cmd_collector.clone() as Rc<RefCell<dyn CommandCollector>>;

    let cmd_history = opts.cmd_history.clone();
    let cmd_history_size = opts.cmd_history_size;
    let cmd_history_file = opts.cmd_history_file.clone();

    let query_history = opts.query_history.clone();
    let history_size = opts.history_size;
    let history_file = opts.history_file.clone();
    //------------------------------------------------------------------------------
    let bin_options = BinOptions::from_opts(&opts);

    //------------------------------------------------------------------------------
    // output

    let Some(result) = check_and_run_popup(&opts).unwrap_or_else(|| {
        // read from pipe or command
        let rx_item = if io::stdin().is_terminal() || (opts.interactive && opts.cmd.is_some()) {
            None
        } else {
            let rx_item = cmd_collector.borrow().of_bufread(BufReader::new(std::io::stdin()));
            Some(rx_item)
        };
        Skim::run_with(opts, rx_item).ok()
    }) else {
        return Ok(135);
    };
    log::debug!("result: {result:?}");

    if result.is_abort {
        return Ok(130);
    }

    // Output — use a large BufWriter to batch all writes into a few syscalls
    // instead of one syscall per item (Rust's default LineWriter flushes on \n).
    {
        let stdout = io::stdout();
        let mut out = BufWriter::with_capacity(1 << 20, stdout.lock());
        result.write_output(&mut out, &bin_options)?;
        out.flush()?;
    }

    //------------------------------------------------------------------------------
    // write the history with latest item
    if let Some(file) = history_file {
        let limit = history_size;
        write_history_to_file(&query_history, &result.query, limit, &file)?;
    }

    if let Some(file) = cmd_history_file {
        let limit = cmd_history_size;
        write_history_to_file(&cmd_history, &result.cmd, limit, &file)?;
    }

    Ok(i32::from(result.selected_items.is_empty()))
}

fn write_history_to_file(
    orig_history: &[String],
    latest: &str,
    limit: usize,
    filename: &str,
) -> Result<(), std::io::Error> {
    if orig_history.last().map(String::as_str) == Some(latest) {
        // no point of having at the end of the history 5x the same command...
        return Ok(());
    }
    let additional_lines = usize::from(!latest.trim().is_empty());
    let start_index = if orig_history.len() + additional_lines > limit {
        orig_history.len() + additional_lines - limit
    } else {
        0
    };

    let mut history = orig_history[start_index..].to_vec();
    history.push(latest.to_string());

    let file = File::create(filename)?;
    let mut file = BufWriter::new(file);
    file.write_all(history.join("\n").as_bytes())?;
    Ok(())
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    use super::*;

    fn read(path: &std::path::Path) -> String {
        std::fs::read_to_string(path).unwrap_or_default()
    }

    #[test]
    fn write_history_appends_latest_entry() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("hist");
        let file_str = file.to_str().unwrap();
        write_history_to_file(&["a".to_string(), "b".to_string()], "c", 10, file_str).unwrap();
        assert_eq!(read(&file), "a\nb\nc");
    }

    #[test]
    fn write_history_skips_duplicate_of_last() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("hist");
        let file_str = file.to_str().unwrap();
        // The latest equals the last entry → nothing is written, no file created.
        write_history_to_file(&["a".to_string(), "b".to_string()], "b", 10, file_str).unwrap();
        assert!(!file.exists());
    }

    #[test]
    fn write_history_truncates_to_limit() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("hist");
        let file_str = file.to_str().unwrap();
        // limit 2 with 3 existing + 1 new keeps only the newest entries.
        write_history_to_file(&["a".to_string(), "b".to_string(), "c".to_string()], "d", 2, file_str).unwrap();
        assert_eq!(read(&file), "c\nd");
    }

    #[test]
    fn write_history_empty_latest_does_not_count_towards_limit() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("hist");
        let file_str = file.to_str().unwrap();
        // An empty latest adds 0 to the length, so no truncation occurs at limit 3.
        write_history_to_file(&["a".to_string(), "b".to_string(), "c".to_string()], "", 3, file_str).unwrap();
        assert_eq!(read(&file), "a\nb\nc\n");
    }
}
