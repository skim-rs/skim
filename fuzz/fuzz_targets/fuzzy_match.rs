#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use skim::fuzzy_matcher::FuzzyMatcher;
use skim::fuzzy_matcher::clangd::ClangdMatcher;
use skim::fuzzy_matcher::fzy::FzyMatcher;
use skim::fuzzy_matcher::skim::SkimMatcherV2;

// Fuzzes the fuzzy matching algorithms directly on arbitrary unicode
// (choice, pattern) pairs. These run a lot of hand-written index/DP-matrix
// arithmetic over `char` boundaries, so they're prone to panics (overflow,
// out-of-bounds) on adversarial unicode input, and the returned match
// indices must always be valid character indices into `choice`.
#[derive(Arbitrary, Debug)]
struct MatchInput<'a> {
    choice: &'a str,
    pattern: &'a str,
}

fuzz_target!(|input: MatchInput| {
    let skim_matcher = SkimMatcherV2::default();
    let fzy_matcher = FzyMatcher::default();
    let clangd_matcher = ClangdMatcher::default();

    let matchers: [&dyn FuzzyMatcher; 3] = [&skim_matcher, &fzy_matcher, &clangd_matcher];

    let num_chars = input.choice.chars().count();
    for matcher in matchers {
        if let Some((_score, indices)) = matcher.fuzzy_indices(input.choice, input.pattern) {
            for &idx in &indices {
                assert!(
                    idx < num_chars,
                    "match index {idx} out of bounds for choice with {num_chars} chars"
                );
            }
        }
    }
});
