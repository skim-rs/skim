//! Command-line interface for skim fuzzy finder.
//!
//! This binary provides the `sk` command-line tool for fuzzy finding and filtering.

extern crate clap;
extern crate env_logger;
extern crate log;
extern crate shlex;
extern crate skim;

use crate::Event;
use color_eyre::Result;
use color_eyre::eyre::eyre;
use derive_builder::Builder;
use interprocess::bound_util::RefWrite;
use interprocess::local_socket::ToNsName as _;
use interprocess::local_socket::traits::Stream as _;
use log::trace;
use skim::binds::parse_action_chain;
use skim::reader::CommandCollector;
use skim::tui::event::Action;
use std::fs::File;
use std::io::{BufReader, BufWriter, IsTerminal, Write};
use std::{env, io};

use skim::prelude::*;

//------------------------------------------------------------------------------
fn main() -> Result<()> {
    let mut opts = SkimOptions::from_env().unwrap_or_else(|e| {
        e.exit();
    });
    color_eyre::install()?;
    let log_target = if let Some(ref log_file) = opts.log_file {
        env_logger::Target::Pipe(Box::new(File::create(log_file).expect("Failed to create log file")))
    } else {
        env_logger::Target::Stdout
    };
    env_logger::builder()
        .target(log_target)
        .format(|buf, record| {
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
        })
        .init();
    // Build the options after setting the log target
    opts = opts.build();
    trace!("Command line: {:?}", std::env::args());

    // Shell completion scripts
    if let Some(shell) = opts.shell {
        // Generate completion script directly to stdout
        skim::completions::generate(&shell);
        if opts.shell_bindings {
            use skim::completions::Shell::*;
            let binds_script = match &shell {
                Bash => include_str!("../../shell/key-bindings.bash"),
                Zsh => include_str!("../../shell/key-bindings.zsh"),
                Fish => include_str!("../../shell/key-bindings.fish"),
                _ => "",
            };
            if !binds_script.is_empty() {
                println!("{binds_script}");
            }
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
    let bin_options = BinOptions {
        print_query: opts.print_query,
        print_cmd: opts.print_cmd,
        print_score: opts.print_score,
        print_header: opts.print_header,
        output_ending: String::from(if opts.print0 { "\0" } else { "\n" }),
        strip_ansi: opts.ansi && !opts.no_strip_ansi,
    };

    //------------------------------------------------------------------------------
    // output

    let Some(result) = (if opts.tmux.is_some() && env::var("TMUX").is_ok() {
        crate::tmux::run_with(&opts)
    } else {
        // read from pipe or command
        let rx_item = if io::stdin().is_terminal() || (opts.interactive && opts.cmd.is_some()) {
            None
        } else {
            let rx_item = cmd_collector.borrow().of_bufread(BufReader::new(std::io::stdin()));
            Some(rx_item)
        };
        Some(Skim::run_with(opts, rx_item)?)
    }) else {
        return Ok(135);
    };
    log::debug!("result: {result:?}");

    if result.is_abort {
        return Ok(130);
    }

    // output query
    if bin_options.print_query {
        print!("{}{}", result.query, bin_options.output_ending);
    }

    if bin_options.print_cmd {
        print!("{}{}", result.cmd, bin_options.output_ending);
    }

    if bin_options.print_header {
        print!("{}{}", result.header, bin_options.output_ending);
    }

    if let Event::Action(Action::Accept(Some(accept_key))) = result.final_event {
        print!("{}{}", accept_key, bin_options.output_ending);
    }

    for item in &result.selected_items {
        if bin_options.strip_ansi {
            print!(
                "{}{}",
                skim::helper::item::strip_ansi(&item.output()).0,
                bin_options.output_ending
            );
        } else {
            print!("{}{}", item.output(), bin_options.output_ending);
        }
        if bin_options.print_score {
            print!("{}{}", item.rank[0], bin_options.output_ending);
        }
    }

    std::io::stdout().flush()?;

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

/// Options specific to the binary/CLI mode
#[derive(Builder)]
#[allow(missing_docs)]
pub struct BinOptions {
    output_ending: String,
    print_query: bool,
    print_cmd: bool,
    print_score: bool,
    print_header: bool,
    strip_ansi: bool,
}
