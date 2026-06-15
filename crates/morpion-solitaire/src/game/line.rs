use super::board::Pos;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Dir {
    H,  // horizontal → (dx=1, dy=0)
    V,  // vertical ↓   (dx=0, dy=1)
    DP, // diagonal ↗   (dx=1, dy=-1)
    DN, // diagonal ↘   (dx=1, dy=+1)
}

impl Dir {
    pub const ALL: [Dir; 4] = [Dir::H, Dir::V, Dir::DP, Dir::DN];

    pub fn delta(self) -> (i16, i16) {
        match self {
            Dir::H => (1, 0),
            Dir::V => (0, 1),
            Dir::DP => (1, -1),
            Dir::DN => (1, 1),
        }
    }
}

/// A line identified by its canonical start point and direction.
/// The origin is always the end with the smaller coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Line {
    pub origin: Pos,
    pub dir: Dir,
}

impl Line {
    pub fn new(origin: Pos, dir: Dir) -> Self {
        Self { origin, dir }
    }

    /// Build a line from any point on it (at the given offset within the line).
    pub fn from_point(pos: Pos, dir: Dir, offset: u8, _line_len: u8) -> Self {
        let (dx, dy) = dir.delta();
        let origin = (pos.0 - offset as i16 * dx, pos.1 - offset as i16 * dy);
        Self { origin, dir }
    }

    /// Iterate over all positions on this line.
    pub fn positions(self, line_len: u8) -> impl Iterator<Item = Pos> {
        let (dx, dy) = self.dir.delta();
        let (ox, oy) = self.origin;
        (0..line_len as i16).map(move |i| (ox + i * dx, oy + i * dy))
    }
}
