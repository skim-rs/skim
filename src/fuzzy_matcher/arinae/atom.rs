//! Byte/Char helpers
use super::Score;
use super::constants::SEPARATOR_TABLE;
use memchr::{memchr, memrchr};

pub(super) trait Atom: PartialEq + Into<char> + Copy {
    #[inline(always)]
    fn eq(self, other: Self, respect_case: bool) -> bool
    where
        Self: PartialEq + Sized,
    {
        if respect_case {
            self == other
        } else {
            self.eq_ignore_case(other)
        }
    }
    fn eq_ignore_case(self, other: Self) -> bool;
    fn is_lowercase(self) -> bool;

    /// Return the index of the first occurrence of `self` in `haystack`,
    /// or `None` if not found.
    ///
    /// Implementations may override this with a SIMD-backed search (e.g.
    /// `memchr` for `u8` in case-sensitive mode).
    #[inline(always)]
    fn find_first_in(self, haystack: &[Self], respect_case: bool) -> Option<usize> {
        haystack.iter().position(|&c| self.eq(c, respect_case))
    }

    /// Return the index of the last occurrence of `self` in `haystack`,
    /// or `None` if not found.
    ///
    /// Implementations may override this with a SIMD-backed search (e.g.
    /// `memrchr` for `u8` in case-sensitive mode).
    #[inline(always)]
    fn find_last_in(self, haystack: &[Self], respect_case: bool) -> Option<usize> {
        haystack.iter().rposition(|&c| self.eq(c, respect_case))
    }

    /// Return the word-separator bonus for this character, or `0` if it is not
    /// a separator.  Uses a table lookup — a single bounds check replaces
    /// several branches and the returned value encodes both *whether* the
    /// character is a separator and *how much* bonus it carries.
    #[inline(always)]
    fn separator_bonus(self) -> Score {
        let ch = self.into() as usize;
        // For ch < 128 we do a table lookup; for ch >= 128 we return 0.
        // The `get` returns None for out-of-range, and `copied().unwrap_or(0)` is
        // typically compiled as a conditional move (branchless).
        SEPARATOR_TABLE.get(ch).copied().unwrap_or(0)
    }
}

impl Atom for u8 {
    #[inline(always)]
    fn eq_ignore_case(self, b: Self) -> bool {
        self.eq_ignore_ascii_case(&b)
    }
    #[inline(always)]
    fn is_lowercase(self) -> bool {
        self.is_ascii_lowercase()
    }

    /// Case-sensitive search uses SIMD-backed `memchr`; case-insensitive
    /// falls back to the generic scalar loop.
    #[inline(always)]
    fn find_first_in(self, haystack: &[Self], respect_case: bool) -> Option<usize> {
        if respect_case {
            memchr(self, haystack)
        } else {
            // Case-insensitive: compare lowercase. Also try the uppercase variant
            // so a single `memchr` can be used for each case variant.
            let lo = self.to_ascii_lowercase();
            let hi = self.to_ascii_uppercase();
            if lo == hi {
                // No case distinction for this byte (digit, symbol, etc.).
                memchr(lo, haystack)
            } else {
                // Check both variants and return the earliest occurrence.
                let p_lo = memchr(lo, haystack);
                let p_hi = memchr(hi, haystack);
                match (p_lo, p_hi) {
                    (None, x) | (x, None) => x,
                    (Some(a), Some(b)) => Some(a.min(b)),
                }
            }
        }
    }

    /// Case-sensitive backward search uses SIMD-backed `memrchr`.
    #[inline(always)]
    fn find_last_in(self, haystack: &[Self], respect_case: bool) -> Option<usize> {
        if respect_case {
            memrchr(self, haystack)
        } else {
            let lo = self.to_ascii_lowercase();
            let hi = self.to_ascii_uppercase();
            if lo == hi {
                memrchr(lo, haystack)
            } else {
                // Return the rightmost occurrence across both case variants.
                let p_lo = memrchr(lo, haystack);
                let p_hi = memrchr(hi, haystack);
                match (p_lo, p_hi) {
                    (None, x) | (x, None) => x,
                    (Some(a), Some(b)) => Some(a.max(b)),
                }
            }
        }
    }
}
impl Atom for char {
    #[inline(always)]
    fn eq_ignore_case(self, b: Self) -> bool {
        self.to_lowercase().eq(b.to_lowercase())
    }
    #[inline(always)]
    fn is_lowercase(self) -> bool {
        self.is_lowercase()
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn u8_find_first_case_sensitive() {
        let hay = b"abcABC";
        // Case-sensitive uses memchr directly.
        assert_eq!(b'A'.find_first_in(hay, true), Some(3));
        assert_eq!(b'z'.find_first_in(hay, true), None);
    }

    #[test]
    fn u8_find_first_case_insensitive_letter() {
        let hay = b"xxABCxx";
        // Case-insensitive letter checks both variants and returns the earliest.
        assert_eq!(b'a'.find_first_in(hay, false), Some(2));
        assert_eq!(b'C'.find_first_in(hay, false), Some(4));
    }

    #[test]
    fn u8_find_first_case_insensitive_digit() {
        let hay = b"a1b2";
        // Digits have no case distinction → single memchr branch.
        assert_eq!(b'2'.find_first_in(hay, false), Some(3));
        assert_eq!(b'9'.find_first_in(hay, false), None);
    }

    #[test]
    fn u8_find_first_only_one_case_present() {
        let hay = b"hello"; // only lowercase present
        // Uppercase query, only lowercase in haystack → (Some, None) arm.
        assert_eq!(b'L'.find_first_in(hay, false), Some(2));
    }

    #[test]
    fn u8_find_last_case_variants() {
        let hay = b"aAbA";
        // Case-sensitive backward search.
        assert_eq!(b'A'.find_last_in(hay, true), Some(3));
        // Case-insensitive returns the rightmost across both variants.
        assert_eq!(b'a'.find_last_in(hay, false), Some(3));
    }

    #[test]
    fn u8_find_last_case_insensitive_digit_and_single_case() {
        let hay = b"1a1";
        // Digit: no case distinction.
        assert_eq!(b'1'.find_last_in(hay, false), Some(2));
        // Only lowercase present, uppercase query → (None, Some)/(Some, None) arm.
        assert_eq!(b'A'.find_last_in(hay, false), Some(1));
    }

    #[test]
    fn char_atom_eq_and_case() {
        assert!('a'.eq('A', false));
        assert!(!'a'.eq('A', true));
        assert!('a'.is_lowercase());
        assert!(!'A'.is_lowercase());
        // Default (non-SIMD) find impls for char.
        let hay: Vec<char> = "abAB".chars().collect();
        assert_eq!('A'.find_first_in(&hay, true), Some(2));
        assert_eq!('a'.find_last_in(&hay, false), Some(2));
    }
}
