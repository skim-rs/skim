//! Matcher using https://crates.io/crates/frizbee
use frizbee::{Scoring, smith_waterman::simd::SmithWatermanMatcher};

use crate::{
    CaseMatching,
    fuzzy_matcher::{FuzzyMatcher, IndexType, ScoreType},
};

const RESPECT_CASE_BONUS: u16 = 10000;

/// Matcher using frizbee,
/// the same one that `blink.cmp` uses in neovim
/// credits to @saghen
#[derive(Default)]
pub struct FrizbeeMatcher {
    /// The case for matching
    /// Will be translated into a matching_case_bonus
    pub case: CaseMatching,
}

impl FuzzyMatcher for FrizbeeMatcher {
    fn fuzzy_indices(&self, choice: &str, pattern: &str) -> Option<(ScoreType, Vec<IndexType>)> {
        let max_typos: u16 = pattern.chars().count().saturating_div(4).try_into().unwrap();
        let scoring = Scoring {
            matching_case_bonus: match self.case {
                CaseMatching::Respect => RESPECT_CASE_BONUS,
                CaseMatching::Ignore => 0,
                CaseMatching::Smart => {
                    if pattern.chars().any(|c| c.is_uppercase()) {
                        RESPECT_CASE_BONUS
                    } else {
                        0
                    }
                }
            },
            ..Default::default()
        };
        let mut matcher = SmithWatermanMatcher::new(pattern.as_bytes(), &scoring);
        matcher
            .match_haystack_indices(choice.as_bytes(), 0, Some(max_typos))
            .and_then(|(m, indices)| {
                debug!("{choice}: {m} ({})", scoring.matching_case_bonus);
                if m > scoring.matching_case_bonus.saturating_mul(
                    pattern
                        .chars()
                        .count()
                        .saturating_sub(max_typos as usize)
                        .try_into()
                        .unwrap(),
                ) {
                    Some((m.into(), indices))
                } else {
                    None
                }
            })
    }
}
