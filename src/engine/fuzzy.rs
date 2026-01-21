use std::cmp::min;
use std::fmt::{Display, Error, Formatter};
use std::sync::Arc;

#[cfg(feature = "nightly-frizbee")]
use crate::fuzzy_matcher::frizbee::FrizbeeMatcher;
use crate::fuzzy_matcher::{FuzzyMatcher, IndexType, ScoreType, clangd::ClangdMatcher, skim::SkimMatcherV2};

use crate::item::RankBuilder;
use crate::{CaseMatching, MatchEngine};
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
    split_match: Option<char>,
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

    pub fn split_match(mut self, split_match: Option<char>) -> Self {
        self.split_match = split_match;
        self
    }

    #[allow(deprecated)]
    pub fn build(self) -> FuzzyEngine {
        use crate::fuzzy_matcher::skim::SkimMatcher;
        #[allow(unused_mut)]
        let mut algorithm = self.algorithm;
        #[cfg(not(feature = "nightly-frizbee"))]
        if algorithm == FuzzyAlgorithm::Frizbee {
            warn!("Frizbee algorithm not enabled, using SkimV2");
            algorithm = FuzzyAlgorithm::SkimV2;
        }
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
            FuzzyAlgorithm::Frizbee => {
                #[cfg(not(feature = "nightly-frizbee"))]
                unreachable!();
                #[cfg(feature = "nightly-frizbee")]
                Box::new(FrizbeeMatcher::default())
            }
        };

        FuzzyEngine {
            matcher,
            query: self.query,
            rank_builder: self.rank_builder,
            split_match: self.split_match,
        }
    }
}

/// The fuzzy matching engine
pub struct FuzzyEngine {
    query: String,
    matcher: Box<dyn FuzzyMatcher>,
    rank_builder: Arc<RankBuilder>,
    split_match: Option<char>,
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

        if let Some(split_char) = self.split_match {
            return self.split_fuzzy_match(choice, pattern, split_char);
        }

        self.matcher.fuzzy_indices(choice, pattern)
    }

    /// Performs split matching based on a delimiter character.
    ///
    /// Behavior:
    /// - If the delimiter is NOT in the pattern: match pattern against the whole choice
    /// - If the delimiter IS in the pattern: split both on the FIRST delimiter, then
    ///   match pattern_before against choice_before AND pattern_after against choice_after
    /// - Only the first delimiter in the pattern is considered; subsequent delimiters are
    ///   treated as part of the second half
    /// - If delimiter is in pattern but not in choice, no match
    fn split_fuzzy_match(&self, choice: &str, pattern: &str, split_char: char) -> Option<(ScoreType, Vec<IndexType>)> {
        // Find the position of the split character in the pattern (query)
        let pattern_split_pos = pattern.find(split_char);

        // Find the position of the split character in the choice (item)
        // We need char index, not byte index, for proper index calculation
        let choice_split_char_idx = choice.chars().position(|c| c == split_char);

        match pattern_split_pos {
            None => self.matcher.fuzzy_indices(choice, pattern),
            Some(pattern_byte_pos) => {
                // Delimiter in pattern: split both and match separately
                let pattern_before = &pattern[..pattern_byte_pos];
                let pattern_after = &pattern[pattern_byte_pos + split_char.len_utf8()..];

                match choice_split_char_idx {
                    Some(choice_char_idx) => {
                        // Get byte position for slicing choice
                        let choice_byte_pos = choice
                            .char_indices()
                            .nth(choice_char_idx)
                            .map(|(i, _)| i)
                            .unwrap_or(choice.len());
                        let choice_before = &choice[..choice_byte_pos];
                        let choice_after = &choice[choice_byte_pos + split_char.len_utf8()..];

                        // Match the "before" parts
                        let before_match = if pattern_before.is_empty() {
                            Some((0, Vec::new()))
                        } else {
                            self.matcher.fuzzy_indices(choice_before, pattern_before)
                        };

                        // Match the "after" parts
                        let after_match = if pattern_after.is_empty() {
                            Some((0, Vec::new()))
                        } else {
                            self.matcher.fuzzy_indices(choice_after, pattern_after)
                        };

                        // Both must match
                        match (before_match, after_match) {
                            (Some((score_before, indices_before)), Some((score_after, indices_after))) => {
                                // Combine scores
                                let total_score = score_before + score_after;

                                // Combine indices, adjusting after-indices by the offset
                                // (choice_char_idx + 1 to account for the delimiter itself)
                                let offset = choice_char_idx + 1;
                                let mut combined_indices = indices_before;
                                combined_indices.extend(indices_after.iter().map(|&i| i + offset));

                                Some((total_score, combined_indices))
                            }
                            _ => None,
                        }
                    }
                    None => {
                        // No delimiter in choice but delimiter in pattern
                        // This cannot match since we expect both parts to be present
                        None
                    }
                }
            }
        }
    }
}

impl MatchEngine for FuzzyEngine {
    fn match_item(&self, item: Arc<dyn SkimItem>) -> Option<MatchResult> {
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

        trace!("matched range {matched_range:?}");
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
