use skim::fuzzy_matcher::FuzzyMatcher;
use skim::fuzzy_matcher::clangd::ClangdMatcher;
use skim::fuzzy_matcher::skim::SkimMatcherV2;
use std::env;
use std::io::{self, BufRead};
use std::process::exit;

type IndexType = usize;

pub fn main() {
    let args: Vec<String> = env::args().collect();

    // arg parsing (manually)
    let mut arg_iter = args.iter().skip(1);
    let mut pattern = "".to_string();
    let mut algorithm = Some("skim");

    while let Some(arg) = arg_iter.next() {
        if arg == "--algo" {
            algorithm = arg_iter.next().map(String::as_ref);
        } else {
            pattern = arg.to_string();
        }
    }

    if &pattern == "" {
        eprintln!("Usage: echo <piped_input> | fz --algo [skim|clangd] <pattern>");
        exit(1);
    }

    let matcher: Box<dyn FuzzyMatcher> = match algorithm {
        Some("skim") | Some("skim_v2") => Box::new(SkimMatcherV2::default()),
        Some("clangd") => Box::new(ClangdMatcher::default()),
        _ => panic!("Algorithm not supported: {:?}", algorithm),
    };

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        if let Ok(line) = line {
            if let Some((score, indices)) = matcher.fuzzy_indices(&line, &pattern) {
                println!("{:8}: {}", score, wrap_matches(&line, &indices));
            }
        }
    }
}

fn wrap_matches(line: &str, indices: &[IndexType]) -> String {
    let mut ret = String::new();
    let mut peekable = indices.iter().peekable();
    let ansi_invert: &str = str::from_utf8(&[27, b'[', b'7', b'm']).unwrap();
    let ansi_reset: &str = str::from_utf8(&[27, b'[', b'0', b'm']).unwrap();
    for (idx, ch) in line.chars().enumerate() {
        let next_id = **peekable.peek().unwrap_or(&&(line.len() as IndexType));
        if next_id == (idx as IndexType) {
            ret.push_str(format!("{}{}{}", ansi_invert, ch, ansi_reset).as_str());
            peekable.next();
        } else {
            ret.push(ch);
        }
    }

    ret
}
