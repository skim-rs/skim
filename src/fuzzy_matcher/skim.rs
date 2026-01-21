//! The fuzzy matching algorithm used by skim
//!
//! # Example:
//! ```edition2018
//! use crate::fuzzy_matcher::FuzzyMatcher;
//! use crate::fuzzy_matcher::skim::SkimMatcherV2;
//!
//! let matcher = SkimMatcherV2::default();
//! assert_eq!(None, matcher.fuzzy_match("abc", "abx"));
//! assert!(matcher.fuzzy_match("axbycz", "abc").is_some());
//! assert!(matcher.fuzzy_match("axbycz", "xyz").is_some());
//!
//! let (score, indices) = matcher.fuzzy_indices("axbycz", "abc").unwrap();
//! assert_eq!(indices, [0, 2, 4]);
//! ```

#![allow(deprecated)]

use std::cell::RefCell;
use std::cmp::max;
use std::fmt::Formatter;

use thread_local::ThreadLocal;

use super::skim::Movement::{Match, Skip};
use super::util::{char_equal, cheap_matches};
use super::{FuzzyMatcher, IndexType, ScoreType};

const BONUS_MATCHED: ScoreType = 4;
const BONUS_CASE_MATCH: ScoreType = 4;
const BONUS_UPPER_MATCH: ScoreType = 6;
const BONUS_ADJACENCY: ScoreType = 10;
const BONUS_SEPARATOR: ScoreType = 8;
const BONUS_CAMEL: ScoreType = 8;
const PENALTY_CASE_UNMATCHED: ScoreType = -1;
const PENALTY_LEADING: ScoreType = -6;
// penalty applied for every letter before the first match
const PENALTY_MAX_LEADING: ScoreType = -18;
// maxing penalty for leading letters
const PENALTY_UNMATCHED: ScoreType = -2;

#[deprecated(since = "0.3.5", note = "Please use SkimMatcherV2 instead")]
/// Legacy fuzzy matcher (V1) - deprecated, use SkimMatcherV2 instead
#[derive(Default, Debug)]
pub struct SkimMatcher {}

/// The V1 matcher is based on ForrestTheWoods's post
/// https://www.forrestthewoods.com/blog/reverse_engineering_sublime_texts_fuzzy_match/
///
/// V1 algorithm is deprecated, checkout `FuzzyMatcherV2`
impl FuzzyMatcher for SkimMatcher {
    fn fuzzy_indices(&self, choice: &str, pattern: &str) -> Option<(ScoreType, Vec<IndexType>)> {
        fuzzy_indices(choice, pattern)
    }

    fn fuzzy_match(&self, choice: &str, pattern: &str) -> Option<ScoreType> {
        fuzzy_match(choice, pattern)
    }
}

#[deprecated(since = "0.3.5", note = "Please use SkimMatcherV2 instead")]
/// Legacy fuzzy matching function - returns match score only
pub fn fuzzy_match(choice: &str, pattern: &str) -> Option<ScoreType> {
    if pattern.is_empty() {
        return Some(0);
    }

    let scores = build_graph(choice, pattern)?;

    let last_row = &scores[scores.len() - 1];
    let (_, &MatchingStatus { final_score, .. }) = last_row
        .iter()
        .enumerate()
        .max_by_key(|&(_, x)| x.final_score)
        .expect("fuzzy_indices failed to iterate over last_row");
    Some(final_score)
}

#[deprecated(since = "0.3.5", note = "Please use SkimMatcherV2 instead")]
/// Legacy fuzzy matching function - returns match score and character indices
pub fn fuzzy_indices(choice: &str, pattern: &str) -> Option<(ScoreType, Vec<IndexType>)> {
    if pattern.is_empty() {
        return Some((0, Vec::new()));
    }

    let mut picked = vec![];
    let scores = build_graph(choice, pattern)?;

    let last_row = &scores[scores.len() - 1];
    let (mut next_col, &MatchingStatus { final_score, .. }) = last_row
        .iter()
        .enumerate()
        .max_by_key(|&(_, x)| x.final_score)
        .expect("fuzzy_indices failed to iterate over last_row");
    let mut pat_idx = scores.len() as i64 - 1;
    while pat_idx >= 0 {
        let status = scores[pat_idx as usize][next_col];
        next_col = status.back_ref as usize;
        picked.push(status.idx);
        pat_idx -= 1;
    }
    picked.reverse();
    Some((final_score, picked))
}

#[derive(Clone, Copy, Debug)]
struct MatchingStatus {
    pub idx: IndexType,
    pub score: ScoreType,
    pub final_score: ScoreType,
    pub adj_num: IndexType,
    pub back_ref: IndexType,
}

impl Default for MatchingStatus {
    fn default() -> Self {
        MatchingStatus {
            idx: 0,
            score: 0,
            final_score: 0,
            adj_num: 1,
            back_ref: 0,
        }
    }
}

fn build_graph(choice: &str, pattern: &str) -> Option<Vec<Vec<MatchingStatus>>> {
    let mut scores = vec![];

    let mut match_start_idx = 0; // to ensure that the pushed char are able to match the pattern
    let mut pat_prev_ch = '\0';

    // initialize the match positions and inline scores
    for (pat_idx, pat_ch) in pattern.chars().enumerate() {
        let mut vec = vec![];
        let mut choice_prev_ch = '\0';
        for (idx, ch) in choice.chars().enumerate() {
            if ch.eq_ignore_ascii_case(&pat_ch) && idx >= match_start_idx {
                let score = fuzzy_score(
                    ch,
                    idx as IndexType,
                    choice_prev_ch,
                    pat_ch,
                    pat_idx as IndexType,
                    pat_prev_ch,
                );
                vec.push(MatchingStatus {
                    idx: idx as IndexType,
                    score,
                    final_score: score,
                    adj_num: 1,
                    back_ref: 0,
                });
            }
            choice_prev_ch = ch;
        }

        if vec.is_empty() {
            // not matched
            return None;
        }
        match_start_idx = vec[0].idx + 1;
        scores.push(vec);
        pat_prev_ch = pat_ch;
    }

    // calculate max scores considering adjacent characters
    for pat_idx in 1..scores.len() {
        let (first_half, last_half) = scores.split_at_mut(pat_idx);

        let prev_row = &first_half[first_half.len() - 1];
        let cur_row = &mut last_half[0];

        for idx in 0..cur_row.len() {
            let next = cur_row[idx];
            let prev = if idx > 0 {
                cur_row[idx - 1]
            } else {
                MatchingStatus::default()
            };

            let mut score_before_idx = prev.final_score - prev.score + next.score;
            score_before_idx += PENALTY_UNMATCHED * ((next.idx - prev.idx) as ScoreType);
            score_before_idx -= if prev.adj_num == 0 { BONUS_ADJACENCY } else { 0 };

            let (back_ref, score, adj_num) = prev_row
                .iter()
                .enumerate()
                .take_while(|&(_, &MatchingStatus { idx, .. })| idx < next.idx)
                .skip_while(|&(_, &MatchingStatus { idx, .. })| idx < prev.idx)
                .map(|(back_ref, cur)| {
                    let adj_num = next.idx - cur.idx - 1;
                    let mut final_score = cur.final_score + next.score;
                    final_score += if adj_num == 0 {
                        BONUS_ADJACENCY
                    } else {
                        PENALTY_UNMATCHED * adj_num as ScoreType
                    };
                    (back_ref, final_score, adj_num)
                })
                .max_by_key(|&(_, x, _)| x)
                .unwrap_or((prev.back_ref, score_before_idx, prev.adj_num));

            cur_row[idx] = if idx > 0 && score < score_before_idx {
                MatchingStatus {
                    final_score: score_before_idx,
                    back_ref: prev.back_ref,
                    adj_num,
                    ..next
                }
            } else {
                MatchingStatus {
                    final_score: score,
                    back_ref: back_ref as IndexType,
                    adj_num,
                    ..next
                }
            };
        }
    }

    Some(scores)
}

// judge how many scores the current index should get
fn fuzzy_score(
    choice_ch: char,
    choice_idx: IndexType,
    choice_prev_ch: char,
    pat_ch: char,
    pat_idx: IndexType,
    _pat_prev_ch: char,
) -> ScoreType {
    let mut score = BONUS_MATCHED;

    let choice_prev_ch_type = CharType::of(choice_prev_ch);
    let choice_role = CharRole::of(choice_prev_ch, choice_ch);

    if pat_ch == choice_ch {
        if pat_ch.is_uppercase() {
            score += BONUS_UPPER_MATCH;
        } else {
            score += BONUS_CASE_MATCH;
        }
    } else {
        score += PENALTY_CASE_UNMATCHED;
    }

    // apply bonus for camelCases
    if choice_role == CharRole::Head || choice_role == CharRole::Break || choice_role == CharRole::Camel {
        score += BONUS_CAMEL;
    }

    // apply bonus for matches after a separator
    if choice_prev_ch_type == CharType::HardSep || choice_prev_ch_type == CharType::SoftSep {
        score += BONUS_SEPARATOR;
    }

    if pat_idx == 0 {
        score += max((choice_idx as ScoreType) * PENALTY_LEADING, PENALTY_MAX_LEADING);
    }

    score
}

#[derive(Copy, Clone, Debug)]
/// Configuration for skim's scoring algorithm
pub struct SkimScoreConfig {
    /// Score for each matched character
    pub score_match: i32,
    /// Penalty for starting a gap (unmatched characters)
    pub gap_start: i32,
    /// Penalty for extending a gap
    pub gap_extension: i32,

    /// The first character in the typed pattern usually has more significance
    /// than the rest so it's important that it appears at special positions where
    /// bonus points are given. e.g. "to-go" vs. "ongoing" on "og" or on "ogo".
    /// The amount of the extra bonus should be limited so that the gap penalty is
    /// still respected.
    pub bonus_first_char_multiplier: i32,

    /// We prefer matches at the beginning of a word, but the bonus should not be
    /// too great to prevent the longer acronym matches from always winning over
    /// shorter fuzzy matches. The bonus point here was specifically chosen that
    /// the bonus is cancelled when the gap between the acronyms grows over
    /// 8 characters, which is approximately the average length of the words found
    /// in web2 dictionary and my file system.
    pub bonus_head: i32,

    /// Just like bonus_head, but its breakage of word is not that strong, so it should
    /// be slighter less then bonus_head
    pub bonus_break: i32,

    /// Edge-triggered bonus for matches in camelCase words.
    /// Compared to word-boundary case, they don't accompany single-character gaps
    /// (e.g. FooBar vs. foo-bar), so we deduct bonus point accordingly.
    pub bonus_camel: i32,

    /// Minimum bonus point given to characters in consecutive chunks.
    /// Note that bonus points for consecutive matches shouldn't have needed if we
    /// used fixed match score as in the original algorithm.
    pub bonus_consecutive: i32,

    /// Skim will match case-sensitively if the pattern contains ASCII upper case,
    /// If case of case insensitive match, the penalty will be given to case mismatch
    pub penalty_case_mismatch: i32,
}

impl Default for SkimScoreConfig {
    fn default() -> Self {
        let score_match = 16;
        let gap_start = -3;
        let gap_extension = -1;
        let bonus_first_char_multiplier = 2;

        Self {
            score_match,
            gap_start,
            gap_extension,
            bonus_first_char_multiplier,
            bonus_head: score_match / 2,
            bonus_break: score_match / 2 + gap_extension,
            bonus_camel: score_match / 2 + 2 * gap_extension,
            bonus_consecutive: -(gap_start + gap_extension),
            penalty_case_mismatch: gap_extension * 2,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum Movement {
    Match,
    Skip,
}

/// Inner state of the score matrix
// Implementation detail: tried to pad to 16B
// will store the m and p matrix together
#[derive(Clone, Debug)]
struct MatrixCell {
    pub m_move: Movement,
    pub m_score: i32,
    pub p_move: Movement,
    pub p_score: i32, // The max score of align pattern[..i] & choice[..j]

    // temporary fields (make use the rest of the padding)
    pub matched: bool,
    pub bonus: i32,
}

const MATRIX_CELL_NEG_INFINITY: i32 = i16::MIN as i32;

impl Default for MatrixCell {
    fn default() -> Self {
        Self {
            m_move: Skip,
            m_score: MATRIX_CELL_NEG_INFINITY,
            p_move: Skip,
            p_score: MATRIX_CELL_NEG_INFINITY,
            matched: false,
            bonus: 0,
        }
    }
}

impl MatrixCell {
    pub fn reset(&mut self) {
        self.m_move = Skip;
        self.m_score = MATRIX_CELL_NEG_INFINITY;
        self.p_move = Skip;
        self.p_score = MATRIX_CELL_NEG_INFINITY;
        self.bonus = 0;
        self.matched = false;
    }
}

/// Simulate a 1-D vector as 2-D matrix
struct ScoreMatrix<'a> {
    matrix: &'a mut [MatrixCell],
    pub rows: usize,
    pub cols: usize,
}

impl<'a> ScoreMatrix<'a> {
    /// given a matrix, extend it to be (rows x cols) and fill in as init_val
    pub fn new(matrix: &'a mut Vec<MatrixCell>, rows: usize, cols: usize) -> Self {
        matrix.resize(rows * cols, MatrixCell::default());
        ScoreMatrix { matrix, rows, cols }
    }

    #[inline]
    fn get_index(&self, row: usize, col: usize) -> usize {
        row * self.cols + col
    }

    fn get_row(&self, row: usize) -> &[MatrixCell] {
        let start = row * self.cols;
        &self.matrix[start..start + self.cols]
    }
}

impl<'a> std::ops::Index<(usize, usize)> for ScoreMatrix<'a> {
    type Output = MatrixCell;

    fn index(&self, index: (usize, usize)) -> &Self::Output {
        &self.matrix[self.get_index(index.0, index.1)]
    }
}

impl<'a> std::ops::IndexMut<(usize, usize)> for ScoreMatrix<'a> {
    fn index_mut(&mut self, index: (usize, usize)) -> &mut Self::Output {
        &mut self.matrix[self.get_index(index.0, index.1)]
    }
}

impl<'a> std::fmt::Debug for ScoreMatrix<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let _ = writeln!(f, "M score:");
        for row in 0..self.rows {
            for col in 0..self.cols {
                let cell = &self[(row, col)];
                write!(
                    f,
                    "{:4}/{}  ",
                    if cell.m_score == MATRIX_CELL_NEG_INFINITY {
                        -999
                    } else {
                        cell.m_score
                    },
                    match cell.m_move {
                        Match => 'M',
                        Skip => 'S',
                    }
                )?;
            }
            writeln!(f)?;
        }

        let _ = writeln!(f, "P score:");
        for row in 0..self.rows {
            for col in 0..self.cols {
                let cell = &self[(row, col)];
                write!(
                    f,
                    "{:4}/{}  ",
                    if cell.p_score == MATRIX_CELL_NEG_INFINITY {
                        -999
                    } else {
                        cell.p_score
                    },
                    match cell.p_move {
                        Match => 'M',
                        Skip => 'S',
                    }
                )?;
            }
            writeln!(f)?;
        }

        Ok(())
    }
}

/// We categorize characters into types:
///
/// - Empty(E): the start of string
/// - Upper(U): the ascii upper case
/// - lower(L): the ascii lower case & other unicode characters
/// - number(N): ascii number
/// - hard separator(S): clearly separate the content: ` ` `/` `\` `|` `(` `) `[` `]` `{` `}`
/// - soft separator(s): other ascii punctuation, e.g. `!` `"` `#` `$`, ...
#[derive(Debug, PartialEq, Copy, Clone)]
enum CharType {
    Empty,
    Upper,
    Lower,
    Number,
    HardSep,
    SoftSep,
}

impl CharType {
    pub fn of(ch: char) -> Self {
        match ch {
            '\0' => CharType::Empty,
            ' ' | '/' | '\\' | '|' | '(' | ')' | '[' | ']' | '{' | '}' => CharType::HardSep,
            '!'..='\'' | '*'..='.' | ':'..='@' | '^'..='`' | '~' => CharType::SoftSep,
            '0'..='9' => CharType::Number,
            'A'..='Z' => CharType::Upper,
            _ => CharType::Lower,
        }
    }
}

/// Ref: https://github.com/llvm-mirror/clang-tools-extra/blob/master/clangd/FuzzyMatch.cpp
///
///
/// ```text
/// +-----------+--------------+-------+
/// | Example   | Chars | Type | Role  |
/// +-----------+--------------+-------+
/// | (f)oo     | ^fo   | Ell  | Head  |
/// | (F)oo     | ^Fo   | EUl  | Head  |
/// | Foo/(B)ar | /Ba   | SUl  | Head  |
/// | Foo/(b)ar | /ba   | Sll  | Head  |
/// | Foo.(B)ar | .Ba   | SUl  | Break |
/// | Foo(B)ar  | oBa   | lUl  | Camel |
/// | 123(B)ar  | 3Ba   | nUl  | Camel |
/// | F(o)oBar  | Foo   | Ull  | Tail  |
/// | H(T)TP    | HTT   | UUU  | Tail  |
/// | others    |       |      | Tail  |
/// +-----------+--------------+-------+
#[derive(Debug, PartialEq, Copy, Clone)]
enum CharRole {
    Head,
    Tail,
    Camel,
    Break,
}

impl CharRole {
    pub fn of(prev: char, cur: char) -> Self {
        Self::of_type(CharType::of(prev), CharType::of(cur))
    }
    pub fn of_type(prev: CharType, cur: CharType) -> Self {
        match (prev, cur) {
            (CharType::Empty, _) | (CharType::HardSep, _) => CharRole::Head,
            (CharType::SoftSep, _) => CharRole::Break,
            (CharType::Lower, CharType::Upper) | (CharType::Number, CharType::Upper) => CharRole::Camel,
            _ => CharRole::Tail,
        }
    }
}

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
enum CaseMatching {
    Respect,
    Ignore,
    Smart,
}

/// Fuzzy matching is a sub problem is sequence alignment.
/// Specifically what we'd like to implement is sequence alignment with affine gap penalty.
/// Ref: https://www.cs.cmu.edu/~ckingsf/bioinfo-lectures/gaps.pdf
///
/// Given `pattern`(i) and `choice`(j), we'll maintain 2 score matrix:
///
/// ```text
/// M[i][j] = match(i, j) + max(M[i-1][j-1] + consecutive, P[i-1][j-1])
/// M[i][j] = -infinity if p[i][j] do not match
///
/// M[i][j] means the score of best alignment of p[..=i] and c[..=j] ending with match/mismatch e.g.:
///
/// c: [.........]b
/// p: [.........]b
///
/// So that p[..=i-1] and c[..=j-1] could be any alignment
///
/// P[i][j] = max(M[i][j-k]-gap(k)) for k in 1..j
///
/// P[i][j] means the score of best alignment of p[..=i] and c[..=j] where c[j] is not matched.
/// So that we need to search through all the previous matches, and calculate the gap.
///
///   (j-k)--.   j
/// c: [....]bcdef
/// p: [....]b----
///          i
/// ```
///
/// Note that the above is O(n^3) in the worst case. However the above algorithm uses a general gap
/// penalty, but we use affine gap: `gap = gap_start + k * gap_extend` where:
/// - u: the cost of starting of gap
/// - v: the cost of extending a gap by one more space.
///
/// So that we could optimize the algorithm by:
///
/// ```text
/// P[i][j] = max(gap_start + gap_extend + M[i][j-1], gap_extend + P[i][j-1])
/// ```
///
/// Besides, since we are doing fuzzy matching, we'll prefer some pattern over others.
/// So we'll calculate in-place bonus for each character. e.g. bonus for camel cases.
///
/// In summary:
///
/// ```text
/// B[j] = in_place_bonus_of(j)
/// M[i][j] = match(i, j) + max(M[i-1][j-1] + consecutive, P[i-1][j-1])
/// M[i][j] = -infinity if p[i] and c[j] do not match
/// P[i][j] = max(gap_start + gap_extend + M[i][j-1], gap_extend + P[i][j-1])
/// ```
#[derive(Debug)]
pub struct SkimMatcherV2 {
    debug: bool,

    score_config: SkimScoreConfig,
    element_limit: usize,
    case: CaseMatching,
    use_cache: bool,

    m_cache: ThreadLocal<RefCell<Vec<MatrixCell>>>,
    c_cache: ThreadLocal<RefCell<Vec<char>>>, // vector to store the characters of choice
    p_cache: ThreadLocal<RefCell<Vec<char>>>, // vector to store the characters of pattern
}

impl Default for SkimMatcherV2 {
    fn default() -> Self {
        Self {
            debug: false,
            score_config: SkimScoreConfig::default(),
            element_limit: 0,
            case: CaseMatching::Smart,
            use_cache: true,

            m_cache: ThreadLocal::new(),
            c_cache: ThreadLocal::new(),
            p_cache: ThreadLocal::new(),
        }
    }
}

impl SkimMatcherV2 {
    /// Sets the scoring configuration for the matcher
    pub fn score_config(mut self, score_config: SkimScoreConfig) -> Self {
        self.score_config = score_config;
        self
    }

    /// Sets the maximum number of elements to process
    pub fn element_limit(mut self, elements: usize) -> Self {
        self.element_limit = elements;
        self
    }

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

    /// Enables or disables debug mode
    pub fn debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// Build the score matrix using the algorithm described above
    fn build_score_matrix(
        &self,
        m: &mut ScoreMatrix,
        choice: &[char],
        pattern: &[char],
        first_match_indices: &[usize],
        compressed: bool,
        case_sensitive: bool,
    ) {
        let mut in_place_bonuses = vec![0; m.cols];

        self.build_in_place_bonus(choice, &mut in_place_bonuses);

        // need to reset M[row][first_match] & M[i][j-1]
        m[(0, 0)].reset();
        for i in 1..m.rows {
            m[(i, first_match_indices[i - 1])].reset();
        }

        for j in 0..m.cols {
            // p[0][j]: the score of best alignment of p[] and c[..=j] where c[j] is not matched
            m[(0, j)].reset();
            m[(0, j)].p_score = self.score_config.gap_extension;
        }

        // update the matrix;
        for (i, &p_ch) in pattern.iter().enumerate() {
            let row = self.adjust_row_idx(i + 1, compressed);
            let row_prev = self.adjust_row_idx(i, compressed);
            let to_skip = first_match_indices[i];

            // Pre-calculate base indices to reduce repeated index calculations
            let row_base = row * m.cols;
            let row_prev_base = row_prev * m.cols;

            for (j, &c_ch) in choice[to_skip..].iter().enumerate() {
                let col = to_skip + j + 1;
                let col_prev = to_skip + j;

                // Use pre-calculated bases to reduce index calculations
                let idx_cur = row_base + col;
                let idx_last = row_base + col_prev;
                let idx_prev = row_prev_base + col_prev;

                // Cache in_place_bonus lookup to avoid repeated array access
                let in_place_bonus = in_place_bonuses[col];

                // update M matrix
                // M[i][j] = match(i, j) + max(M[i-1][j-1], P[i-1][j-1])
                if let Some(cur_match_score) = self.calculate_match_score(c_ch, p_ch, case_sensitive) {
                    let prev_cell = &m.matrix[idx_prev];
                    let prev_match_score = prev_cell.m_score;
                    let prev_skip_score = prev_cell.p_score;

                    let prev_match_bonus = m.matrix[idx_last].bonus;

                    let consecutive_bonus = max(
                        prev_match_bonus,
                        max(in_place_bonus, self.score_config.bonus_consecutive),
                    );
                    m.matrix[idx_last].bonus = consecutive_bonus;

                    let score_match = prev_match_score + consecutive_bonus;
                    let score_skip = prev_skip_score + in_place_bonus;

                    let cur_cell = &mut m.matrix[idx_cur];
                    if score_match >= score_skip {
                        cur_cell.m_score = score_match + cur_match_score as i32;
                        cur_cell.m_move = Movement::Match;
                    } else {
                        cur_cell.m_score = score_skip + cur_match_score as i32;
                        cur_cell.m_move = Movement::Skip;
                    }
                } else {
                    let cur_cell = &mut m.matrix[idx_cur];
                    cur_cell.m_score = MATRIX_CELL_NEG_INFINITY;
                    cur_cell.m_move = Movement::Skip;
                    cur_cell.bonus = 0;
                }

                // update P matrix
                // P[i][j] = max(gap_start + gap_extend + M[i][j-1], gap_extend + P[i][j-1])
                let last_cell = &m.matrix[idx_last];
                let prev_match_score =
                    self.score_config.gap_start + self.score_config.gap_extension + last_cell.m_score;
                let prev_skip_score = self.score_config.gap_extension + last_cell.p_score;

                let cur_cell = &mut m.matrix[idx_cur];
                if prev_match_score >= prev_skip_score {
                    cur_cell.p_score = prev_match_score;
                    cur_cell.p_move = Movement::Match;
                } else {
                    cur_cell.p_score = prev_skip_score;
                    cur_cell.p_move = Movement::Skip;
                }
            }
        }
    }

    /// check bonus for start of camel case, etc.
    fn build_in_place_bonus(&self, choice: &[char], b: &mut [i32]) {
        let mut prev_ch = '\0';
        for (j, &c_ch) in choice.iter().enumerate() {
            let prev_ch_type = CharType::of(prev_ch);
            let ch_type = CharType::of(c_ch);
            b[j + 1] = self.in_place_bonus(prev_ch_type, ch_type);
            prev_ch = c_ch;
        }

        if b.len() > 1 {
            b[1] *= self.score_config.bonus_first_char_multiplier;
        }
    }

    /// In case we don't need to backtrack the matching indices, we could use only 2 rows for the
    /// matrix, this function could be used to rotate accessing these two rows.
    fn adjust_row_idx(&self, row_idx: usize, compressed: bool) -> usize {
        if compressed { row_idx & 1 } else { row_idx }
    }

    /// Calculate the matching score of the characters
    /// return None if not matched.
    fn calculate_match_score(&self, c: char, p: char, case_sensitive: bool) -> Option<u16> {
        if !char_equal(c, p, case_sensitive) {
            return None;
        }

        let score = self.score_config.score_match;
        let mut bonus = 0;

        // penalty on case mismatch
        if !case_sensitive && p != c {
            bonus += self.score_config.penalty_case_mismatch;
        }

        Some(max(0, score + bonus) as u16)
    }

    #[inline]
    fn in_place_bonus(&self, prev_char_type: CharType, char_type: CharType) -> i32 {
        match CharRole::of_type(prev_char_type, char_type) {
            CharRole::Head => self.score_config.bonus_head,
            CharRole::Camel => self.score_config.bonus_camel,
            CharRole::Break => self.score_config.bonus_break,
            CharRole::Tail => 0,
        }
    }

    fn contains_upper(&self, string: &str) -> bool {
        string.chars().any(|ch| ch.is_ascii_uppercase())
    }

    /// Performs fuzzy matching with full algorithm and returns score and indices
    pub fn fuzzy(&self, choice: &str, pattern: &str, with_pos: bool) -> Option<(ScoreType, Vec<IndexType>)> {
        if pattern.is_empty() {
            return Some((0, Vec::new()));
        }

        let case_sensitive = match self.case {
            CaseMatching::Respect => true,
            CaseMatching::Ignore => false,
            CaseMatching::Smart => self.contains_upper(pattern),
        };

        let compressed = !with_pos;

        // initialize the score matrix
        let mut m = self.m_cache.get_or(|| RefCell::new(Vec::new())).borrow_mut();
        let mut choice_chars = self.c_cache.get_or(|| RefCell::new(Vec::new())).borrow_mut();
        let mut pattern_chars = self.p_cache.get_or(|| RefCell::new(Vec::new())).borrow_mut();

        choice_chars.clear();
        choice_chars.reserve(choice.chars().count());
        choice_chars.extend(choice.chars());

        pattern_chars.clear();
        pattern_chars.reserve(pattern.chars().count());
        pattern_chars.extend(pattern.chars());

        let first_match_indices = cheap_matches(&choice_chars, &pattern_chars, case_sensitive)?;

        let cols = choice_chars.len() + 1;
        let num_char_pattern = pattern_chars.len();
        let rows = if compressed { 2 } else { num_char_pattern + 1 };

        if self.element_limit > 0 && self.element_limit < rows * cols {
            return self.simple_match(
                &choice_chars,
                &pattern_chars,
                &first_match_indices,
                case_sensitive,
                with_pos,
            );
        }

        let mut m = ScoreMatrix::new(&mut m, rows, cols);
        self.build_score_matrix(
            &mut m,
            &choice_chars,
            &pattern_chars,
            &first_match_indices,
            compressed,
            case_sensitive,
        );
        let first_col_of_last_row = first_match_indices[first_match_indices.len() - 1];
        let last_row = m.get_row(self.adjust_row_idx(num_char_pattern, compressed));
        let (pat_idx, &MatrixCell { m_score, .. }) = last_row[first_col_of_last_row..]
            .iter()
            .enumerate()
            .max_by_key(|&(_, x)| x.m_score)
            .map(|(idx, cell)| (idx + first_col_of_last_row, cell))
            .expect("fuzzy_matcher failed to iterate over last_row");

        let mut positions = if with_pos {
            Vec::with_capacity(num_char_pattern)
        } else {
            Vec::new()
        };
        if with_pos {
            let mut i = m.rows - 1;
            let mut j = pat_idx;
            let mut track_m = true;
            let mut current_move = Match;
            let first_col_first_row = first_match_indices[0];
            while i > 0 && j > first_col_first_row {
                if current_move == Match {
                    positions.push((j - 1) as IndexType);
                }

                let cell = &m[(i, j)];
                current_move = if track_m { cell.m_move } else { cell.p_move };
                if track_m {
                    i -= 1;
                }

                j -= 1;

                track_m = match current_move {
                    Match => true,
                    Skip => false,
                };
            }
            positions.reverse();
        }

        if self.debug {
            println!("Matrix:\n{:?}", m);
        }

        if !self.use_cache {
            // drop the allocated memory
            self.m_cache.get().map(|cell| cell.replace(vec![]));
            self.c_cache.get().map(|cell| cell.replace(vec![]));
            self.p_cache.get().map(|cell| cell.replace(vec![]));
        }

        Some((m_score as ScoreType, positions))
    }

    /// Performs simple pattern matching for cases where the full algorithm isn't needed
    pub fn simple_match(
        &self,
        choice: &[char],
        pattern: &[char],
        first_match_indices: &[usize],
        case_sensitive: bool,
        with_pos: bool,
    ) -> Option<(ScoreType, Vec<IndexType>)> {
        if pattern.is_empty() {
            return Some((0, Vec::new()));
        } else if pattern.len() == 1 {
            let match_idx = first_match_indices[0];
            let prev_ch = if match_idx > 0 { choice[match_idx - 1] } else { '\0' };
            let prev_ch_type = CharType::of(prev_ch);
            let ch_type = CharType::of(choice[match_idx]);
            let in_place_bonus = self.in_place_bonus(prev_ch_type, ch_type);
            return Some((in_place_bonus as ScoreType, vec![match_idx as IndexType]));
        }

        let mut start_idx = first_match_indices[0];
        let end_idx = first_match_indices[first_match_indices.len() - 1];

        let mut pattern_iter = pattern.iter().rev().peekable();
        for (idx, &c) in choice[start_idx..=end_idx].iter().enumerate().rev() {
            match pattern_iter.peek() {
                Some(&&p) => {
                    if char_equal(c, p, case_sensitive) {
                        let _ = pattern_iter.next();
                        start_idx = idx;
                    }
                }
                None => break,
            }
        }

        Some(self.calculate_score_with_pos(choice, pattern, start_idx, end_idx, case_sensitive, with_pos))
    }

    fn calculate_score_with_pos(
        &self,
        choice: &[char],
        pattern: &[char],
        start_idx: usize,
        end_idx: usize,
        case_sensitive: bool,
        with_pos: bool,
    ) -> (ScoreType, Vec<IndexType>) {
        let mut pos = Vec::new();

        let choice_iter = choice[start_idx..=end_idx].iter().enumerate();
        let mut pattern_iter = pattern.iter().enumerate().peekable();

        // unfortunately we could not get the the character before the first character's(for performance)
        // so we tread them as NonWord
        let mut prev_ch = '\0';

        let mut score: i32 = 0;
        let mut in_gap = false;
        let mut prev_match_bonus = 0;

        for (c_idx, &c) in choice_iter {
            let op = pattern_iter.peek();
            if op.is_none() {
                break;
            }

            let prev_ch_type = CharType::of(prev_ch);
            let ch_type = CharType::of(c);
            let in_place_bonus = self.in_place_bonus(prev_ch_type, ch_type);

            let (_p_idx, &p) = *op.unwrap();

            if let Some(match_score) = self.calculate_match_score(c, p, case_sensitive) {
                if with_pos {
                    pos.push((c_idx + start_idx) as IndexType);
                }

                score += match_score as i32;

                let consecutive_bonus = max(
                    prev_match_bonus,
                    max(in_place_bonus, self.score_config.bonus_consecutive),
                );
                prev_match_bonus = consecutive_bonus;

                if !in_gap {
                    score += consecutive_bonus;
                }

                in_gap = false;
                let _ = pattern_iter.next();
            } else {
                if !in_gap {
                    score += self.score_config.gap_start;
                }

                score += self.score_config.gap_extension;
                in_gap = true;
                prev_match_bonus = 0;
            }

            prev_ch = c;
        }

        (score as ScoreType, pos)
    }
}

impl FuzzyMatcher for SkimMatcherV2 {
    fn fuzzy_indices(&self, choice: &str, pattern: &str) -> Option<(ScoreType, Vec<IndexType>)> {
        self.fuzzy(choice, pattern, true)
    }

    fn fuzzy_match(&self, choice: &str, pattern: &str) -> Option<ScoreType> {
        self.fuzzy(choice, pattern, false).map(|(score, _)| score)
    }
}

#[cfg(test)]
mod tests {
    use crate::fuzzy_matcher::util::{assert_order, wrap_matches};

    use super::*;

    fn wrap_fuzzy_match(matcher: &dyn FuzzyMatcher, line: &str, pattern: &str) -> Option<String> {
        let (_score, indices) = matcher.fuzzy_indices(line, pattern)?;
        println!("score: {:?}, indices: {:?}", _score, indices);
        Some(wrap_matches(line, &indices))
    }

    #[test]
    fn test_match_or_not() {
        let matcher = SkimMatcherV2::default();
        assert_eq!(Some(0), matcher.fuzzy_match("", ""));
        assert_eq!(Some(0), matcher.fuzzy_match("abcdefaghi", ""));
        assert_eq!(None, matcher.fuzzy_match("", "a"));
        assert_eq!(None, matcher.fuzzy_match("abcdefaghi", "中"));
        assert_eq!(None, matcher.fuzzy_match("abc", "abx"));
        assert!(matcher.fuzzy_match("axbycz", "abc").is_some());
        assert!(matcher.fuzzy_match("axbycz", "xyz").is_some());

        assert_eq!("[a]x[b]y[c]z", &wrap_fuzzy_match(&matcher, "axbycz", "abc").unwrap());
        assert_eq!("a[x]b[y]c[z]", &wrap_fuzzy_match(&matcher, "axbycz", "xyz").unwrap());
        assert_eq!(
            "[H]ello, [世]界",
            &wrap_fuzzy_match(&matcher, "Hello, 世界", "H世").unwrap()
        );
    }

    #[test]
    fn test_match_quality() {
        let matcher = SkimMatcherV2::default().ignore_case();

        // initials
        assert_order(&matcher, "ab", &["ab", "aoo_boo", "acb"]);
        assert_order(&matcher, "CC", &["CamelCase", "camelCase", "camelcase"]);
        assert_order(&matcher, "cC", &["camelCase", "CamelCase", "camelcase"]);
        assert_order(
            &matcher,
            "cc",
            &["camel case", "camelCase", "CamelCase", "camelcase", "camel ace"],
        );
        assert_order(
            &matcher,
            "Da.Te",
            &["Data.Text", "Data.Text.Lazy", "Data.Aeson.Encoding.text"],
        );
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

    fn simple_match(
        matcher: &SkimMatcherV2,
        choice: &str,
        pattern: &str,
        case_sensitive: bool,
        with_pos: bool,
    ) -> Option<(ScoreType, Vec<IndexType>)> {
        let choice: Vec<char> = choice.chars().collect();
        let pattern: Vec<char> = pattern.chars().collect();
        let first_match_indices = cheap_matches(&choice, &pattern, case_sensitive)?;
        matcher.simple_match(&choice, &pattern, &first_match_indices, case_sensitive, with_pos)
    }

    #[test]
    fn test_match_or_not_simple() {
        let matcher = SkimMatcherV2::default();
        assert_eq!(
            simple_match(&matcher, "axbycz", "xyz", false, true).unwrap().1,
            vec![1, 3, 5]
        );

        assert_eq!(simple_match(&matcher, "", "", false, false), Some((0, vec![])));
        assert_eq!(
            simple_match(&matcher, "abcdefaghi", "", false, false),
            Some((0, vec![]))
        );
        assert_eq!(simple_match(&matcher, "", "a", false, false), None);
        assert_eq!(simple_match(&matcher, "abcdefaghi", "中", false, false), None);
        assert_eq!(simple_match(&matcher, "abc", "abx", false, false), None);
        assert_eq!(
            simple_match(&matcher, "axbycz", "abc", false, true).unwrap().1,
            vec![0, 2, 4]
        );
        assert_eq!(
            simple_match(&matcher, "axbycz", "xyz", false, true).unwrap().1,
            vec![1, 3, 5]
        );
        assert_eq!(
            simple_match(&matcher, "Hello, 世界", "H世", false, true).unwrap().1,
            vec![0, 7]
        );
    }

    #[test]
    fn test_match_or_not_v2() {
        let matcher = SkimMatcherV2::default().debug(true);

        assert_eq!(matcher.fuzzy_match("", ""), Some(0));
        assert_eq!(matcher.fuzzy_match("abcdefaghi", ""), Some(0));
        assert_eq!(matcher.fuzzy_match("", "a"), None);
        assert_eq!(matcher.fuzzy_match("abcdefaghi", "中"), None);
        assert_eq!(matcher.fuzzy_match("abc", "abx"), None);
        assert!(matcher.fuzzy_match("axbycz", "abc").is_some());
        assert!(matcher.fuzzy_match("axbycz", "xyz").is_some());

        assert_eq!(&wrap_fuzzy_match(&matcher, "axbycz", "abc").unwrap(), "[a]x[b]y[c]z");
        assert_eq!(&wrap_fuzzy_match(&matcher, "axbycz", "xyz").unwrap(), "a[x]b[y]c[z]");
        assert_eq!(
            &wrap_fuzzy_match(&matcher, "Hello, 世界", "H世").unwrap(),
            "[H]ello, [世]界"
        );
    }

    #[test]
    fn test_case_option_v2() {
        let matcher = SkimMatcherV2::default().ignore_case();
        assert!(matcher.fuzzy_match("aBc", "abc").is_some());
        assert!(matcher.fuzzy_match("aBc", "aBc").is_some());
        assert!(matcher.fuzzy_match("aBc", "aBC").is_some());

        let matcher = SkimMatcherV2::default().respect_case();
        assert!(matcher.fuzzy_match("aBc", "abc").is_none());
        assert!(matcher.fuzzy_match("aBc", "aBc").is_some());
        assert!(matcher.fuzzy_match("aBc", "aBC").is_none());

        let matcher = SkimMatcherV2::default().smart_case();
        assert!(matcher.fuzzy_match("aBc", "abc").is_some());
        assert!(matcher.fuzzy_match("aBc", "aBc").is_some());
        assert!(matcher.fuzzy_match("aBc", "aBC").is_none());
    }

    #[test]
    fn test_matcher_quality_v2() {
        let matcher = SkimMatcherV2::default();
        assert_order(&matcher, "ab", &["ab", "aoo_boo", "acb"]);
        assert_order(
            &matcher,
            "cc",
            &["camel case", "camelCase", "CamelCase", "camelcase", "camel ace"],
        );
        assert_order(
            &matcher,
            "Da.Te",
            &["Data.Text", "Data.Text.Lazy", "Data.Aeson.Encoding.Text"],
        );
        assert_order(&matcher, "is", &["isIEEE", "inSuf"]);
        assert_order(&matcher, "ma", &["map", "many", "maximum"]);
        assert_order(&matcher, "print", &["printf", "sprintf"]);
        assert_order(&matcher, "ast", &["ast", "AST", "INT_FAST16_MAX"]);
        assert_order(&matcher, "int", &["int", "INT", "PRINT"]);
    }

    #[test]
    fn test_reuse_should_not_affect_indices() {
        let matcher = SkimMatcherV2::default();
        let pattern = "139";
        for num in 0..10000 {
            let choice = num.to_string();
            if let Some((_score, indices)) = matcher.fuzzy_indices(&choice, pattern) {
                assert_eq!(indices.len(), 3);
            }
        }
    }
}
