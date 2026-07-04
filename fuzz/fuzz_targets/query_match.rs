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

    let factory = Matcher::create_engine_factory(&options);
    let engine = factory.create_engine_with_case(input.query, options.case);

    let delimiter = Regex::new(" ").unwrap();
    let item = DefaultSkimItem::new(input.text, input.ansi, &[], &[], &delimiter);

    if let Some(result) = engine.match_item(&item) {
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
