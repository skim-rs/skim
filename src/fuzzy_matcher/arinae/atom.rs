//! Byte/Char helpers
use memchr::memchr;
/// 128-byte lookup table for separator detection. A byte is 1 if the
/// corresponding ASCII codepoint is a word separator, 0 otherwise.
/// Non-ASCII (>= 128) is handled by the bounds check in `is_separator`.
static SEPARATOR_TABLE: [u8; 128] = {
    let mut t = [0u8; 128];
    t[b' ' as usize] = 1;
    t[b'-' as usize] = 1;
    t[b'.' as usize] = 1;
    t[b'/' as usize] = 1;
    t[b'\\' as usize] = 1;
    t[b'_' as usize] = 1;
    t
};

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
    #[inline]
    fn find_first_in(self, haystack: &[Self], respect_case: bool) -> Option<usize> {
        haystack.iter().position(|&c| self.eq(c, respect_case))
    }
    /// Check if a character is a word separator for bonus computation.
    /// Uses a table lookup — a single bounds check replaces three branches.
    #[inline(always)]
    fn is_separator(self) -> bool {
        let ch = self.into() as u32;
        // For ch < 128 we do a table lookup; for ch >= 128 we return false.
        // The `get` returns None for out-of-range, and `copied().unwrap_or(0)` is
        // typically compiled as a conditional move (branchless).
        SEPARATOR_TABLE.get(ch as usize).copied().unwrap_or(0) != 0
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
    #[inline]
    fn find_first_in(self, haystack: &[Self], respect_case: bool) -> Option<usize> {
        if respect_case {
            // SAFETY: `self` is a u8 and memchr searches for it in a byte slice.
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
}
impl Atom for char {
    #[inline(always)]
    fn eq_ignore_case(self, b: Self) -> bool {
        self.to_lowercase().eq(b.to_lowercase())
    }
    #[inline(always)]
    fn is_lowercase(self) -> bool {
        self.is_ascii_lowercase()
    }
}
