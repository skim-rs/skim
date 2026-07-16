use std::fmt::{Display, Error, Formatter};
use std::sync::Arc;

use regex::Regex;

use crate::engine::util::regex_match;
use crate::item::RankBuilder;
use crate::{CaseMatching, MatchEngine, MatchRange, MatchResult, SkimItem};
use std::cmp::min;

//------------------------------------------------------------------------------
// Regular Expression engine
#[derive(Debug)]
pub struct RegexEngine {
    query_regex: Option<Regex>,
    rank_builder: Arc<RankBuilder>,
}

impl RegexEngine {
    pub fn builder(query: &str, case: CaseMatching) -> Self {
        let mut query_builder = String::new();

        match case {
            CaseMatching::Ignore => query_builder.push_str("(?i)"),
            CaseMatching::Respect | CaseMatching::Smart => {}
        }

        query_builder.push_str(query);

        RegexEngine {
            query_regex: Regex::new(&query_builder).ok(),
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

impl MatchEngine for RegexEngine {
    fn match_item(&self, item: &dyn SkimItem) -> Option<MatchResult> {
        let mut matched_result = None;
        let item_text = item.text();
        let default_range = [(0, item_text.len())];
        for &(start, end) in item.get_matching_ranges().unwrap_or(&default_range) {
            let start = min(start, item_text.len());
            let end = min(end, item_text.len());
            if self.query_regex.is_none() {
                matched_result = Some((0, 0));
                break;
            }

            matched_result =
                regex_match(&item_text[start..end], self.query_regex.as_ref()).map(|(s, e)| (s + start, e + start));

            if matched_result.is_some() {
                break;
            }
        }

        let (begin, end) = matched_result?;
        let score = i32::try_from(end - begin).unwrap_or(i32::MAX);

        Some(MatchResult {
            rank: self.rank_builder.build_rank(score, begin, end, &item_text),
            matched_range: MatchRange::ByteRange(begin, end),
        })
    }
}

impl Display for RegexEngine {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(
            f,
            "(Regex: {})",
            self.query_regex
                .as_ref()
                .map_or(String::new(), |re| re.as_str().to_string())
        )
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    use super::*;

    fn engine(query: &str, case: CaseMatching) -> RegexEngine {
        RegexEngine::builder(query, case).build()
    }

    #[test]
    fn matches_regex_pattern() {
        let e = engine("ba.", CaseMatching::Respect);
        let result = e.match_item(&"foobar".to_string()).unwrap();
        assert_eq!(result.matched_range, MatchRange::ByteRange(3, 6));
    }

    #[test]
    fn no_match_returns_none() {
        let e = engine("xyz", CaseMatching::Respect);
        assert!(e.match_item(&"foobar".to_string()).is_none());
    }

    #[test]
    fn ignore_case_matches_insensitively() {
        let e = engine("foo", CaseMatching::Ignore);
        assert!(e.match_item(&"FOOBAR".to_string()).is_some());
    }

    #[test]
    fn smart_case_is_sensitive() {
        let e = engine("foo", CaseMatching::Smart);
        assert!(e.match_item(&"foobar".to_string()).is_some());
        assert!(e.match_item(&"FOOBAR".to_string()).is_none());
    }

    #[test]
    fn empty_query_matches_everything() {
        // An empty pattern produces a regex that matches at position 0.
        let e = engine("", CaseMatching::Respect);
        assert!(e.match_item(&"anything".to_string()).is_some());
    }

    #[test]
    fn invalid_regex_yields_no_regex_and_matches_all() {
        // An unparsable pattern leaves `query_regex` as None, which short-circuits
        // to a zero-length match for every item.
        let e = engine("(", CaseMatching::Respect);
        let result = e.match_item(&"abc".to_string()).unwrap();
        assert_eq!(result.matched_range, MatchRange::ByteRange(0, 0));
    }

    #[test]
    fn display_shows_pattern() {
        let e = engine("ba.", CaseMatching::Respect);
        assert_eq!(format!("{e}"), "(Regex: ba.)");
    }
}
