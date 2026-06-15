use std::sync::atomic::{AtomicBool, Ordering};

pub type Pos = (i16, i16);

/// Bitset word backing one grid row / one line track. The grid is `GRID × GRID`
/// with `GRID = Row::BITS`, so resizing the grid is just changing this alias:
/// `u64` → 64×64, `u128` → 128×128. A larger grid would need a `Row` newtype
/// over `[u128; k]` implementing the handful of bit ops used here (`Shl`, `Shr`,
/// `BitAnd`, `BitOr`, `Not`, `trailing_zeros`, `== 0`) — there is no primitive
/// `u256`. Everything else derives from `Row`, so nothing else changes.
pub type Row = u128;

/// Side length of the fixed square grid = number of bits in `Row`.
pub const GRID: i16 = Row::BITS as i16;
const N: usize = GRID as usize;

/// Internal-coordinate origin → grid index, centring the initial cross near the
/// grid centre (`GRID/2`). Derived from `GRID` so it tracks the alias.
pub(crate) const OFFSET: i16 = GRID / 2 - 5;

/// No cell may be placed within this many cells of the edge: a window queried by
/// `legal_moves` extends at most `n − 1 = 4` cells past an occupied (interior)
/// cell, so every `contains`/`row` query stays in `[0, GRID)` without a bounds
/// check.
const MARGIN: i16 = 4;

/// Set when a move would fall outside the fixed grid. Rather than crash, the
/// search polls this flag to stop and save the game gracefully (then the grid
/// can be enlarged by widening `Row`). The consumer resets it.
pub static GRID_OVERFLOW: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone)]
pub struct Board {
    /// Occupied cells in placement order (initial cross first, then moves).
    /// This is the iteration source for move generation; `apply`/`undo`
    /// push/pop here, matching the search's LIFO discipline.
    pub cells: Vec<Pos>,
    /// `GRID` rows of `Row`: bit `gx` of row `gy` set ⇔ cell `(gx, gy)` occupied.
    grid: Box<[Row; N]>,
}

impl Default for Board {
    fn default() -> Self {
        Self {
            cells: Vec::new(),
            grid: Box::new([0 as Row; N]),
        }
    }
}

impl Board {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn contains(&self, pos: Pos) -> bool {
        let gx = (pos.0 + OFFSET) as u32;
        let gy = (pos.1 + OFFSET) as usize;
        (self.grid[gy] >> gx) & 1 != 0
    }

    /// Occupancy of grid row `gy` as a `Row` (bit `gx` = cell occupied).
    /// Building block for the bitboard move generator (`moves::append_dir`).
    #[inline]
    pub(crate) fn row(&self, gy: usize) -> Row {
        self.grid[gy]
    }

    /// Place a point. Returns `false` *without writing* if it would fall outside
    /// the grid margin, setting [`GRID_OVERFLOW`]; the caller must then stop and
    /// save instead of continuing (the state is left unchanged).
    pub fn insert(&mut self, pos: Pos) -> bool {
        let (gx, gy) = (pos.0 + OFFSET, pos.1 + OFFSET);
        if !(MARGIN..GRID - MARGIN).contains(&gx) || !(MARGIN..GRID - MARGIN).contains(&gy) {
            GRID_OVERFLOW.store(true, Ordering::Relaxed);
            return false;
        }
        self.grid[gy as usize] |= (1 as Row) << (gx as u32);
        self.cells.push(pos);
        true
    }

    pub fn remove(&mut self, pos: Pos) {
        let gx = (pos.0 + OFFSET) as u32;
        let gy = (pos.1 + OFFSET) as usize;
        self.grid[gy] &= !((1 as Row) << gx);
        // Undo is strictly LIFO: the removed cell is always the last placed.
        let popped = self.cells.pop();
        debug_assert_eq!(popped, Some(pos), "board remove out of order");
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_contains_remove_with_negative_coords() {
        let mut b = Board::new();
        // Coordinates span both signs, as real games do once they grow.
        let pts = [(0i16, 0i16), (-30, 12), (40, -25), (-50, -50), (55, 55)];
        for &p in &pts {
            assert!(!b.contains(p));
            assert!(b.insert(p));
            assert!(b.contains(p));
        }
        assert_eq!(b.len(), pts.len());
        // remove is LIFO (undo discipline): pop in reverse order.
        for &p in pts.iter().rev() {
            assert!(b.contains(p));
            b.remove(p);
            assert!(!b.contains(p));
        }
        assert!(b.is_empty());
    }

    #[test]
    fn insert_past_margin_signals_overflow_without_panicking() {
        GRID_OVERFLOW.store(false, Ordering::Relaxed);
        let mut b = Board::new();
        // Far outside the interior — must report overflow, not write, not panic.
        assert!(!b.insert((10_000, 0)));
        assert!(GRID_OVERFLOW.load(Ordering::Relaxed));
        assert!(b.is_empty());
        GRID_OVERFLOW.store(false, Ordering::Relaxed);
    }
}
