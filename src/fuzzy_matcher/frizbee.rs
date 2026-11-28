//! Matcher using <https://crates.io/crates/frizbee>
use std::cell::RefCell;

use frizbee::{Config, Matcher};

use crate::CaseMatching;
use crate::fuzzy_matcher::{FuzzyMatcher, MatchIndices, ScoreType};

thread_local! {
    /// One reusable frizbee Matcher per thread, with the pattern it was built for
    static LOCAL_MATCHER: RefCell<Option<(Matcher, String)>> = const { RefCell::new(None) };
}

/// Matcher using frizbee,
/// the same one that `blink.cmp` uses in neovim
/// credits to @saghen
pub struct FrizbeeMatcher {
    config: Config,
}
impl Default for FrizbeeMatcher {
    fn default() -> Self {
        Self {
            config: Config {
                casing: CaseMatching::Respect.into(),
                ..Config::default()
            },
        }
    }
}

impl FrizbeeMatcher {
    /// Set the max typos to use
    #[must_use]
    pub fn max_typos(mut self, typos: Option<usize>) -> Self {
        self.config.max_typos = Some(typos.map_or(0, |x| u16::try_from(x).unwrap_or(u16::MAX)));
        self
    }

    /// Set the case matching strategy
    #[must_use]
    pub fn case(mut self, case: CaseMatching) -> Self {
        self.config.casing = case.into();
        self
    }

    /// Run `f` with a thread-local matcher configured with this matcher's config
    /// and the given needle.
    fn with_matcher<R>(&self, pattern: &str, f: impl FnOnce(&mut Matcher) -> R) -> R {
        LOCAL_MATCHER.with(|cell| {
            let mut slot = cell.borrow_mut();

            let (matcher, last_pattern) = slot.get_or_insert_with(|| (Matcher::new("", &self.config), String::new()));
            // `set_config` clones and `set_pattern` allocates on every call: skip
            // them when nothing changed (the same query is matched on every item).
            if matcher.config() != &self.config {
                matcher.set_config(self.config.clone());
            }
            if last_pattern != pattern {
                matcher.set_pattern(pattern);
                pattern.clone_into(last_pattern);
            }
            f(matcher)
        })
    }
}

impl FuzzyMatcher for FrizbeeMatcher {
    fn fuzzy_indices(&self, choice: &str, pattern: &str) -> Option<(ScoreType, MatchIndices)> {
        self.with_matcher(pattern, |m| {
            m.match_one_indices(choice, 0).map(|mut hit| {
                hit.indices.reverse();
                (
                    hit.score.into(),
                    hit.indices.into_iter().map(|index| index as usize).collect(),
                )
            })
        })
    }

    fn fuzzy_match(&self, choice: &str, pattern: &str) -> Option<i64> {
        self.with_matcher(pattern, |m| m.match_one(choice, 0).map(|hit| hit.score.into()))
    }

    /// Same as the first / last index of `fuzzy_indices`, without building the
    /// index list. Not `match_one` + a computed begin: for a single item,
    /// frizbee's SIMD scorer is not faster than its indices path, and frizbee
    /// gives no begin column.
    fn fuzzy_match_range(&self, choice: &str, pattern: &str) -> Option<(i64, usize, usize)> {
        self.with_matcher(pattern, |m| {
            m.match_one_indices(choice, 0).map(|hit| {
                // frizbee gives the indices in reverse order.
                let first = hit.indices.last().map_or(0, |&i| i as usize);
                let last = hit.indices.first().map_or(0, |&i| i as usize);
                (hit.score.into(), first, last)
            })
        })
    }
}

impl From<CaseMatching> for frizbee::CaseMatching {
    fn from(case: CaseMatching) -> Self {
        match case {
            CaseMatching::Respect => frizbee::CaseMatching::Respect,
            CaseMatching::Ignore => frizbee::CaseMatching::Ignore,
            CaseMatching::Smart => frizbee::CaseMatching::Smart,
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
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

    #[test]
    fn fuzzy_indices_ignore_case() {
        // Ignore case → matching_case_bonus is 0 in fuzzy_indices.
        let m = FrizbeeMatcher::default().case(CaseMatching::Ignore);
        assert!(m.fuzzy_indices("FOOBAR", "foo").is_some());
    }

    #[test]
    fn fuzzy_indices_no_match_returns_none() {
        // A non-subsequence pattern exercises the None branch.
        let m = FrizbeeMatcher::default();
        assert!(m.fuzzy_indices("foobar", "zzz").is_none());
    }

    #[test]
    fn fuzzy_match_respect_and_smart_case() {
        // fuzzy_match (score-only) across the Respect and Smart case arms.
        let respect = FrizbeeMatcher::default().case(CaseMatching::Respect);
        assert!(respect.fuzzy_match("FooBar", "Foo").is_some());

        let smart = FrizbeeMatcher::default().case(CaseMatching::Smart);
        assert!(smart.fuzzy_match("FooBar", "Foo").is_some());
        assert!(smart.fuzzy_match("foobar", "foo").is_some());
    }

    #[test]
    fn match_range_is_first_and_last_index() {
        for typos in [0, 1] {
            let m = FrizbeeMatcher::default()
                .case(CaseMatching::Smart)
                .max_typos(Some(typos));
            for (choice, pattern) in [
                ("src/foo/bar.rs", "fbr"),
                ("foobar", "fxo"),
                ("abc", "zzz"),
                ("héllo", "lo"),
            ] {
                let expected = m
                    .fuzzy_indices(choice, pattern)
                    .map(|(s, idx)| (s, idx.first().copied().unwrap_or(0), idx.last().copied().unwrap_or(0)));
                assert_eq!(m.fuzzy_match_range(choice, pattern), expected, "{choice} {pattern}");
            }
        }
    }

    /// The thread-local matcher is reused: a new pattern or config must apply.
    #[test]
    fn cached_matcher_follows_pattern_and_config_changes() {
        let respect = FrizbeeMatcher::default().case(CaseMatching::Respect);
        assert!(respect.fuzzy_match("foobar", "foo").is_some());
        assert!(respect.fuzzy_match("foobar", "zzz").is_none());
        assert!(respect.fuzzy_match("foobar", "FOO").is_none());
        let ignore = FrizbeeMatcher::default().case(CaseMatching::Ignore);
        assert!(ignore.fuzzy_match("foobar", "FOO").is_some());
        assert!(respect.fuzzy_match("foobar", "FOO").is_none());
    }
}
