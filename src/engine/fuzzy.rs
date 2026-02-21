use std::cmp::min;
use std::fmt::{Display, Error, Formatter};
use std::sync::Arc;

use crate::fuzzy_matcher::MatchIndices;
use crate::fuzzy_matcher::frizbee::FrizbeeMatcher;
use crate::fuzzy_matcher::skim_v3::SkimV3Matcher;
use crate::fuzzy_matcher::{FuzzyMatcher, clangd::ClangdMatcher, fzy::FzyMatcher, skim::SkimMatcherV2};

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
    Frizbee,
    /// SkimV3: Needleman-Wunsch + Damerau-Levenshtein variation with affine gaps
    SkimV3,
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
            FuzzyAlgorithm::SkimV3 => {
                let mut matcher = SkimV3Matcher::default();
                matcher.case = self.case;
                matcher.allow_typos = !matches!(self.typos, Typos::Disabled);
                debug!("Initialized SkimV3 algorithm");
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
}

impl MatchEngine for FuzzyEngine {
    fn match_item(&self, item: &dyn SkimItem) -> Option<MatchResult> {
        let item_text = item.text();
        let default_range = [(0, item_text.len())];

        let mut matched_result = None;
        for &(start, end) in item.get_matching_ranges().unwrap_or(&default_range) {
            let start = min(start, item_text.len());
            let end = min(end, item_text.len());

            let result = if self.query.is_empty() {
                Some((0i64, MatchIndices::new()))
            } else if item_text[start..end].is_empty() {
                None
            } else {
                self.matcher.fuzzy_indices(&item_text[start..end], &self.query)
            };

            matched_result = result.map(|(s, vec)| {
                if start != 0 {
                    let start_char = item_text[..start].chars().count();
                    (s, vec.iter().map(|x| x + start_char).collect::<MatchIndices>())
                } else {
                    (s, vec)
                }
            });

            if matched_result.is_some() {
                break;
            }
        }

        let (score, matched_indices) = matched_result?;
        let begin = *matched_indices.first().unwrap_or(&0);
        let end = *matched_indices.last().unwrap_or(&0);
        let item_len = item_text.len();
        let matched_range = MatchRange::Chars(matched_indices);

        Some(MatchResult {
            rank: self
                .rank_builder
                .build_rank(score as i32, begin, end, item_len, item.get_index()),
            matched_range,
        })
    }

    fn match_items(&self, items: &[Arc<dyn SkimItem>]) -> Vec<Option<MatchResult>> {
        // Delegate to per-item matching (default implementation).
        // SIMD batch score-only can't skip enough full-DP calls to be worthwhile:
        // with typos enabled, nearly all prefilter-passing items score > 0,
        // so the 2-pass approach just adds overhead.
        items.iter().map(|item| self.match_item(item.as_ref())).collect()
    }
}

impl Display for FuzzyEngine {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "(Fuzzy: {})", self.query)
    }
}
