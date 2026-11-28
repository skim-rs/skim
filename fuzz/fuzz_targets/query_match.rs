#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use regex::Regex;
use skim::helper::item::DefaultSkimItem;
use skim::matcher::Matcher;
use skim::{SkimItem, SkimOptions};

// End-to-end fuzz of the query -> engine -> match pipeline: builds a real
// `DefaultSkimItem` (exercising ANSI stripping / field transforms) and
// matches it with an engine built the same way skim builds it from CLI
// options (exact/regex/andor/fuzzy-algorithm wrapping), using an arbitrary
// query string. Checks that matching never panics and that any reported
// match range stays within the bounds of the text that was actually matched.
#[derive(Arbitrary, Debug)]
struct QueryInput<'a> {
    query: &'a str,
    text: &'a str,
    exact: bool,
    regex: bool,
    ansi: bool,
    case: u8,
    algo: u8,
    typos: bool,
    normalize: bool,
    split_match: bool,
}

fuzz_target!(|input: QueryInput| {
    // The regex engine path takes the query as a user-supplied pattern; keep
    // it short so fuzzing time goes into skim's logic, not regex parsing.
    if input.regex && input.query.len() > 32 {
        return;
    }

    let mut options = SkimOptions::default();
    options.exact = input.exact;
    options.regex = input.regex;
    options.case = match input.case % 3 {
        0 => skim::CaseMatching::Respect,
        1 => skim::CaseMatching::Ignore,
        _ => skim::CaseMatching::Smart,
    };
    options.algorithm = match input.algo % 4 {
        0 => skim::FuzzyAlgorithm::Arinae,
        1 => skim::FuzzyAlgorithm::SkimV2,
        2 => skim::FuzzyAlgorithm::Clangd,
        _ => skim::FuzzyAlgorithm::Fzy,
    };
    options.typos = if input.typos {
        skim::Typos::Smart
    } else {
        skim::Typos::Disabled
    };
    options.normalize = input.normalize;
    options.split_match = input.split_match.then_some(':');

    let factory = Matcher::create_engine_factory(&options);
    let engine = factory.create_engine_with_case(input.query, options.case);

    let delimiter = Regex::new(" ").unwrap();
    let item = DefaultSkimItem::new(input.text, input.ansi, &[], &[], &delimiter);

    // Skim ranks every item with `match_rank` and highlights the displayed ones
    // with `match_item`: both must agree.
    let result = engine.match_item(&item);
    assert_eq!(
        engine.match_rank(&item),
        result.as_ref().map(|r| r.rank),
        "match_rank vs match_item, engine {engine}"
    );
    if let Some(result) = result {
        let matched_text = item.text();
        let num_chars = matched_text.chars().count();
        for idx in result.range_char_indices(&matched_text) {
            assert!(
                idx <= num_chars,
                "matched char index {idx} out of bounds ({num_chars} chars)"
            );
        }
    }
});
