use super::{FuzzyMatcher, IndexType, ScoreType};

pub fn cheap_matches(choice: &[char], pattern: &[char], case_sensitive: bool) -> Option<Vec<usize>> {
    let mut first_match_indices = vec![];
    let mut pattern_iter = pattern.iter().peekable();
    for (idx, &c) in choice.iter().enumerate() {
        match pattern_iter.peek() {
            Some(&&p) => {
                if char_equal(c, p, case_sensitive) {
                    first_match_indices.push(idx);
                    let _ = pattern_iter.next();
                }
            }
            None => break,
        }
    }

    if pattern_iter.peek().is_none() {
        Some(first_match_indices)
    } else {
        None
    }
}

/// Convert the Unicode fullwidth form of an ASCII character to ASCII.
#[inline]
fn narrow_ascii_width(ch: char) -> char {
    match ch {
        '\u{3000}' => ' ',
        '\u{FF01}'..='\u{FF5E}' => char::from_u32(ch as u32 - 0xFEE0).unwrap_or(ch),
        _ => ch,
    }
}

/// Given two characters, check if they are equal after folding ASCII width and,
/// when requested, case.
/// e.g. ('a', 'A', true) => false
/// e.g. ('a', 'A', false) => true
/// e.g. ('ａ', 'a', true) => true
#[inline]
pub fn char_equal(a: char, b: char, case_sensitive: bool) -> bool {
    if a == b {
        return true;
    }

    let a = narrow_ascii_width(a);
    let b = narrow_ascii_width(b);

    if a == b {
        return true;
    }

    if case_sensitive {
        return false;
    }

    if a.is_ascii() && b.is_ascii() {
        return a.eq_ignore_ascii_case(&b);
    }

    a.to_lowercase().eq(b.to_lowercase())
}

#[derive(Debug, PartialEq)]
pub enum CharType {
    NonWord,
    Lower,
    Upper,
    Number,
}

#[inline]
pub fn char_type_of(ch: char) -> CharType {
    if ch.is_lowercase() {
        CharType::Lower
    } else if ch.is_uppercase() {
        CharType::Upper
    } else if ch.is_numeric() {
        CharType::Number
    } else {
        CharType::NonWord
    }
}

#[derive(Debug, PartialEq)]
pub enum CharRole {
    Tail,
    Head,
}

// checkout https://github.com/llvm-mirror/clang-tools-extra/blob/master/clangd/FuzzyMatch.cpp
// The Role can be determined from the Type of a character and its neighbors:
//
//   Example  | Chars | Type | Role
//   ---------+--------------+-----
//   F(o)oBar | Foo   | Ull  | Tail
//   Foo(B)ar | oBa   | lUl  | Head
//   (f)oo    | ^fo   | Ell  | Head
//   H(T)TP   | HTT   | UUU  | Tail
//
//      Curr= Empty Lower Upper Separ
// Prev=Empty 0x00, 0xaa, 0xaa, 0xff, // At start, Lower|Upper->Head
// Prev=Lower 0x00, 0x55, 0xaa, 0xff, // In word, Upper->Head;Lower->Tail
// Prev=Upper 0x00, 0x55, 0x59, 0xff, // Ditto, but U(U)U->Tail
// Prev=Separ 0x00, 0xaa, 0xaa, 0xff, // After separator, like at start
pub fn char_role(prev: char, cur: char) -> CharRole {
    use self::CharRole::{Head, Tail};
    use self::CharType::{Lower, NonWord, Upper};
    match (char_type_of(prev), char_type_of(cur)) {
        (Lower | NonWord, Upper) | (NonWord, Lower) => Head,
        _ => Tail,
    }
}

#[allow(dead_code)]
pub fn assert_order(matcher: &dyn FuzzyMatcher, pattern: &str, choices: &[&'static str]) {
    let result = filter_and_sort(matcher, pattern, choices);

    if result != choices {
        // debug print
        println!("pattern: {pattern}");
        for &choice in choices {
            if let Some((score, indices)) = matcher.fuzzy_indices(choice, pattern) {
                println!("{}: {:?}", score, wrap_matches(choice, &indices));
            } else {
                println!("NO MATCH for {choice}");
            }
        }
    }

    assert_eq!(result, choices);
}

#[allow(dead_code)]
pub fn filter_and_sort(matcher: &dyn FuzzyMatcher, pattern: &str, lines: &[&'static str]) -> Vec<&'static str> {
    let mut lines_with_score: Vec<(ScoreType, &'static str)> = lines
        .iter()
        .filter_map(|&s| matcher.fuzzy_match(s, pattern).map(|score| (score, s)))
        .collect();
    lines_with_score.sort_by_key(|(score, _)| -score);
    lines_with_score.into_iter().map(|(_, string)| string).collect()
}

#[allow(dead_code)]
pub fn wrap_matches(line: &str, indices: &[IndexType]) -> String {
    let mut ret = String::new();
    let mut peekable = indices.iter().peekable();
    for (idx, ch) in line.chars().enumerate() {
        let next_id = **peekable.peek().unwrap_or(&&(line.len() as IndexType));
        if next_id == (idx as IndexType) {
            ret.push_str(format!("[{ch}]").as_str());
            peekable.next();
        } else {
            ret.push(ch);
        }
    }

    ret
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn char_equal_case_sensitive() {
        assert!(char_equal('a', 'a', true));
        assert!(!char_equal('a', 'A', true));
    }

    #[test]
    fn char_equal_case_insensitive() {
        assert!(char_equal('a', 'A', false));
        assert!(!char_equal('a', 'b', false));
    }

    #[test]
    fn char_equal_folds_fullwidth_ascii() {
        assert!(char_equal('ａ', 'a', true));
        assert!(char_equal('Ａ', 'A', true));
        assert!(!char_equal('Ａ', 'a', true));
        assert!(char_equal('Ａ', 'a', false));
        assert!(char_equal('１', '1', true));
        assert!(char_equal('　', ' ', true));
    }

    #[test]
    fn char_equal_multichar_lowercase_mismatch() {
        // 'İ' (U+0130) lowercases to two chars ("i" + combining dot), so it is
        // not equal to the single char 'i' — exercising the length-mismatch path.
        assert!(!char_equal('İ', 'i', false));
        // ...and the comparison must be symmetric. This direction used to
        // return `true` because the shorter sequence was exhausted first, which
        // made `cheap_matches` and `allow_match` disagree and drove the clangd
        // matcher's backtracking past the start of the DP matrix.
        assert!(!char_equal('i', 'İ', false));
        assert!(!char_equal('I', 'İ', false));
        assert!(!char_equal('İ', 'I', false));
    }

    #[test]
    fn char_equal_is_symmetric() {
        // Exhaustively check symmetry against chars with multi-char or
        // otherwise unusual lowercase mappings.
        let probes = ['i', 'I', 'İ', 'ı', 'ß', 'ẞ', 'ﬄ', 'ς', 'Σ', 'K', 'İ', 'ǅ'];
        for u in 0..=0x2FFFu32 {
            let Some(ch) = char::from_u32(u) else { continue };
            for p in probes {
                for case_sensitive in [true, false] {
                    assert_eq!(
                        char_equal(ch, p, case_sensitive),
                        char_equal(p, ch, case_sensitive),
                        "char_equal is asymmetric for ({ch:?}, {p:?}, {case_sensitive})"
                    );
                }
            }
        }
    }

    #[test]
    fn char_equal_reflexive() {
        for u in 0..=0x2FFFu32 {
            let Some(ch) = char::from_u32(u) else { continue };
            assert!(char_equal(ch, ch, true));
            assert!(char_equal(ch, ch, false));
        }
    }

    #[test]
    fn cheap_matches_subsequence() {
        let choice: Vec<char> = "hello".chars().collect();
        let pattern: Vec<char> = "hlo".chars().collect();
        assert_eq!(cheap_matches(&choice, &pattern, true), Some(vec![0, 2, 4]));
    }

    #[test]
    fn cheap_matches_no_match() {
        let choice: Vec<char> = "hello".chars().collect();
        let pattern: Vec<char> = "xyz".chars().collect();
        assert_eq!(cheap_matches(&choice, &pattern, true), None);
    }

    #[test]
    fn char_type_and_role() {
        assert_eq!(char_type_of('a'), CharType::Lower);
        assert_eq!(char_type_of('A'), CharType::Upper);
        assert_eq!(char_type_of('1'), CharType::Number);
        assert_eq!(char_type_of('-'), CharType::NonWord);

        assert_eq!(char_role('o', 'B'), CharRole::Head);
        assert_eq!(char_role('-', 'f'), CharRole::Head);
        assert_eq!(char_role('F', 'o'), CharRole::Tail);
        assert_eq!(char_role('H', 'T'), CharRole::Tail);
    }

    #[test]
    fn wrap_matches_brackets_indices() {
        assert_eq!(wrap_matches("hello", &[0, 4]), "[h]ell[o]");
        assert_eq!(wrap_matches("hi", &[]), "hi");
    }

    #[test]
    #[should_panic = "assertion `left == right` failed"]
    fn assert_order_panics_on_wrong_order() {
        use crate::fuzzy_matcher::skim::SkimMatcherV2;

        let matcher = SkimMatcherV2::default();
        assert_order(&matcher, "abc", &["zzz", "abc"]);
    }
}
