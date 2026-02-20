//! Matcher using https://crates.io/crates/frizbee
use frizbee::{Scoring, smith_waterman::SmithWatermanMatcher};

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
    case: CaseMatching,
    max_typos: Option<u16>,
}

impl FrizbeeMatcher {
    /// Set the max typos to use
    pub fn max_typos(mut self, typos: Option<usize>) -> Self {
        self.max_typos = Some(typos.map(|x| x.try_into().unwrap()).unwrap_or(0));
        self
    }
    /// Set the case, will be converted to a matching_case_bonus
    pub fn case(mut self, case: CaseMatching) -> Self {
        self.case = case;
        self
    }
}

impl FuzzyMatcher for FrizbeeMatcher {
    fn fuzzy_indices(&self, choice: &str, pattern: &str) -> Option<(ScoreType, Vec<IndexType>)> {
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
            .match_haystack_indices(choice.as_bytes(), 0, self.max_typos)
            .and_then(|(m, indices)| {
                debug!("{choice}: {m} ({})", scoring.matching_case_bonus);
                if m > scoring.matching_case_bonus.saturating_mul(
                    pattern
                        .chars()
                        .count()
                        .saturating_sub(self.max_typos.unwrap_or(0).into())
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
