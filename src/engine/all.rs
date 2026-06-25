use std::fmt::{Display, Error, Formatter};
use std::sync::Arc;

use crate::item::RankBuilder;
use crate::{MatchEngine, MatchRange, MatchResult, SkimItem};

//------------------------------------------------------------------------------
#[derive(Debug)]
pub struct MatchAllEngine {
    rank_builder: Arc<RankBuilder>,
}

impl MatchAllEngine {
    pub fn builder() -> Self {
        Self {
            rank_builder: Default::default(),
        }
    }

    pub fn rank_builder(mut self, rank_builder: Arc<RankBuilder>) -> Self {
        self.rank_builder = rank_builder;
        self
    }

    pub fn build(self) -> Self {
        self
    }
}

impl MatchEngine for MatchAllEngine {
    fn match_item(&self, item: &dyn SkimItem) -> Option<MatchResult> {
        let item_text = item.text();
        Some(MatchResult {
            rank: self.rank_builder.build_rank(0, 0, 0, &item_text),
            matched_range: MatchRange::ByteRange(0, 0),
        })
    }
}

impl Display for MatchAllEngine {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "Noop")
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn matches_every_item_with_empty_range() {
        let engine = MatchAllEngine::builder().build();
        let result = engine.match_item(&"anything".to_string()).unwrap();
        assert_eq!(result.matched_range, MatchRange::ByteRange(0, 0));
    }

    #[test]
    fn rank_builder_override_is_used() {
        let engine = MatchAllEngine::builder()
            .rank_builder(Arc::new(RankBuilder::default()))
            .build();
        assert!(engine.match_item(&"x".to_string()).is_some());
    }

    #[test]
    fn display_is_noop() {
        let engine = MatchAllEngine::builder().build();
        assert_eq!(format!("{engine}"), "Noop");
    }
}
