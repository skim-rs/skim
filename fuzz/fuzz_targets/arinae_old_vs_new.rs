#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use skim::fuzzy_matcher::FuzzyMatcher;
use skim::fuzzy_matcher::arinae::ArinaeMatcher;
use skim::{CaseMatching, SkimOptions, Typos};

#[path = "../old_arinae/shim.rs"]
mod shim;
pub use shim::fuzzy_matcher;
#[allow(dead_code, clippy::all)]
#[path = "../old_arinae/mod.rs"]
mod old_arinae;

use old_arinae::ArinaeMatcher as OldArinaeMatcher;

// Differential fuzz of the Arinae algorithm. Skim used to rank every item with
// `fuzzy_indices` (full matrix + traceback); it now ranks with the rolling
// `fuzzy_match_range`. Both must give the same items, score, begin and end as
// the pre-optimization Arinae (vendored in `old_arinae/`). The highlight path
// (`fuzzy_indices`) must not change either. At the engine level, `match_rank`
// must agree with the rank of `match_item` (the old ranking path).
#[derive(Arbitrary, Debug)]
struct Input<'a> {
    choice: &'a str,
    pattern: &'a str,
    typos: bool,
    last_match: bool,
    case: u8,
    /// Map both strings to a small ASCII alphabet with separators and case:
    /// hits the SIMD path (ASCII, pattern <= 8) with many partial matches.
    small_alphabet: bool,
}

fn to_small_alphabet(s: &str, max_len: usize) -> String {
    const ALPHABET: &[u8] = b"abcABC_/- .e";
    s.bytes()
        .take(max_len)
        .map(|b| ALPHABET[usize::from(b) % ALPHABET.len()] as char)
        .collect()
}

fuzz_target!(|input: Input| {
    let case = match input.case % 3 {
        0 => CaseMatching::Respect,
        1 => CaseMatching::Ignore,
        _ => CaseMatching::Smart,
    };
    let (choice, pattern) = if input.small_alphabet {
        (
            to_small_alphabet(input.choice, usize::MAX),
            to_small_alphabet(input.pattern, 8),
        )
    } else {
        (input.choice.to_string(), input.pattern.to_string())
    };
    let (choice, pattern) = (choice.as_str(), pattern.as_str());

    // Algorithm level: new vs old.
    let new = ArinaeMatcher::new(case, input.typos, input.last_match);
    let old = OldArinaeMatcher::new(case, input.typos, input.last_match);
    let old_indices = old.fuzzy_indices(choice, pattern);
    let old_range = old_indices
        .as_ref()
        .map(|(s, idx)| (*s, idx.first().copied().unwrap_or(0), idx.last().copied().unwrap_or(0)));
    assert_eq!(new.fuzzy_match_range(choice, pattern), old_range, "fuzzy_match_range");
    assert_eq!(
        new.fuzzy_match(choice, pattern),
        old_range.map(|(s, _, _)| s),
        "fuzzy_match"
    );
    assert_eq!(new.fuzzy_indices(choice, pattern), old_indices, "fuzzy_indices");

    // Engine level, built from options like skim does (query syntax, AND/OR, ...).
    let mut options = SkimOptions::default();
    options.case = case;
    options.typos = if input.typos { Typos::Smart } else { Typos::Disabled };
    options.last_match = input.last_match;
    let engine = skim::matcher::Matcher::create_engine_factory(&options).create_engine_with_case(pattern, case);
    let item = choice.to_string();
    assert_eq!(
        engine.match_rank(&item),
        engine.match_item(&item).map(|r| r.rank),
        "match_rank vs match_item rank, engine {engine}"
    );
});
