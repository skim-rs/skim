// ---------------------------------------------------------------------------
// Scoring constants
// ---------------------------------------------------------------------------
use super::Score;

/// Points awarded for each correctly matched character.
pub(super) const MATCH_BONUS: Score = 18;

/// Extra bonus when the match is at position 0 of the choice string.
pub(super) const START_OF_STRING_BONUS: Score = 12;

/// Extra bonus when the match follows a word separator.
pub(super) const START_OF_WORD_BONUS: Score = 8;

/// Extra bonus for a camelCase transition.
pub(super) const CAMEL_CASE_BONUS: Score = 6;

/// Bonus for each additional consecutive matched character.
pub(super) const CONSECUTIVE_BONUS: Score = 8;

/// Cost to open a gap (skip characters in choice).
pub(super) const GAP_OPEN: Score = 6;

/// Cost to extend a gap by one more character.
pub(super) const GAP_EXTEND: Score = 2;

pub(super) const TYPO_PENALTY: Score = 4;

/// Penalty for aligning a pattern char to a different choice char (typos only).
pub(super) const MISMATCH_PENALTY: Score = 12;

/// Maximum pattern length supported by the banding arrays (stack-allocated).
pub(super) const MAX_PAT_LEN: usize = 16;

/// Bandwidth for typo-mode banding. In typo mode we allow diagonal moves
/// (match/mismatch) plus UP (skip pattern char) and LEFT (skip choice char),
/// so the optimal path can wander off the main diagonal. A bandwidth of
/// `n + TYPO_BAND_SLACK` columns around the diagonal is generous enough
/// to capture all viable alignments while still pruning far-off cells.
pub(super) const TYPO_BAND_SLACK: usize = 4;
