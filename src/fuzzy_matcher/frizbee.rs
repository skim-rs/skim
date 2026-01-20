//! Matcher using https://crates.io/crates/frizbee
use frizbee::{Config, Scoring, match_indices};

use crate::fuzzy_matcher::{FuzzyMatcher, IndexType, ScoreType};

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
                prefilter: true,
                max_typos: Some(2),
                sort: false,
                scoring: Scoring::default(),
            },
        }
    }
}

impl FuzzyMatcher for FrizbeeMatcher {
    fn fuzzy_indices(&self, choice: &str, pattern: &str) -> Option<(ScoreType, Vec<IndexType>)> {
        let res = match_indices(pattern, choice, &self.config);
        res.map(|indices| (indices.score.into(), indices.indices.to_vec()))
    }
}
