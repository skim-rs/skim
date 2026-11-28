//! Base structs for the matching algorithm: Cell & `SWMatrix`

use super::Score;

/// Direction the optimal path took to reach a cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[allow(dead_code)] // variants are constructed via transmute from bits
pub(super) enum Dir {
    /// No valid path (score == 0).
    ///
    /// Assigned tag 0 so that `Cell::new(0, Dir::None)` encodes as all-zero
    /// bits, allowing boundary rows/columns to be bulk-zeroed with
    /// `write_bytes(0)` instead of a scalar loop.
    None = 0,
    /// Diagonal: match or mismatch (came from [i-1][j-1])
    Diag = 1,
    /// Up: gap in choice (came from [i-1][j], skip pattern char)
    Up = 2,
    /// Left: gap in pattern (came from [i][j-1], skip choice char)
    Left = 3,
}

/// Packed cell stored as a `u32`: bits [15:0] = score (as u16 bitcast from
/// i16), bits [17:16] = direction tag.  This gives 4 bytes per cell with no
/// padding and enables branchless direction extraction via bitmask.
#[derive(Copy, Clone)]
pub(super) struct Cell(u32);

pub(super) const CELL_ZERO: Cell = Cell::new(0, Dir::None);

impl std::fmt::Debug for Cell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cell")
            .field("score", &self.score())
            .field("dir", &self.dir())
            .finish()
    }
}

impl Cell {
    #[inline(always)]
    pub(super) const fn new(score: Score, dir: Dir) -> Cell {
        // Store score as u16 bits in low 16 bits, dir in bits 16-17.
        Cell((score.cast_unsigned() as u32) | ((dir as u32) << 16))
    }
    #[inline(always)]
    pub(super) fn score(self) -> Score {
        // Truncation is intentional: low 16 bits store the score as a bitcast i16.
        #[allow(clippy::cast_possible_truncation)]
        let low16 = self.0 as u16;
        low16.cast_signed()
    }
    #[inline(always)]
    pub(super) fn dir(self) -> Dir {
        // SAFETY: Dir has repr(u8) with values 0..=3 and we only ever store
        // valid Dir values in bits 16-17. Truncation from u32 to u8 is intentional.
        #[allow(clippy::cast_possible_truncation)]
        let tag = (self.0 >> 16) as u8 & 0x3;
        unsafe { std::mem::transmute(tag) }
    }
    /// Branchless check: true when dir == Diag (tag 1).
    #[inline(always)]
    pub(super) fn is_diag(self) -> bool {
        (self.0 >> 16) & 0x3 == 1
    }
}

#[derive(Default, Debug)]
pub(super) struct SWMatrix {
    pub(super) data: Vec<Cell>,
    pub(super) cols: usize,
    pub(super) rows: usize,
}

impl SWMatrix {
    pub fn zero(rows: usize, cols: usize) -> Self {
        let mut res = SWMatrix::default();
        res.resize(rows, cols);
        res
    }
    pub fn resize(&mut self, rows: usize, cols: usize) {
        let needed = rows * cols;
        if needed > self.data.len() {
            self.data.resize(needed, CELL_ZERO);
        }
        self.rows = rows;
        self.cols = cols;
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn cell_packs_score_and_direction() {
        let cell = Cell::new(42, Dir::Diag);
        assert_eq!(cell.score(), 42);
        assert_eq!(cell.dir(), Dir::Diag);
        assert!(cell.is_diag());

        // Negative scores round-trip through the i16 bitcast.
        let neg = Cell::new(-7, Dir::Up);
        assert_eq!(neg.score(), -7);
        assert_eq!(neg.dir(), Dir::Up);
        assert!(!neg.is_diag());
    }

    #[test]
    fn cell_zero_is_none_direction() {
        assert_eq!(CELL_ZERO.score(), 0);
        assert_eq!(CELL_ZERO.dir(), Dir::None);
    }

    #[test]
    fn cell_debug_shows_score_and_dir() {
        let s = format!("{:?}", Cell::new(5, Dir::Left));
        assert!(s.contains("Cell"));
        assert!(s.contains("score"));
        assert!(s.contains("Left"));
    }

    #[test]
    fn matrix_zero_and_resize_grow() {
        let mut m = SWMatrix::zero(2, 3);
        assert_eq!(m.rows, 2);
        assert_eq!(m.cols, 3);
        assert!(m.data.len() >= 6);

        // Growing increases the backing storage.
        m.resize(4, 4);
        assert_eq!(m.rows, 4);
        assert_eq!(m.cols, 4);
        assert!(m.data.len() >= 16);

        // Shrinking keeps the (larger) allocation but updates dims.
        m.resize(1, 1);
        assert_eq!(m.rows, 1);
        assert_eq!(m.cols, 1);
    }
}
