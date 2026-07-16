//! Field extraction and parsing utilities.
//!
//! This module provides utilities for parsing field ranges and extracting
//! fields from text based on delimiters.

use regex::Regex;
use std::cmp::{max, min};
use std::sync::LazyLock;

static FIELD_RANGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<left>-?\d+)?(?P<sep>\.\.)?(?P<right>-?\d+)?$").unwrap());

/// Represents a range of fields to extract from text
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum FieldRange {
    /// A single field at the given index
    Single(i32),
    /// All fields from the start up to and including the given index
    LeftInf(i32),
    /// All fields from the given index to the end
    RightInf(i32),
    /// Fields between two indices (inclusive)
    Both(i32, i32),
}

impl FieldRange {
    /// Parses a field range from a string (e.g., "1", "1..", "..10", "1..10")
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(range: &str) -> Option<FieldRange> {
        use self::FieldRange::{Both, LeftInf, RightInf, Single};

        // "1", "1..", "..10", "1..10", etc.
        let opt_caps = FIELD_RANGE.captures(range);
        if let Some(caps) = opt_caps {
            let opt_left = caps.name("left").map(|s| s.as_str().parse().unwrap_or(1));
            let opt_right = caps.name("right").map(|s| s.as_str().parse().unwrap_or(-1));
            let opt_sep = caps.name("sep").map(|s| s.as_str().to_string());

            match (opt_left, opt_right) {
                (None, None) => Some(RightInf(0)),
                (Some(left), None) => {
                    match opt_sep {
                        None => Some(Single(left)),      // 1
                        Some(_) => Some(RightInf(left)), // 1..
                    }
                }
                (None, Some(right)) => {
                    match opt_sep {
                        None => Some(Single(right)),     // 1 (should not happen)
                        Some(_) => Some(LeftInf(right)), // ..1 (should not happen)
                    }
                }
                (Some(left), Some(right)) => Some(Both(left, right)), // 1..3
            }
        } else {
            None
        }
    }

    /// Converts a field range to an index pair (left, right).
    ///
    /// For example, 1..3 => (0, 4). Note that field range is inclusive while
    /// the output index will exclude the right end.
    #[must_use]
    pub fn to_index_pair(&self, length: usize) -> Option<(usize, usize)> {
        use self::FieldRange::{Both, LeftInf, RightInf, Single};
        match *self {
            Single(num) => {
                let num = FieldRange::translate_neg(num, length);
                if num == 0 || num > length {
                    None
                } else {
                    Some((num - 1, num))
                }
            }
            LeftInf(right) => {
                let right = FieldRange::translate_neg(right, length);
                if length == 0 || right == 0 {
                    None
                } else {
                    let right = min(right, length);
                    Some((0, right))
                }
            }
            RightInf(left) => {
                let left = FieldRange::translate_neg(left, length);
                if length == 0 || left > length {
                    None
                } else {
                    let left = max(left, 1);
                    Some((left - 1, length))
                }
            }
            Both(left, right) => {
                let left = FieldRange::translate_neg(left, length);
                let right = FieldRange::translate_neg(right, length);
                if length == 0 || right == 0 || left > right || left > length {
                    None
                } else {
                    Some((max(left, 1) - 1, min(right, length)))
                }
            }
        }
    }

    fn translate_neg(idx: i32, length: usize) -> usize {
        let len = i32::try_from(length).unwrap_or(i32::MAX);
        let idx = if idx < 0 { idx + len + 1 } else { idx };
        max(0, idx).unsigned_abs() as usize
    }
}

// ("|", "a|b||c") -> [(0, 2), (2, 4), (4, 5), (5, 6)]
// explain: split to ["a|", "b|", "|", "c"]
fn get_ranges_by_delimiter(delimiter: &Regex, text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut last = 0;
    for mat in delimiter.find_iter(text) {
        ranges.push((last, mat.start()));
        last = mat.end();
    }
    ranges.push((last, text.len()));
    ranges
}

/// Extracts a substring from text based on a field range and delimiter.
///
/// For example, with delimiter = `Regex::new(",").unwrap()`, text "a,b,c", and field Single(2),
/// this returns "b". Note that this is different from `to_index_pair`, it uses delimiters.
#[must_use]
pub fn get_string_by_field<'a>(delimiter: &Regex, text: &'a str, field: &FieldRange) -> Option<&'a str> {
    let ranges = get_ranges_by_delimiter(delimiter, text);

    if let Some((start, stop)) = field.to_index_pair(ranges.len()) {
        let &(begin, _) = &ranges[start];
        let &(_, end) = ranges.get(stop - 1).unwrap_or(&(text.len(), 0));
        Some(&text[begin..end])
    } else {
        None
    }
}

/// Extracts a substring from text by parsing a range string and using a delimiter
#[must_use]
pub fn get_string_by_range<'a>(delimiter: &Regex, text: &'a str, range: &str) -> Option<&'a str> {
    FieldRange::from_str(range).and_then(|field| get_string_by_field(delimiter, text, &field))
}

/// Parses matching fields and returns a vector of byte ranges.
///
/// Given delimiter `,`, text: "a,b,c", and fields &[Single(2), LeftInf(2)],
/// this returns [(2, 4), (0, 4)].
#[must_use]
pub fn parse_matching_fields(delimiter: &Regex, text: &str, fields: &[FieldRange]) -> Vec<(usize, usize)> {
    let ranges = get_ranges_by_delimiter(delimiter, text);

    let mut ret = Vec::new();
    for field in fields {
        if let Some((start, stop)) = field.to_index_pair(ranges.len()) {
            let &(begin, _) = &ranges[start];
            let &(end, _) = ranges.get(stop).unwrap_or(&(text.len(), 0));
            ret.push((begin, end));
        }
    }
    ret
}

/// Extracts the specified fields from text using the delimiter
#[must_use]
pub fn parse_transform_fields(delimiter: &Regex, text: &str, fields: &[FieldRange]) -> String {
    let ranges = get_ranges_by_delimiter(delimiter, text);

    let mut ret = String::new();
    for field in fields {
        if let Some((start, stop)) = field.to_index_pair(ranges.len()) {
            let &(begin, _) = &ranges[start];
            let &(end, _) = ranges.get(stop).unwrap_or(&(text.len(), 0));
            ret.push_str(&text[begin..end]);
        }
    }
    ret
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod test;
