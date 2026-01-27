//! Matcher using https://crates.io/crates/frizbee
use frizbee::{Config, Scoring, match_indices};

use crate::fuzzy_matcher::{FuzzyMatcher, IndexType, ScoreType};

/// Matcher using frizbee,
/// the same one that `blink.cmp` uses in neovim
/// credits to @saghen
#[derive(Default)]
pub struct FrizbeeMatcher {}

fn adaptive(pattern: &str) -> Config {
    Config {
        max_typos: Some(pattern.chars().count().saturating_div(4).try_into().unwrap()),
        prefilter: true,
        sort: false,
        scoring: Scoring::default(),
    }
}

impl FuzzyMatcher for FrizbeeMatcher {
    fn fuzzy_indices(&self, choice: &str, pattern: &str) -> Option<(ScoreType, Vec<IndexType>)> {
        let matches = match_indices(pattern, choice, &adaptive(pattern))?;

        let mut indices = Vec::new();
        for matched_idx in matches.indices {
            for (char_idx, (byte_idx, _)) in choice.char_indices().enumerate() {
                if byte_idx == matched_idx {
                    indices.push(char_idx);
                    break;
                }
            }
        }

        Some((matches.score.into(), indices))
    }
}
