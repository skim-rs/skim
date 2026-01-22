use crate::engine::util::{contains_upper, map_byte_range_to_original, normalize_with_byte_mapping, regex_match};
use crate::item::RankBuilder;
use crate::{CaseMatching, MatchEngine, MatchRange, MatchResult, SkimItem};
use regex::{Regex, escape};
use std::borrow::Cow;
use std::cmp::min;
use std::fmt::{Display, Error, Formatter};
use std::sync::Arc;

//------------------------------------------------------------------------------
// Exact engine
#[derive(Debug, Copy, Clone, Default)]
pub struct ExactMatchingParam {
    pub prefix: bool,
    pub postfix: bool,
    pub inverse: bool,
    pub case: CaseMatching,
    pub normalize: bool,
    __non_exhaustive: bool,
}

#[derive(Debug)]
pub struct ExactEngine {
    #[allow(dead_code)]
    query: String,
    query_regex: Option<Regex>,
    rank_builder: Arc<RankBuilder>,
    inverse: bool,
    normalize: bool,
}

impl ExactEngine {
    pub fn builder(query: &str, param: ExactMatchingParam) -> Self {
        let case_sensitive = match param.case {
            CaseMatching::Respect => true,
            CaseMatching::Ignore => false,
            CaseMatching::Smart => contains_upper(query),
        };

        // Normalize query if requested
        let query_for_regex = if param.normalize {
            let (normalized, _) = normalize_with_byte_mapping(query);
            normalized
        } else {
            query.to_string()
        };

        let mut query_builder = String::new();
        if !case_sensitive {
            query_builder.push_str("(?i)");
        }

        if param.prefix {
            query_builder.push('^');
        }

        query_builder.push_str(&escape(&query_for_regex));

        if param.postfix {
            query_builder.push('$');
        }

        let query_regex = if query.is_empty() {
            None
        } else {
            Regex::new(&query_builder).ok()
        };

        ExactEngine {
            query: query.to_string(),
            query_regex,
            rank_builder: Default::default(),
            inverse: param.inverse,
            normalize: param.normalize,
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

impl MatchEngine for ExactEngine {
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

            if self.inverse {
                matched_result = matched_result.xor(Some((0, 0)))
            }

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

impl Display for ExactEngine {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(
            f,
            "(Exact|{}{})",
            if self.inverse { "!" } else { "" },
            self.query_regex.as_ref().map(|x| x.as_str()).unwrap_or("")
        )
    }
}
