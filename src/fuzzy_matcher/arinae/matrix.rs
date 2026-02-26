//! Base structs for the matching algorithm: Cell & SWMatrix

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
        Cell((score as u16 as u32) | ((dir as u32) << 16))
    }
    #[inline(always)]
    pub(super) fn score(self) -> Score {
        self.0 as u16 as i16
    }
    #[inline(always)]
    pub(super) fn dir(self) -> Dir {
        // SAFETY: Dir has repr(u8) with values 0..=3 and we only ever store
        // valid Dir values in bits 16-17.
        unsafe { std::mem::transmute((self.0 >> 16) as u8 & 0x3) }
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
