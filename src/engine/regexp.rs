use std::borrow::Cow;
use std::cmp::min;
use std::fmt::{Display, Error, Formatter};
use std::sync::Arc;

use regex::Regex;

use crate::engine::util::{map_byte_range_to_original, normalize_with_byte_mapping, regex_match};
use crate::item::RankBuilder;
use crate::{CaseMatching, MatchEngine};
use crate::{MatchRange, MatchResult, SkimItem};

//------------------------------------------------------------------------------
// Regular Expression engine
#[derive(Debug)]
pub struct RegexEngine {
    query_regex: Option<Regex>,
    rank_builder: Arc<RankBuilder>,
    normalize: bool,
}

impl RegexEngine {
    pub fn builder(query: &str, case: CaseMatching, normalize: bool) -> Self {
        let mut query_builder = String::new();

        match case {
            CaseMatching::Respect => {}
            CaseMatching::Ignore => query_builder.push_str("(?i)"),
            CaseMatching::Smart => {}
        }

        // Normalize query if requested
        let query_for_regex = if normalize {
            let (normalized, _) = normalize_with_byte_mapping(query);
            normalized
        } else {
            query.to_string()
        };

        query_builder.push_str(&query_for_regex);

        RegexEngine {
            query_regex: Regex::new(&query_builder).ok(),
            rank_builder: Default::default(),
            normalize,
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
    fn match_item(&self, item: Arc<dyn SkimItem>) -> Option<MatchResult> {
        let mut matched_result = None;
        let item_text = item.text();

        // Get normalized text and byte mapping if normalization is enabled
        let (item_text_for_match, byte_mapping): (Cow<str>, Option<Vec<usize>>) = if self.normalize {
            let (normalized, mapping) = normalize_with_byte_mapping(&item_text);
            (Cow::Owned(normalized), Some(mapping))
        } else {
            (Cow::Borrowed(&*item_text), None)
        };

        let default_range = [(0, item_text_for_match.len())];
        for &(start, end) in item.get_matching_ranges().unwrap_or(&default_range) {
            let start = min(start, item_text_for_match.len());
            let end = min(end, item_text_for_match.len());
            if self.query_regex.is_none() {
                matched_result = Some((0, 0));
                break;
            }

            matched_result =
                regex_match(&item_text_for_match[start..end], &self.query_regex).map(|(s, e)| (s + start, e + start));

            if matched_result.is_some() {
                break;
            }
        }

        let (begin, end) = matched_result?;

        // Map byte range back to original string if we normalized
        let (begin, end) = if let Some(ref mapping) = byte_mapping {
            map_byte_range_to_original(begin, end, mapping, &item_text)
        } else {
            (begin, end)
        };

        let score = (end - begin) as i32;
        let item_len = item_text.len();

        Some(MatchResult {
            rank: self
                .rank_builder
                .build_rank(score, begin, end, item_len, item.get_index()),
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
                .map_or("".to_string(), |re| re.as_str().to_string())
        )
    }
}
