//! Root shims so the pre-optimization Arinae sources in this directory (copied
//! verbatim from skim 2160abb) compile inside the fuzz binary: they refer to
//! `crate::CaseMatching` and `crate::fuzzy_matcher::*`.

pub mod fuzzy_matcher {
    pub use skim::fuzzy_matcher::FuzzyMatcher;
    pub type IndexType = usize;
    pub type ScoreType = i64;
    pub type MatchIndices = Vec<IndexType>;

    pub mod util {
        // Copy of skim's private `fuzzy_matcher::util::char_equal` (unchanged).
        fn narrow_ascii_width(ch: char) -> char {
            match ch {
                '\u{3000}' => ' ',
                '\u{FF01}'..='\u{FF5E}' => char::from_u32(ch as u32 - 0xFEE0).unwrap_or(ch),
                _ => ch,
            }
        }

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
    }
}
