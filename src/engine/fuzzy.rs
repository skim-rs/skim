use std::cmp::min;
use std::fmt::{Display, Error, Formatter};
use std::sync::Arc;

use crate::fuzzy_matcher::frizbee::FrizbeeMatcher;
use crate::fuzzy_matcher::{
    FuzzyMatcher, IndexType, ScoreType, clangd::ClangdMatcher, fzy::FzyMatcher, skim::SkimMatcherV2,
};

use crate::item::RankBuilder;
use crate::{CaseMatching, MatchEngine, Typos};
use crate::{MatchRange, MatchResult, SkimItem};

//------------------------------------------------------------------------------
/// Fuzzy matching algorithm to use
#[derive(Debug, Copy, Clone, Default, PartialEq)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[cfg_attr(feature = "cli", clap(rename_all = "snake_case"))]
pub enum FuzzyAlgorithm {
    /// Original skim fuzzy matching algorithm (v1)
    SkimV1,
    /// Improved skim fuzzy matching algorithm (v2, default)
    #[default]
    SkimV2,
    /// Clangd fuzzy matching algorithm
    Clangd,
    /// Fzy matching algorithm (https://github.com/jhawthorn/fzy)
    Fzy,
    /// Frizbee matching algorithm, typo resistant
    /// Will fallback to SkimV2 if the feature is not enabled
    Frizbee,
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
    /// - `Typos::Smart`: adaptive (pattern_length / 4)
    /// - `Typos::Fixed(n)`: exactly n typos allowed
    typos: Typos,
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

    /// Compute the effective max_typos for the given query.
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
        use crate::fuzzy_matcher::skim::SkimMatcher;
        #[allow(unused_mut)]
        let mut algorithm = self.algorithm;
        let max_typos = self.effective_max_typos();
        let matcher: Box<dyn FuzzyMatcher> = match algorithm {
            FuzzyAlgorithm::SkimV1 => {
                debug!("Initialized SkimV1 algorithm");
                Box::new(SkimMatcher::default())
            }
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
            FuzzyAlgorithm::Frizbee => Box::new(FrizbeeMatcher::default().case(self.case).max_typos(max_typos)),
            FuzzyAlgorithm::Fzy => {
                let matcher = FzyMatcher::default().max_typos(max_typos);
                let matcher = match self.case {
                    CaseMatching::Respect => matcher.respect_case(),
                    CaseMatching::Ignore => matcher.ignore_case(),
                    CaseMatching::Smart => matcher.smart_case(),
                };
                debug!("Initialized Fzy algorithm (max_typos: {:?})", max_typos);
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
    pub fn builder() -> FuzzyEngineBuilder {
        FuzzyEngineBuilder::default()
    }

    fn fuzzy_match(&self, choice: &str, pattern: &str) -> Option<(ScoreType, Vec<IndexType>)> {
        if pattern.is_empty() {
            return Some((0, Vec::new()));
        } else if choice.is_empty() {
            return None;
        }

        self.matcher.fuzzy_indices(choice, pattern)
    }
}

impl MatchEngine for FuzzyEngine {
    fn match_item(&self, item: &dyn SkimItem) -> Option<MatchResult> {
        // iterate over all matching fields:
        let mut matched_result = None;
        let item_text = item.text();
        let default_range = [(0, item_text.len())];
        for &(start, end) in item.get_matching_ranges().unwrap_or(&default_range) {
            let start = min(start, item_text.len());
            let end = min(end, item_text.len());
            matched_result = self.fuzzy_match(&item_text[start..end], &self.query).map(|(s, vec)| {
                if start != 0 {
                    let start_char = &item_text[..start].chars().count();
                    (s, vec.iter().map(|x| x + start_char).collect())
                } else {
                    (s, vec)
                }
            });

            if matched_result.is_some() {
                break;
            }
        }

        let (score, matched_range) = matched_result?;

        let begin = *matched_range.first().unwrap_or(&0);
        let end = *matched_range.last().unwrap_or(&0);

        let item_len = item_text.len();

        // Use individual character indices for highlighting instead of byte range
        // This allows each matched character to be highlighted individually
        let matched_range = MatchRange::Chars(matched_range);

        Some(MatchResult {
            rank: self
                .rank_builder
                .build_rank(score as i32, begin, end, item_len, item.get_index()),
            matched_range,
        })
    }
}

impl Display for FuzzyEngine {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "(Fuzzy: {})", self.query)
    }
}
