//! Matcher using <https://crates.io/crates/frizbee>
use frizbee::Scoring;
use frizbee::smith_waterman::SmithWatermanMatcher;

use crate::CaseMatching;
use crate::fuzzy_matcher::{FuzzyMatcher, MatchIndices, ScoreType};

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
    #[must_use]
    pub fn max_typos(mut self, typos: Option<usize>) -> Self {
        self.max_typos = Some(typos.map_or(0, |x| u16::try_from(x).unwrap_or(u16::MAX)));
        self
    }
    /// Set the case, will be converted to a `matching_case_bonus`
    #[must_use]
    pub fn case(mut self, case: CaseMatching) -> Self {
        self.case = case;
        self
    }
}

impl FuzzyMatcher for FrizbeeMatcher {
    fn fuzzy_indices(&self, choice: &str, pattern: &str) -> Option<(ScoreType, MatchIndices)> {
        let scoring = Scoring {
            matching_case_bonus: match self.case {
                CaseMatching::Respect => RESPECT_CASE_BONUS,
                CaseMatching::Ignore => 0,
                CaseMatching::Smart => {
                    if pattern.chars().any(char::is_uppercase) {
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
            .and_then(|(m, mut indices)| {
                debug!("{choice}: {m} ({})", scoring.matching_case_bonus);
                if m > scoring.matching_case_bonus.saturating_mul(
                    pattern
                        .chars()
                        .count()
                        .saturating_sub(self.max_typos.unwrap_or(0).into())
                        .try_into()
                        .unwrap(),
                ) {
                    indices.reverse();
                    Some((m.into(), MatchIndices::from(indices)))
                } else {
                    None
                }
            })
    }
    fn fuzzy_match(&self, choice: &str, pattern: &str) -> Option<i64> {
        let scoring = Scoring {
            matching_case_bonus: match self.case {
                CaseMatching::Respect => RESPECT_CASE_BONUS,
                CaseMatching::Ignore => 0,
                CaseMatching::Smart => {
                    if pattern.chars().any(char::is_uppercase) {
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
            .match_haystack(choice.as_bytes(), self.max_typos)
            .map(ScoreType::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fuzzy_matcher::FuzzyMatcher;

    #[test]
    fn matches_subsequence() {
        let m = FrizbeeMatcher::default();
        assert!(m.fuzzy_match("foobar", "foo").is_some());
        assert!(m.fuzzy_indices("foobar", "foo").is_some());
    }

    #[test]
    fn respect_case_variant() {
        let m = FrizbeeMatcher::default().case(CaseMatching::Respect);
        assert!(m.fuzzy_indices("FooBar", "Foo").is_some());
    }

    #[test]
    fn smart_case_variant() {
        let m = FrizbeeMatcher::default().case(CaseMatching::Smart);
        // Uppercase pattern triggers the case bonus branch.
        assert!(m.fuzzy_indices("FooBar", "Foo").is_some());
        // Lowercase pattern -> no bonus.
        assert!(m.fuzzy_indices("foobar", "foo").is_some());
    }

    #[test]
    fn ignore_case_variant() {
        let m = FrizbeeMatcher::default().case(CaseMatching::Ignore);
        assert!(m.fuzzy_match("FOOBAR", "foo").is_some());
    }

    #[test]
    fn max_typos_tolerates_mismatch() {
        let m = FrizbeeMatcher::default().max_typos(Some(1));
        assert!(m.fuzzy_match("foobar", "fxo").is_some());
    }
}
