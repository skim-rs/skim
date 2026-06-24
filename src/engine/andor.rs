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

    fn merge_matched_items(items: Vec<MatchResult>, text: &str) -> MatchResult {
        let mut ranges = MatchIndices::new();
        let mut rank = crate::Rank {
            score: 0,
            begin: i32::MAX,
            end: i32::MIN,
            ..items[0].rank
        };
        for item in items {
            match item.matched_range {
                MatchRange::ByteRange(..) => {
                    ranges.extend(item.range_char_indices(text));
                }
                MatchRange::CharRange(start, end) => {
                    ranges.extend(start..end);
                }
                MatchRange::Chars(vec) => {
                    ranges.extend(vec.iter().copied());
                }
            }
            rank.score = rank.score.saturating_add(item.rank.score);
            rank.begin = rank.begin.min(item.rank.begin);
            rank.end = rank.end.max(item.rank.end);
        }

        ranges.sort_unstable();
        ranges.dedup();
        MatchResult {
            rank,
            matched_range: MatchRange::Chars(ranges),
        }
    }
}

impl MatchEngine for AndEngine {
    fn match_item(&self, item: &dyn SkimItem) -> Option<MatchResult> {
        // Fast path: single sub-engine — skip merge entirely.
        if self.engines.len() == 1 {
            return self.engines[0].match_item(item);
        }

        let mut results = vec![];
        for engine in &self.engines {
            let result = engine.match_item(item)?;
            results.push(result);
        }

        if results.is_empty() {
            None
        } else {
            Some(Self::merge_matched_items(results, &item.text()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::exact::{ExactEngine, ExactMatchingParam};

    fn exact(query: &str) -> Box<dyn MatchEngine> {
        Box::new(ExactEngine::builder(query, ExactMatchingParam::default()).build())
    }

    #[test]
    fn or_engine_matches_if_any_subengine_matches() {
        let engine = OrEngine::builder().engines(vec![exact("foo"), exact("zzz")]).build();
        assert!(engine.match_item(&"a foo bar".to_string()).is_some());
    }

    #[test]
    fn or_engine_returns_none_when_no_subengine_matches() {
        let engine = OrEngine::builder().engines(vec![exact("xxx"), exact("zzz")]).build();
        assert!(engine.match_item(&"a foo bar".to_string()).is_none());
    }

    #[test]
    fn or_engine_empty_returns_none() {
        let engine = OrEngine::builder().build();
        assert!(engine.match_item(&"anything".to_string()).is_none());
    }

    #[test]
    fn and_engine_single_engine_fast_path() {
        let engine = AndEngine::builder().engines(vec![exact("foo")]).build();
        assert!(engine.match_item(&"foobar".to_string()).is_some());
        assert!(engine.match_item(&"nope".to_string()).is_none());
    }

    #[test]
    fn and_engine_requires_all_subengines_to_match() {
        let engine = AndEngine::builder().engines(vec![exact("foo"), exact("bar")]).build();
        // Both substrings present -> matched, ranges merged.
        let result = engine.match_item(&"foo and bar".to_string());
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(matches!(result.matched_range, MatchRange::Chars(_)));

        // Missing one substring -> no match.
        assert!(engine.match_item(&"foo only".to_string()).is_none());
    }

    #[test]
    fn and_engine_empty_returns_none() {
        // With no sub-engines the multi-engine path collects nothing and bails.
        let engine = AndEngine::builder().build();
        assert!(engine.match_item(&"anything".to_string()).is_none());
    }

    #[test]
    fn display_formats_combinators() {
        let or = OrEngine::builder().engines(vec![exact("a")]).build();
        assert!(format!("{or}").starts_with("(Or:"));
        let and = AndEngine::builder().engines(vec![exact("a")]).build();
        assert!(format!("{and}").starts_with("(And:"));
    }
}
