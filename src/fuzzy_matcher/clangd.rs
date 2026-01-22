//! The fuzzy matching algorithm used in clangd.
//! https://github.com/llvm-mirror/clang-tools-extra/blob/master/clangd/FuzzyMatch.cpp
//!
//! # Example:
//! ```edition2018
//! use crate::fuzzy_matcher::FuzzyMatcher;
//! use crate::fuzzy_matcher::clangd::ClangdMatcher;
//!
//! let matcher = ClangdMatcher::default();
//!
//! assert_eq!(None, matcher.fuzzy_match("abc", "abx"));
//! assert!(matcher.fuzzy_match("axbycz", "abc").is_some());
//! assert!(matcher.fuzzy_match("axbycz", "xyz").is_some());
//!
//! let (score, indices) = matcher.fuzzy_indices("axbycz", "abc").unwrap();
//! assert_eq!(indices, [0, 2, 4]);
//!
//! ```
//!
//! Algorithm modified from
//! https://github.com/llvm-mirror/clang-tools-extra/blob/master/clangd/FuzzyMatch.cpp
//! Also check: https://github.com/lewang/flx/issues/98
use crate::fuzzy_matcher::util::*;
use crate::fuzzy_matcher::{FuzzyMatcher, IndexType, ScoreType};
use std::cell::RefCell;
use std::cmp::max;
use thread_local::ThreadLocal;

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
enum CaseMatching {
    Respect,
    Ignore,
    Smart,
}

#[derive(Debug)]
/// Fuzzy matcher using the clangd algorithm
pub struct ClangdMatcher {
    case: CaseMatching,

    use_cache: bool,

    c_cache: ThreadLocal<RefCell<Vec<char>>>, // vector to store the characters of choice
    p_cache: ThreadLocal<RefCell<Vec<char>>>, // vector to store the characters of pattern
}

impl Default for ClangdMatcher {
    fn default() -> Self {
        Self {
            case: CaseMatching::Ignore,
            use_cache: true,
            c_cache: ThreadLocal::new(),
            p_cache: ThreadLocal::new(),
        }
    }
}

impl ClangdMatcher {
    /// Sets the matcher to ignore case when matching
    pub fn ignore_case(mut self) -> Self {
        self.case = CaseMatching::Ignore;
        self
    }

    /// Sets the matcher to use smart case (case insensitive unless pattern contains uppercase)
    pub fn smart_case(mut self) -> Self {
        self.case = CaseMatching::Smart;
        self
    }

    /// Sets the matcher to respect case when matching
    pub fn respect_case(mut self) -> Self {
        self.case = CaseMatching::Respect;
        self
    }

    /// Enables or disables caching for improved performance
    pub fn use_cache(mut self, use_cache: bool) -> Self {
        self.use_cache = use_cache;
        self
    }

    fn contains_upper(&self, string: &str) -> bool {
        for ch in string.chars() {
            if ch.is_uppercase() {
                return true;
            }
        }

        false
    }

    fn is_case_sensitive(&self, pattern: &str) -> bool {
        match self.case {
            CaseMatching::Respect => true,
            CaseMatching::Ignore => false,
            CaseMatching::Smart => self.contains_upper(pattern),
        }
    }
}

impl FuzzyMatcher for ClangdMatcher {
    fn fuzzy_indices(&self, choice: &str, pattern: &str) -> Option<(ScoreType, Vec<IndexType>)> {
        let case_sensitive = self.is_case_sensitive(pattern);

        let mut choice_chars = self.c_cache.get_or(|| RefCell::new(Vec::new())).borrow_mut();
        let mut pattern_chars = self.p_cache.get_or(|| RefCell::new(Vec::new())).borrow_mut();

        choice_chars.clear();
        for char in choice.chars() {
            choice_chars.push(char);
        }

        pattern_chars.clear();
        for char in pattern.chars() {
            pattern_chars.push(char);
        }

        cheap_matches(&choice_chars, &pattern_chars, case_sensitive)?;

        let num_pattern_chars = pattern_chars.len();
        let num_choice_chars = choice_chars.len();

        let dp = build_graph(&choice_chars, &pattern_chars, false, case_sensitive);

        // search backwards for the matched indices
        let mut indices_reverse = Vec::with_capacity(num_pattern_chars);
        let cell = dp[num_pattern_chars][num_choice_chars];

        let (mut last_action, score) = if cell.match_score > cell.miss_score {
            (Action::Match, cell.match_score)
        } else {
            (Action::Miss, cell.miss_score)
        };

        let mut row = num_pattern_chars;
        let mut col = num_choice_chars;

        while row > 0 || col > 0 {
            if last_action == Action::Match {
                indices_reverse.push((col - 1) as IndexType);
            }

            let cell = &dp[row][col];
            if last_action == Action::Match {
                last_action = cell.last_action_match;
                row -= 1;
                col -= 1;
            } else {
                last_action = cell.last_action_miss;
                col -= 1;
            }
        }

        if !self.use_cache {
            // drop the allocated memory
            self.c_cache.get().map(|cell| cell.replace(vec![]));
            self.p_cache.get().map(|cell| cell.replace(vec![]));
        }

        indices_reverse.reverse();
        Some((adjust_score(score, num_choice_chars), indices_reverse))
    }

    fn fuzzy_match(&self, choice: &str, pattern: &str) -> Option<ScoreType> {
        let case_sensitive = self.is_case_sensitive(pattern);

        let mut choice_chars = self.c_cache.get_or(|| RefCell::new(Vec::new())).borrow_mut();
        let mut pattern_chars = self.p_cache.get_or(|| RefCell::new(Vec::new())).borrow_mut();

        choice_chars.clear();
        for char in choice.chars() {
            choice_chars.push(char);
        }

        pattern_chars.clear();
        for char in pattern.chars() {
            pattern_chars.push(char);
        }

        cheap_matches(&choice_chars, &pattern_chars, case_sensitive)?;

        let num_pattern_chars = pattern_chars.len();
        let num_choice_chars = choice_chars.len();

        let dp = build_graph(&choice_chars, &pattern_chars, true, case_sensitive);

        let cell = dp[num_pattern_chars & 1][num_choice_chars];
        let score = max(cell.match_score, cell.miss_score);

        if !self.use_cache {
            // drop the allocated memory
            self.c_cache.get().map(|cell| cell.replace(vec![]));
            self.p_cache.get().map(|cell| cell.replace(vec![]));
        }

        Some(adjust_score(score, num_choice_chars))
    }
}

/// fuzzy match `line` with `pattern`, returning the score and indices of matches
pub fn fuzzy_indices(line: &str, pattern: &str) -> Option<(ScoreType, Vec<IndexType>)> {
    ClangdMatcher::default().ignore_case().fuzzy_indices(line, pattern)
}

/// fuzzy match `line` with `pattern`, returning the score(the larger the better) on match
pub fn fuzzy_match(line: &str, pattern: &str) -> Option<ScoreType> {
    ClangdMatcher::default().ignore_case().fuzzy_match(line, pattern)
}

// checkout https://github.com/llvm-mirror/clang-tools-extra/blob/master/clangd/FuzzyMatch.cpp
// for the description
fn build_graph(line: &[char], pattern: &[char], compressed: bool, case_sensitive: bool) -> Vec<Vec<Score>> {
    let num_line_chars = line.len();
    let num_pattern_chars = pattern.len();
    let max_rows = if compressed { 2 } else { num_pattern_chars + 1 };

    let mut dp: Vec<Vec<Score>> = Vec::with_capacity(max_rows);

    for _ in 0..max_rows {
        dp.push(vec![Score::default(); num_line_chars + 1]);
    }

    dp[0][0].miss_score = 0;

    // first line
    for (idx, &ch) in line.iter().enumerate() {
        dp[0][idx + 1] = Score {
            miss_score: dp[0][idx].miss_score - skip_penalty(idx, ch, Action::Miss),
            last_action_miss: Action::Miss,
            match_score: AWFUL_SCORE,
            last_action_match: Action::Miss,
        };
    }

    // build the matrix
    let mut pat_prev_ch = '\0';
    for (pat_idx, &pat_ch) in pattern.iter().enumerate() {
        let current_row_idx = if compressed { (pat_idx + 1) & 1 } else { pat_idx + 1 };
        let prev_row_idx = if compressed { pat_idx & 1 } else { pat_idx };

        let mut line_prev_ch = '\0';
        for (line_idx, &line_ch) in line.iter().enumerate() {
            if line_idx < pat_idx {
                line_prev_ch = line_ch;
                continue;
            }

            // what if we skip current line character?
            // we need to calculate the cases where the pre line character is matched/missed
            let pre_miss = &dp[current_row_idx][line_idx];
            let mut match_miss_score = pre_miss.match_score;
            let mut miss_miss_score = pre_miss.miss_score;
            if pat_idx < num_pattern_chars - 1 {
                match_miss_score -= skip_penalty(line_idx, line_ch, Action::Match);
                miss_miss_score -= skip_penalty(line_idx, line_ch, Action::Miss);
            }

            let (miss_score, last_action_miss) = if match_miss_score > miss_miss_score {
                (match_miss_score, Action::Match)
            } else {
                (miss_miss_score, Action::Miss)
            };

            // what if we want to match current line character?
            // so we need to calculate the cases where the pre pattern character is matched/missed
            let pre_match = &dp[prev_row_idx][line_idx];
            let match_match_score = if allow_match(pat_ch, line_ch, case_sensitive) {
                pre_match.match_score
                    + match_bonus(
                        pat_idx,
                        pat_ch,
                        pat_prev_ch,
                        line_idx,
                        line_ch,
                        line_prev_ch,
                        Action::Match,
                    )
            } else {
                AWFUL_SCORE
            };

            let miss_match_score = if allow_match(pat_ch, line_ch, case_sensitive) {
                pre_match.miss_score
                    + match_bonus(
                        pat_idx,
                        pat_ch,
                        pat_prev_ch,
                        line_idx,
                        line_ch,
                        line_prev_ch,
                        Action::Match,
                    )
            } else {
                AWFUL_SCORE
            };

            let (match_score, last_action_match) = if match_match_score > miss_match_score {
                (match_match_score, Action::Match)
            } else {
                (miss_match_score, Action::Miss)
            };

            dp[current_row_idx][line_idx + 1] = Score {
                miss_score,
                last_action_miss,
                match_score,
                last_action_match,
            };

            line_prev_ch = line_ch;
        }

        pat_prev_ch = pat_ch;
    }

    dp
}

fn adjust_score(score: ScoreType, num_line_chars: usize) -> ScoreType {
    // line width will affect 10 scores
    score - (((num_line_chars + 1) as f64).ln().floor() as ScoreType)
}

const AWFUL_SCORE: ScoreType = -(1 << 30);

#[derive(Debug, PartialEq, Clone, Copy)]
enum Action {
    Miss,
    Match,
}

#[derive(Debug, Clone, Copy)]
struct Score {
    pub last_action_miss: Action,
    pub last_action_match: Action,
    pub miss_score: ScoreType,
    pub match_score: ScoreType,
}

impl Default for Score {
    fn default() -> Self {
        Self {
            last_action_miss: Action::Miss,
            last_action_match: Action::Miss,
            miss_score: AWFUL_SCORE,
            match_score: AWFUL_SCORE,
        }
    }
}

fn skip_penalty(_ch_idx: usize, ch: char, last_action: Action) -> ScoreType {
    let mut score = 1;
    if last_action == Action::Match {
        // Non-consecutive match.
        score += 3;
    }

    if char_type_of(ch) == CharType::NonWord {
        // skip separator
        score += 6;
    }

    score
}

fn allow_match(pat_ch: char, line_ch: char, case_sensitive: bool) -> bool {
    char_equal(pat_ch, line_ch, case_sensitive)
}

fn match_bonus(
    pat_idx: usize,
    pat_ch: char,
    pat_prev_ch: char,
    line_idx: usize,
    line_ch: char,
    line_prev_ch: char,
    last_action: Action,
) -> ScoreType {
    let mut score = 10;
    let pat_role = char_role(pat_prev_ch, pat_ch);
    let line_role = char_role(line_prev_ch, line_ch);

    // Bonus: pattern so far is a (case-insensitive) prefix of the word.
    if pat_idx == line_idx {
        score += 10;
    }

    // Bonus: case match
    if pat_ch == line_ch {
        score += 8;
    }

    // Bonus: match header
    if line_role == CharRole::Head {
        score += 9;
    }

    // Bonus: a Head in the pattern aligns with one in the word.
    if pat_role == CharRole::Head && line_role == CharRole::Head {
        score += 10;
    }

    // Penalty: matching inside a segment (and previous char wasn't matched).
    if line_role == CharRole::Tail && pat_idx > 0 && last_action == Action::Miss {
        score -= 30;
    }

    // Penalty: a Head in the pattern matches in the middle of a word segment.
    if pat_role == CharRole::Head && line_role == CharRole::Tail {
        score -= 10;
    }

    // Penalty: matching the first pattern character in the middle of a segment.
    if pat_idx == 0 && line_role == CharRole::Tail {
        score -= 40;
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fuzzy_matcher::util::{assert_order, wrap_matches};

    fn wrap_fuzzy_match(line: &str, pattern: &str) -> Option<String> {
        let (_score, indices) = fuzzy_indices(line, pattern)?;
        Some(wrap_matches(line, &indices))
    }

    #[test]
    fn test_match_or_not() {
        assert_eq!(None, fuzzy_match("abcdefaghi", "中"));
        assert_eq!(None, fuzzy_match("abc", "abx"));
        assert!(fuzzy_match("axbycz", "abc").is_some());
        assert!(fuzzy_match("axbycz", "xyz").is_some());

        assert_eq!("[a]x[b]y[c]z", &wrap_fuzzy_match("axbycz", "abc").unwrap());
        assert_eq!("a[x]b[y]c[z]", &wrap_fuzzy_match("axbycz", "xyz").unwrap());
        assert_eq!("[H]ello, [世]界", &wrap_fuzzy_match("Hello, 世界", "H世").unwrap());
    }

    #[test]
    fn test_match_quality() {
        let matcher = ClangdMatcher::default();
        // case
        assert_order(&matcher, "monad", &["monad", "Monad", "mONAD"]);

        // initials
        assert_order(&matcher, "ab", &["ab", "aoo_boo", "acb"]);
        assert_order(&matcher, "CC", &["CamelCase", "camelCase", "camelcase"]);
        assert_order(&matcher, "cC", &["camelCase", "CamelCase", "camelcase"]);
        assert_order(
            &matcher,
            "cc",
            &["camel case", "camelCase", "camelcase", "CamelCase", "camel ace"],
        );
        assert_order(
            &matcher,
            "Da.Te",
            &["Data.Text", "Data.Text.Lazy", "Data.Aeson.Encoding.text"],
        );
        assert_order(&matcher, "foobar.h", &["foobar.h", "foo/bar.h"]);
        // prefix
        assert_order(&matcher, "is", &["isIEEE", "inSuf"]);
        // shorter
        assert_order(&matcher, "ma", &["map", "many", "maximum"]);
        assert_order(&matcher, "print", &["printf", "sprintf"]);
        // score(PRINT) = kMinScore
        assert_order(&matcher, "ast", &["ast", "AST", "INT_FAST16_MAX"]);
        // score(PRINT) > kMinScore
        assert_order(&matcher, "Int", &["int", "INT", "PRINT"]);
    }
}

#[allow(dead_code)]
fn print_dp(line: &str, pattern: &str, dp: &[Vec<Score>]) {
    let num_line_chars = line.chars().count();
    let num_pattern_chars = pattern.chars().count();

    print!("\t");
    for (idx, ch) in line.chars().enumerate() {
        print!("\t\t{}/{}", idx + 1, ch);
    }

    for (row_num, row) in dp.iter().enumerate().take(num_pattern_chars + 1) {
        print!("\n{}\t", row_num);
        for cell in row.iter().take(num_line_chars + 1) {
            print!(
                "({},{})/({},{})\t",
                cell.miss_score,
                if cell.last_action_miss == Action::Miss {
                    'X'
                } else {
                    'O'
                },
                cell.match_score,
                if cell.last_action_match == Action::Miss {
                    'X'
                } else {
                    'O'
                }
            );
        }
    }
}
