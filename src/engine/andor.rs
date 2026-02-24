use std::fmt::{Display, Error, Formatter};

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
        let result = self
            .engines
            .iter()
            .map(|e| e.match_item(item))
            .max_by_key(|res| res.as_ref().map(|matched| matched.rank.score));

        result?
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
        let mut ranges = MatchIndices::new();
        let mut rank = items[0].rank;
        for item in items {
            match item.matched_range {
                MatchRange::ByteRange(..) => {
                    ranges.extend(item.range_char_indices(text));
                }
                MatchRange::Chars(vec) => {
                    ranges.extend(vec.iter().copied());
                }
            }
            rank.score = rank.score.max(item.rank.score);
            rank.begin = rank.begin.min(item.rank.begin);
            rank.end = rank.end.max(item.rank.end);
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
