// ---------------------------------------------------------------------------
// Scoring constants
// ---------------------------------------------------------------------------
use super::Score;

/// Points awarded for each correctly matched character.
pub(super) const MATCH_BONUS: Score = 18;

/// Extra bonus when the match is at position 0 of the choice string.
pub(super) const START_OF_STRING_BONUS: Score = 16;

/// Extra bonus for a camelCase transition.
pub(super) const CAMEL_CASE_BONUS: Score = 6;

/// Bonus for each additional consecutive matched character.
pub(super) const CONSECUTIVE_BONUS: Score = 11;

/// Cost to open a gap (skip characters in choice).
pub(super) const GAP_OPEN: Score = 6;

/// Cost to extend a gap by one more character.
pub(super) const GAP_EXTEND: Score = 2;

pub(super) const TYPO_PENALTY: Score = 8;

/// Penalty for aligning a pattern char to a different choice char (typos only).
pub(super) const MISMATCH_PENALTY: Score = 16;

/// Maximum pattern length supported by the banding arrays (stack-allocated).
pub(super) const MAX_PAT_LEN: usize = 32;

/// Bandwidth for typo-mode banding. In typo mode we allow diagonal moves
/// (match/mismatch) plus UP (skip pattern char) and LEFT (skip choice char),
/// so the optimal path can wander off the main diagonal. A bandwidth of
/// `n + TYPO_BAND_SLACK` columns around the diagonal is generous enough
/// to capture all viable alignments while still pruning far-off cells.
pub(super) const TYPO_BAND_SLACK: usize = 4;

/// Per-separator bonus lookup table. Each entry holds the `Score` awarded when
/// a matched character immediately follows that ASCII codepoint. Non-separator
/// characters (and all non-ASCII codepoints) map to `0`.
///
/// Different separators can carry different bonuses — for example, `/` and `\`
/// delimit path components (high bonus), while `_` or `-` delimit sub-words
/// (standard bonus).  Entries that are `0` are not considered separators.
pub(super) const SEPARATOR_TABLE: [Score; 128] = {
    let mut t = [0 as Score; 128];
    t[b' ' as usize] = 12; // space
    t[b'-' as usize] = 10; // hyphen / kebab-case
    t[b'.' as usize] = 12; // dot (file extensions, domain names)
    t[b'/' as usize] = 16; // forward slash (path separator — higher bonus)
    t[b'\\' as usize] = 16; // backslash (Windows path separator — higher bonus)
    t[b'_' as usize] = 12; // underscore / snake_case
    t
};
