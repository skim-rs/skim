use std::cmp::min;
use std::fmt::{Display, Error, Formatter};
use std::sync::Arc;

use crate::fuzzy_matcher::FuzzyMatcher;
use crate::fuzzy_matcher::arinae::ArinaeMatcher;
use crate::fuzzy_matcher::clangd::ClangdMatcher;
#[cfg(frizbee)]
use crate::fuzzy_matcher::frizbee::FrizbeeMatcher;
use crate::fuzzy_matcher::fzy::FzyMatcher;
use crate::fuzzy_matcher::skim::SkimMatcherV2;

use crate::item::RankBuilder;
use crate::{CaseMatching, MatchEngine, MatchRange, MatchResult, SkimItem, Typos};

//------------------------------------------------------------------------------
/// Fuzzy matching algorithm to use
#[derive(Debug, Copy, Clone, Default, PartialEq)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[cfg_attr(feature = "cli", clap(rename_all = "snake_case"))]
pub enum FuzzyAlgorithm {
    /// Arinae: typo-resistant & natural algorithm, default
    #[cfg_attr(feature = "cli", clap(alias = "ari"))]
    #[default]
    Arinae,
    /// Clangd fuzzy matching algorithm
    Clangd,
    /// Fzy matching algorithm (<https://github.com/jhawthorn/fzy>)
    Fzy,
    /// Frizbee matching algorithm, typo resistant (`x86_64` and `aarch64` only)
    #[cfg(frizbee)]
    Frizbee,
    /// Previous skim fuzzy matching algorithm (v2)
    SkimV2,
}

const BYTES_1M: usize = 1024 * 1024 * 1024;

//------------------------------------------------------------------------------
// Fuzzy engine
#[derive(Default)]
pub struct FuzzyEngineBuilder {
    query: String,
    case: CaseMatching,
    algorithm: FuzzyAlgorithm,
    rank_builder: Arc<RankBuilder>,
    /// Typo tolerance configuration:
    /// - `Typos::Disabled`: no typo tolerance
    /// - `Typos::Smart`: adaptive (`pattern_length` / 4)
    /// - `Typos::Fixed(n)`: exactly n typos allowed
    typos: Typos,
    /// When true, prefer the last (rightmost) occurrence on tied scores.
    last_match: bool,
}

impl FuzzyEngineBuilder {
    pub fn query(mut self, query: &str) -> Self {
        self.query = query.to_string();
        self
    }

    pub fn case(mut self, case: CaseMatching) -> Self {
        self.case = case;
        self
    }

    pub fn algorithm(mut self, algorithm: FuzzyAlgorithm) -> Self {
        self.algorithm = algorithm;
        self
    }

    pub fn rank_builder(mut self, rank_builder: Arc<RankBuilder>) -> Self {
        self.rank_builder = rank_builder;
        self
    }

    pub fn typos(mut self, typos: Typos) -> Self {
        self.typos = typos;
        self
    }

    /// No-op: `fuzzy_match_range` is now always used (`ByteRange`).
    /// Kept for API backward compatibility.
    pub fn filter_mode(self, _filter_mode: bool) -> Self {
        self
    }

    pub fn last_match(mut self, last_match: bool) -> Self {
        self.last_match = last_match;
        self
    }

    /// Compute the effective `max_typos` for the given query.
    ///
    /// - `Typos::Disabled` → `None` (no typo tolerance)
    /// - `Typos::Smart` → adaptive: `Some(query.chars().count() / 4)`
    /// - `Typos::Fixed(n)` → `Some(n)`
    fn effective_max_typos(&self) -> Option<usize> {
        match self.typos {
            Typos::Disabled => None,
            Typos::Smart => Some(self.query.chars().count().saturating_div(4)),
            Typos::Fixed(n) => Some(n),
        }
    }

    #[allow(deprecated)]
    pub fn build(self) -> FuzzyEngine {
        #[allow(unused_mut)]
        let mut algorithm = self.algorithm;
        let max_typos = self.effective_max_typos();
        let matcher: Box<dyn FuzzyMatcher> = match algorithm {
            FuzzyAlgorithm::SkimV2 => {
                let matcher = SkimMatcherV2::default().element_limit(BYTES_1M);
                let matcher = match self.case {
                    CaseMatching::Respect => matcher.respect_case(),
                    CaseMatching::Ignore => matcher.ignore_case(),
                    CaseMatching::Smart => matcher.smart_case(),
                };
                debug!("Initialized SkimV2 algorithm");
                Box::new(matcher)
            }
            FuzzyAlgorithm::Clangd => {
                let matcher = ClangdMatcher::default();
                let matcher = match self.case {
                    CaseMatching::Respect => matcher.respect_case(),
                    CaseMatching::Ignore => matcher.ignore_case(),
                    CaseMatching::Smart => matcher.smart_case(),
                };
                debug!("Initialized Clangd algorithm");
                Box::new(matcher)
            }
            #[cfg(frizbee)]
            FuzzyAlgorithm::Frizbee => Box::new(FrizbeeMatcher::default().case(self.case).max_typos(max_typos)),
            FuzzyAlgorithm::Fzy => {
                let matcher = FzyMatcher::default().max_typos(max_typos);
                let matcher = match self.case {
                    CaseMatching::Respect => matcher.respect_case(),
                    CaseMatching::Ignore => matcher.ignore_case(),
                    CaseMatching::Smart => matcher.smart_case(),
                };
                debug!("Initialized Fzy algorithm (max_typos: {max_typos:?})");
                Box::new(matcher)
            }
            FuzzyAlgorithm::Arinae => {
                let matcher = ArinaeMatcher::new(self.case, !matches!(self.typos, Typos::Disabled), self.last_match);
                debug!("Initialized Arinae algorithm");
                Box::new(matcher)
            }
        };

        FuzzyEngine {
            matcher,
            query: self.query,
            rank_builder: self.rank_builder,
        }
    }
}

/// The fuzzy matching engine
pub struct FuzzyEngine {
    query: String,
    matcher: Box<dyn FuzzyMatcher>,
    rank_builder: Arc<RankBuilder>,
}

impl FuzzyEngine {
    /// Returns a default builder for chaining
    #[must_use]
    pub fn builder() -> FuzzyEngineBuilder {
        FuzzyEngineBuilder::default()
    }
}

impl MatchEngine for FuzzyEngine {
    fn match_item(&self, item: &dyn SkimItem) -> Option<MatchResult> {
        let item_text = item.text();
        let default_range = [(0, item_text.len())];

        let mut best: Option<(i64, Vec<usize>)> = None;
        for &(start, end) in item.get_matching_ranges().unwrap_or(&default_range) {
            let start = min(start, item_text.len());
            let end = min(end, item_text.len());

            let result = if self.query.is_empty() {
                Some((0i64, vec![]))
            } else if item_text[start..end].is_empty() {
                None
            } else {
                self.matcher
                    .fuzzy_indices(&item_text[start..end], &self.query)
                    .map(|(s, indices)| {
                        let offset = if start != 0 {
                            item_text[..start].chars().count()
                        } else {
                            0
                        };
                        let indices = indices.into_iter().map(|i| i + offset).collect();
                        (s, indices)
                    })
            };

            if result.is_some() {
                best = result;
                break;
            }
        }

        let (score, indices) = best?;
        let begin = indices.first().copied().unwrap_or(0);
        let end_excl = indices.last().map_or(0, |&i| i + 1);

        let matched_range = if indices.is_empty() {
            MatchRange::CharRange(0, 0)
        } else {
            MatchRange::Chars(indices)
        };

        Some(MatchResult {
            rank: self
                .rank_builder
                .build_rank(i32::try_from(score).unwrap_or(i32::MAX), begin, end_excl, &item_text),
            matched_range,
        })
    }
}

impl Display for FuzzyEngine {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "(Fuzzy: {})", self.query)
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn effective_max_typos_per_variant() {
        let disabled = FuzzyEngine::builder().query("hello").typos(Typos::Disabled);
        assert_eq!(disabled.effective_max_typos(), None);

        let smart = FuzzyEngine::builder().query("hello").typos(Typos::Smart);
        assert_eq!(smart.effective_max_typos(), Some(1)); // 5 / 4 = 1

        let fixed = FuzzyEngine::builder().query("hello").typos(Typos::Fixed(3));
        assert_eq!(fixed.effective_max_typos(), Some(3));
    }

    /// Every algorithm × case combination should build and match a basic query.
    #[test]
    fn builds_every_algorithm_and_case() {
        let algorithms = [
            FuzzyAlgorithm::SkimV2,
            FuzzyAlgorithm::Clangd,
            FuzzyAlgorithm::Fzy,
            FuzzyAlgorithm::Arinae,
        ];
        let cases = [CaseMatching::Respect, CaseMatching::Ignore, CaseMatching::Smart];
        for algo in algorithms {
            for case in cases {
                let engine = FuzzyEngine::builder().query("foo").algorithm(algo).case(case).build();
                assert!(
                    engine.match_item(&"foobar".to_string()).is_some(),
                    "algo {algo:?} case {case:?} should match"
                );
            }
        }
    }

    #[test]
    fn empty_query_yields_empty_char_range() {
        let engine = FuzzyEngine::builder().query("").build();
        let result = engine.match_item(&"anything".to_string()).unwrap();
        assert_eq!(result.matched_range, MatchRange::CharRange(0, 0));
    }

    #[test]
    fn matching_query_yields_char_indices() {
        let engine = FuzzyEngine::builder().query("fb").build();
        let result = engine.match_item(&"foobar".to_string()).unwrap();
        assert!(matches!(result.matched_range, MatchRange::Chars(_)));
    }

    #[test]
    fn no_match_returns_none() {
        let engine = FuzzyEngine::builder().query("zzz").build();
        assert!(engine.match_item(&"foobar".to_string()).is_none());
    }

    #[test]
    fn display_shows_query() {
        let engine = FuzzyEngine::builder().query("foo").build();
        assert_eq!(format!("{engine}"), "(Fuzzy: foo)");
    }
}
