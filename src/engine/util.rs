use regex::Regex;
use unicode_normalization::UnicodeNormalization;

/// Normalize a string and return a mapping from normalized char indices to original char indices.
///
/// Returns (normalized_string, mapping) where mapping[i] gives the original char index
/// for the i-th character in the normalized string.
pub fn normalize_with_char_mapping(s: &str) -> (String, Vec<usize>) {
    let mut normalized = String::new();
    let mut mapping = Vec::new();

    for (orig_char_idx, orig_char) in s.chars().enumerate() {
        // Decompose this character into NFD form
        for decomposed_char in orig_char.nfd() {
            if !unicode_normalization::char::is_combining_mark(decomposed_char) {
                normalized.push(decomposed_char);
                mapping.push(orig_char_idx);
            }
        }
    }

    (normalized, mapping)
}

/// Map character indices from normalized string back to original string.
///
/// Given indices into a normalized string and the char mapping from normalize_with_char_mapping,
/// returns the corresponding indices in the original string.
pub fn map_char_indices_to_original(normalized_indices: &[usize], char_mapping: &[usize]) -> Vec<usize> {
    normalized_indices
        .iter()
        .filter_map(|&idx| char_mapping.get(idx).copied())
        .collect()
}

/// Normalize a string and return a mapping from normalized byte positions to original byte positions.
///
/// Returns (normalized_string, byte_mapping) where byte_mapping[i] gives the original byte position
/// for the i-th byte in the normalized string.
pub fn normalize_with_byte_mapping(s: &str) -> (String, Vec<usize>) {
    let mut normalized = String::new();
    let mut byte_mapping = Vec::new();

    for (orig_byte_pos, orig_char) in s.char_indices() {
        // Decompose this character into NFD form
        for decomposed_char in orig_char.nfd() {
            if !unicode_normalization::char::is_combining_mark(decomposed_char) {
                let char_start = normalized.len();
                normalized.push(decomposed_char);
                // Map each byte of the decomposed char to the original byte position
                for _ in char_start..normalized.len() {
                    byte_mapping.push(orig_byte_pos);
                }
            }
        }
    }

    (normalized, byte_mapping)
}

/// Map a byte range from normalized string back to original string.
///
/// Given a (start, end) byte range in a normalized string and the byte mapping,
/// returns the corresponding (start, end) byte range in the original string.
pub fn map_byte_range_to_original(
    normalized_start: usize,
    normalized_end: usize,
    byte_mapping: &[usize],
    original_str: &str,
) -> (usize, usize) {
    if byte_mapping.is_empty() || normalized_start >= byte_mapping.len() {
        return (0, 0);
    }

    let orig_start = byte_mapping[normalized_start];

    // For the end, we need to find the end of the original character
    // that contains the last byte of the normalized range
    let orig_end = if normalized_end > 0 && normalized_end <= byte_mapping.len() {
        let last_byte_orig_pos = byte_mapping[normalized_end - 1];
        // Find the end of the character at this position in the original string
        original_str[last_byte_orig_pos..]
            .chars()
            .next()
            .map(|c| last_byte_orig_pos + c.len_utf8())
            .unwrap_or(original_str.len())
    } else if normalized_end >= byte_mapping.len() {
        original_str.len()
    } else {
        orig_start
    };

    (orig_start, orig_end)
}

pub fn regex_match(choice: &str, pattern: &Option<Regex>) -> Option<(usize, usize)> {
    match *pattern {
        Some(ref pat) => {
            let mat = pat.find(choice)?;
            Some((mat.start(), mat.end()))
        }
        None => None,
    }
}

pub fn contains_upper(string: &str) -> bool {
    for ch in string.chars() {
        if ch.is_uppercase() {
            return true;
        }
    }
    false
}
