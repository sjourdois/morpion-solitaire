//! Conflict index for drawn lines, kept as fixed-grid bitsets.
//!
//! Two parallel lines can only conflict under the T/D touch rule if they lie on
//! the same *track* (are collinear). We group drawn lines by direction and
//! track, so checking a candidate inspects only its own track instead of
//! scanning every line drawn so far (O(1) vs O(L)).
//!
//! Track and position follow the same decomposition the old scan used:
//!
//! | Dir | track key | position |
//! |-----|-----------|----------|
//! | H   | `y`       | `x`      |
//! | V   | `x`       | `y`      |
//! | DP  | `x + y`   | `x`      |
//! | DN  | `x − y`   | `x`      |
//!
//! With `GRID = 128`, a track holds at most 128 positions, so each track is a
//! single `u128`: the conflict window becomes one bit-mask test, and the exact
//! duplicate check is subsumed (a same-origin line sets the bit at its own
//! position, i.e. distance 0). Line origins live in the same fixed grid as the
//! board, so this never overflows for a game the board itself accepts.

use super::board::{Row, GRID, OFFSET};
use super::line::{Dir, Line};

/// Positions per track, and number of tracks for the axis-aligned directions.
const G: usize = GRID as usize;
/// Tracks for the diagonals, whose key (`x ± y`) ranges over twice the grid.
const DIAG: usize = 2 * G;

#[derive(Clone)]
pub struct LineIndex {
    h: Box<[Row; G]>,
    v: Box<[Row; G]>,
    dp: Box<[Row; DIAG]>,
    dn: Box<[Row; DIAG]>,
}

impl LineIndex {
    pub fn new() -> Self {
        Self {
            h: Box::new([0 as Row; G]),
            v: Box::new([0 as Row; G]),
            dp: Box::new([0 as Row; DIAG]),
            dn: Box::new([0 as Row; DIAG]),
        }
    }

    /// Grid coordinates of a line's origin.
    #[inline]
    fn grid(line: &Line) -> (usize, usize) {
        (
            (line.origin.0 + OFFSET) as usize,
            (line.origin.1 + OFFSET) as usize,
        )
    }

    /// Mutable track word + position for `line`.
    #[inline]
    fn slot_mut(&mut self, line: &Line) -> (&mut Row, u8) {
        let (gx, gy) = Self::grid(line);
        match line.dir {
            Dir::H => (&mut self.h[gy], gx as u8),
            Dir::V => (&mut self.v[gx], gy as u8),
            Dir::DP => (&mut self.dp[gx + gy], gx as u8),
            Dir::DN => (&mut self.dn[gx + G - gy], gx as u8),
        }
    }

    /// Track word value + position for `line`.
    #[inline]
    fn slot(&self, line: &Line) -> (Row, u8) {
        let (gx, gy) = Self::grid(line);
        match line.dir {
            Dir::H => (self.h[gy], gx as u8),
            Dir::V => (self.v[gx], gy as u8),
            Dir::DP => (self.dp[gx + gy], gx as u8),
            Dir::DN => (self.dn[gx + G - gy], gx as u8),
        }
    }

    pub fn insert(&mut self, line: &Line) {
        let (word, pos) = self.slot_mut(line);
        *word |= (1 as Row) << pos;
    }

    pub fn remove(&mut self, line: &Line) {
        let (word, pos) = self.slot_mut(line);
        *word &= !((1 as Row) << pos);
    }

    /// Exact-duplicate test: is this exact line already drawn?
    pub fn contains(&self, line: &Line) -> bool {
        let (bits, pos) = self.slot(line);
        bits & ((1 as Row) << pos) != 0
    }

    /// Would drawing `line` violate the touch rule? `forbid` is the largest
    /// track distance that still conflicts: `W = line_len − max_overlap − 1`
    /// (3 for 5T, 4 for 5D, 2 for 4T, 3 for 4D). Any same-track line within
    /// `±forbid` positions conflicts; distance 0 is an exact duplicate.
    #[inline]
    pub fn conflicts(&self, line: &Line, forbid: u8) -> bool {
        let (bits, pos) = self.slot(line);
        bits & window_mask(pos, forbid) != 0
    }
}

impl Default for LineIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for LineIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LineIndex { .. }")
    }
}

/// Bit-mask of positions `[pos − forbid, pos + forbid]`, clamped to `[0, GRID−1]`.
#[inline]
fn window_mask(pos: u8, forbid: u8) -> Row {
    const MAXPOS: u8 = (GRID - 1) as u8;
    let lo = pos.saturating_sub(forbid);
    let hi = (pos as u16 + forbid as u16).min(MAXPOS as u16) as u8;
    let hi_mask = if hi >= MAXPOS {
        Row::MAX
    } else {
        ((1 as Row) << (hi + 1)) - 1
    };
    let lo_mask = if lo == 0 { 0 } else { ((1 as Row) << lo) - 1 };
    hi_mask & !lo_mask
}
