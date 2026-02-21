use std::fmt::{Display, Error, Formatter};
use std::sync::Arc;

use crate::fuzzy_matcher::MatchIndices;
use crate::{MatchEngine, MatchRange, MatchResult, SkimItem};

//------------------------------------------------------------------------------
// OrEngine, a combinator
pub struct OrEngine {
    engines: Vec<Box<dyn MatchEngine>>,
}

impl OrEngine {
    pub fn builder() -> Self {
        Self { engines: vec![] }
    }

    pub fn engines(mut self, mut engines: Vec<Box<dyn MatchEngine>>) -> Self {
        self.engines.append(&mut engines);
        self
    }

    pub fn build(self) -> Self {
        self
    }
}

impl MatchEngine for OrEngine {
    fn match_item(&self, item: &dyn SkimItem) -> Option<MatchResult> {
        for engine in &self.engines {
            let result = engine.match_item(item);
            if result.is_some() {
                return result;
            }
        }

        None
    }

    fn match_items(&self, items: &[Arc<dyn SkimItem>]) -> Vec<Option<MatchResult>> {
        if self.engines.len() == 1 {
            self.engines[0].match_items(items)
        } else {
            items.iter().map(|item| self.match_item(item.as_ref())).collect()
        }
    }
}

impl Display for OrEngine {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(
            f,
            "(Or: {})",
            self.engines
                .iter()
                .map(|e| format!("{e}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

//------------------------------------------------------------------------------
// AndEngine, a combinator
pub struct AndEngine {
    engines: Vec<Box<dyn MatchEngine>>,
}

impl AndEngine {
    pub fn builder() -> Self {
        Self { engines: vec![] }
    }

    pub fn engines(mut self, mut engines: Vec<Box<dyn MatchEngine>>) -> Self {
        self.engines.append(&mut engines);
        self
    }

    pub fn build(self) -> Self {
        self
    }

    fn merge_matched_items(&self, items: Vec<MatchResult>, text: &str) -> MatchResult {
        let rank = items[0].rank;
        let mut ranges = MatchIndices::new();
        for item in items {
            match item.matched_range {
                MatchRange::ByteRange(..) => {
                    ranges.extend(item.range_char_indices(text));
                }
                MatchRange::Chars(vec) => {
                    ranges.extend(vec.iter().copied());
                }
            }
        }

        ranges.sort();
        ranges.dedup();
        MatchResult {
            rank,
            matched_range: MatchRange::Chars(ranges),
        }
    }
}

impl MatchEngine for AndEngine {
    fn match_item(&self, item: &dyn SkimItem) -> Option<MatchResult> {
        // mock
        let mut results = vec![];
        for engine in &self.engines {
            let result = engine.match_item(item)?;
            results.push(result);
        }

        if results.is_empty() {
            None
        } else {
            Some(self.merge_matched_items(results, &item.text()))
        }
    }

    fn match_items(&self, items: &[Arc<dyn SkimItem>]) -> Vec<Option<MatchResult>> {
        if self.engines.len() == 1 {
            // Single engine — delegate directly for batch optimization
            self.engines[0].match_items(items)
        } else {
            // Multiple engines — fall back to per-item
            items.iter().map(|item| self.match_item(item.as_ref())).collect()
        }
    }
}

impl Display for AndEngine {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(
            f,
            "(And: {})",
            self.engines
                .iter()
                .map(|e| format!("{e}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}
