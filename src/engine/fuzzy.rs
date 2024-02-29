use std::fmt::{Display, Error, Formatter};
use std::sync::Arc;

use fuzzy_matcher::clangd::ClangdMatcher;
use fuzzy_matcher::simple::SimpleMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use crate::item::RankBuilder;
use crate::{CaseMatching, MatchEngine};
use crate::{MatchRange, MatchResult, SkimItem};

//------------------------------------------------------------------------------
#[derive(Debug, Copy, Clone, Default)]
pub enum FuzzyAlgorithm {
    SkimV1,
    #[default]
    SkimV2,
    Clangd,
    Simple,
}

impl FuzzyAlgorithm {
    pub fn of(algorithm: &str) -> Self {
        match algorithm.to_ascii_lowercase().as_ref() {
            "skim_v1" => FuzzyAlgorithm::SkimV1,
            "skim_v2" | "skim" => FuzzyAlgorithm::SkimV2,
            "clangd" => FuzzyAlgorithm::Clangd,
            "simple" => FuzzyAlgorithm::Simple,
            _ => FuzzyAlgorithm::SkimV2,
        }
    }
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

    #[allow(deprecated)]
    pub fn build(self) -> FuzzyEngine {
        let matcher: Box<dyn FuzzyMatcher> = match self.algorithm {
            FuzzyAlgorithm::SkimV1 => Box::<fuzzy_matcher::skim::SkimMatcher>::default(),
            FuzzyAlgorithm::SkimV2 => {
                let matcher = SkimMatcherV2::default().element_limit(BYTES_1M);
                let matcher = match self.case {
                    CaseMatching::Respect => matcher.respect_case(),
                    CaseMatching::Ignore => matcher.ignore_case(),
                    CaseMatching::Smart => matcher.smart_case(),
                };
                Box::new(matcher)
            }
            FuzzyAlgorithm::Clangd => {
                let matcher = ClangdMatcher::default();
                let matcher = match self.case {
                    CaseMatching::Respect => matcher.respect_case(),
                    CaseMatching::Ignore => matcher.ignore_case(),
                    CaseMatching::Smart => matcher.smart_case(),
                };
                Box::new(matcher)
            }
            FuzzyAlgorithm::Simple => {
                let matcher = SimpleMatcher::default();
                let matcher = match self.case {
                    CaseMatching::Respect => matcher.respect_case(),
                    CaseMatching::Ignore => matcher.ignore_case(),
                    CaseMatching::Smart => matcher.smart_case(),
                };
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

pub struct FuzzyEngine {
    query: String,
    matcher: Box<dyn FuzzyMatcher>,
    rank_builder: Arc<RankBuilder>,
}

impl FuzzyEngine {
    pub fn builder() -> FuzzyEngineBuilder {
        FuzzyEngineBuilder::default()
    }

    fn fuzzy_match(&self, choice: &str, pattern: &str) -> Option<(i64, Vec<usize>)> {
        if pattern.is_empty() {
            return Some((0, Vec::new()));
        }

        if choice.is_empty() {
            return None;
        }

        self.matcher.fuzzy_indices(choice, pattern)
    }
}

impl MatchEngine for FuzzyEngine {
    fn match_item(&self, item: &dyn SkimItem) -> Option<MatchResult> {
        // iterate over all matching fields:
        let item_text = item.text();
        let item_len = item_text.chars().count();
        let query_text = &self.query;
        let default_range = [(0, item_len)];

        let matched_result: Option<(i64, Vec<usize>)> = item
            .get_matching_ranges()
            .unwrap_or(&default_range)
            .iter()
            .map(|(start, end)| {
                let start = std::cmp::min(*start, item_len);
                let end = std::cmp::min(*end, item_len);
                let choice_range = &item_text[start..end];
                (start, choice_range)
            })
            .find_map(|(start, choice_range)| {
                self.fuzzy_match(choice_range, query_text).map(|(score, indices)| {
                    if start != 0 {
                        let start_char = &item_text[..start].len();
                        return (score, indices.iter().map(|x| x + start_char).collect());
                    }

                    (score, indices)
                })
            });

        matched_result.map(|(score, matched_range)| {
            let begin = *matched_range.first().unwrap_or(&0);
            let end = *matched_range.last().unwrap_or(&0);

            MatchResult {
                rank: self.rank_builder.build_rank(score as i32, begin, end, item_len),
                matched_range: MatchRange::Chars(matched_range),
            }
        })
    }
}

impl Display for FuzzyEngine {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "(Fuzzy: {})", self.query)
    }
}
