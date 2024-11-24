extern crate clap;
extern crate env_logger;
extern crate log;
extern crate shlex;
extern crate skim;
extern crate time;

use clap::{Error, Parser};
use derive_builder::Builder;
use std::fs::File;
use std::io::{BufReader, BufWriter, IsTerminal, Write};
use std::{env, io};

use skim::prelude::*;

fn parse_args() -> Result<SkimOptions, Error> {
    let mut args = Vec::new();

    args.push(
        env::args()
            .next()
            .expect("there should be at least one arg: the application name"),
    );
    args.extend(
        env::var("SKIM_DEFAULT_OPTIONS")
            .ok()
            .and_then(|val| shlex::split(&val))
            .unwrap_or_default(),
    );
    for arg in env::args().skip(1) {
        args.push(arg);
    }

    Ok(SkimOptions::try_parse_from(args)?.build())
}

//------------------------------------------------------------------------------
fn main() {
    env_logger::builder().format_timestamp_nanos().init();

    use SkMainError::*;
    match sk_main() {
        Ok(exit_code) => std::process::exit(exit_code),
        Err(err) => {
            // if downstream pipe is closed, exit silently, see PR#279
            match err {
                IoError(e) => {
                    if e.kind() == std::io::ErrorKind::BrokenPipe {
                        std::process::exit(0)
                    } else {
                        std::process::exit(2)
                    }
                }
                ArgError(e) => e.exit(),
            }
        }
    }
}

enum SkMainError {
    IoError(std::io::Error),
    ArgError(clap::Error),
}

impl From<std::io::Error> for SkMainError {
    fn from(value: std::io::Error) -> Self {
        Self::IoError(value)
    }
}

impl From<clap::Error> for SkMainError {
    fn from(value: clap::Error) -> Self {
        Self::ArgError(value)
    }
}

fn sk_main() -> Result<i32, SkMainError> {
    let mut opts = parse_args()?;

    let reader_opts = SkimItemReaderOption::default()
        .ansi(opts.ansi)
        .delimiter(&opts.delimiter)
        .with_nth(opts.with_nth.iter().map(|s| s.as_str()))
        .nth(opts.nth.iter().map(|s| s.as_str()))
        .read0(opts.read0)
        .show_error(opts.show_cmd_error);
    let cmd_collector = Rc::new(RefCell::new(SkimItemReader::new(reader_opts)));
    opts.cmd_collector = cmd_collector.clone();
    //------------------------------------------------------------------------------
    let bin_options = BinOptions {
        filter: opts.filter.clone(),
        print_query: opts.print_query,
        print_cmd: opts.print_cmd,
        output_ending: String::from(if opts.print0 { "\0" } else { "\n" }),
    };

    //------------------------------------------------------------------------------
    // read from pipe or command
    let rx_item = if !io::stdin().is_terminal() {
        let rx_item = cmd_collector.borrow().of_bufread(BufReader::new(std::io::stdin()));
        Some(rx_item)
    } else {
        None
    };

    //------------------------------------------------------------------------------
    // filter mode
    if opts.filter.is_some() {
        return Ok(filter(&bin_options, &opts, rx_item)?);
    }

    //------------------------------------------------------------------------------
    // output

    let Some(result) = Skim::run_with(&opts, rx_item) else {
        return Ok(0);
    };

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

    if !opts.expect.is_empty() {
        match result.final_event {
            Event::EvActAccept(Some(accept_key)) => {
                print!("{}{}", accept_key, bin_options.output_ending);
            }
            Event::EvActAccept(None) => {
                print!("{}", bin_options.output_ending);
            }
            _ => {}
        }
    }

    for item in result.selected_items.iter() {
        print!("{}{}", item.output(), bin_options.output_ending);
    }

    std::io::stdout().flush()?;

    //------------------------------------------------------------------------------
    // write the history with latest item
    if let Some(file) = opts.history_file {
        let limit = opts.history_size;
        write_history_to_file(&opts.query_history, &result.query, limit, &file)?;
    }

    if let Some(file) = opts.cmd_history_file {
        let limit = opts.cmd_history_size;
        write_history_to_file(&opts.cmd_history, &result.cmd, limit, &file)?;
    }

    Ok(if result.selected_items.is_empty() { 1 } else { 0 })
}

fn write_history_to_file(
    orig_history: &[String],
    latest: &str,
    limit: usize,
    filename: &str,
) -> Result<(), std::io::Error> {
    if orig_history.last().map(|l| l.as_str()) == Some(latest) {
        // no point of having at the end of the history 5x the same command...
        return Ok(());
    }
    let additional_lines = if latest.trim().is_empty() { 0 } else { 1 };
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

#[derive(Builder)]
pub struct BinOptions {
    filter: Option<String>,
    output_ending: String,
    print_query: bool,
    print_cmd: bool,
}

pub fn filter(
    bin_option: &BinOptions,
    options: &SkimOptions,
    source: Option<SkimItemReceiver>,
) -> Result<i32, std::io::Error> {
    let default_command = match env::var("SKIM_DEFAULT_COMMAND").as_ref().map(String::as_ref) {
        Ok("") | Err(_) => "find .".to_owned(),
        Ok(val) => val.to_owned(),
    };
    let query = bin_option.filter.clone().unwrap_or_default();
    let cmd = options.cmd.clone().unwrap_or(default_command);

    // output query
    if bin_option.print_query {
        print!("{}{}", query, bin_option.output_ending);
    }

    if bin_option.print_cmd {
        print!("{}{}", cmd, bin_option.output_ending);
    }

    //------------------------------------------------------------------------------
    // matcher
    let engine_factory: Box<dyn MatchEngineFactory> = if options.regex {
        Box::new(RegexEngineFactory::builder())
    } else {
        let fuzzy_engine_factory = ExactOrFuzzyEngineFactory::builder()
            .fuzzy_algorithm(options.algorithm)
            .exact_mode(options.exact)
            .build();
        Box::new(AndOrEngineFactory::new(fuzzy_engine_factory))
    };

    let engine = engine_factory.create_engine_with_case(&query, options.case);

    //------------------------------------------------------------------------------
    // start
    let components_to_stop = Arc::new(AtomicUsize::new(0));

    let stream_of_item = source.unwrap_or_else(|| {
        let (ret, _control) = options.cmd_collector.borrow_mut().invoke(&cmd, components_to_stop);
        ret
    });

    let mut num_matched = 0;
    stream_of_item
        .into_iter()
        .filter_map(|item| engine.match_item(item.clone()).map(|result| (item, result)))
        .for_each(|(item, _match_result)| {
            num_matched += 1;
            print!("{}{}", item.output(), bin_option.output_ending)
        });

    Ok(if num_matched == 0 { 1 } else { 0 })
}
