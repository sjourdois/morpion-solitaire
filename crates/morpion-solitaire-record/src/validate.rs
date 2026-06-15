//! A pure reference validator: replay a record under the Morpion Solitaire rules
//! and confirm every move is legal.
//!
//! This is correctness-first, not speed-first: it uses a plain point set and a
//! linear scan of drawn lines, mirroring the rules directly rather than the
//! bit-parallel structures a solver would use. That makes it a trustworthy
//! reference for the MSR standard, independent of any particular engine.

use crate::cross::initial_cross;
use crate::model::{Direction, Record};
use std::collections::HashSet;

/// Why a record failed validation. Each variant carries the offending move's
/// index (0-based) in `record.moves`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// `pos` is outside `0..line_len`.
    BadLinePosition {
        /// Offending move index (0-based).
        index: usize,
        /// The out-of-range line position.
        pos: u8,
    },
    /// The new point is already occupied.
    PointOccupied {
        /// Offending move index (0-based).
        index: usize,
        /// The already-occupied point.
        point: (i16, i16),
    },
    /// A non-new point of the line is not present on the board.
    MissingPoint {
        /// Offending move index (0-based).
        index: usize,
        /// The missing line point.
        point: (i16, i16),
    },
    /// The line breaks the touch rule against an earlier collinear line.
    TouchRule {
        /// Offending move index (0-based).
        index: usize,
    },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::BadLinePosition { index, pos } => {
                write!(f, "move {}: line position {pos} out of range", index + 1)
            }
            ValidationError::PointOccupied { index, point } => {
                write!(f, "move {}: point {point:?} already occupied", index + 1)
            }
            ValidationError::MissingPoint { index, point } => {
                write!(f, "move {}: line point {point:?} is not present", index + 1)
            }
            ValidationError::TouchRule { index } => {
                write!(f, "move {}: violates the touch rule", index + 1)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Track key + position of a line, used for the touch rule. Two lines conflict
/// only if collinear (same direction and same track); `position` then measures
/// their offset along that track.
fn track_position(origin: (i16, i16), dir: Direction) -> (i16, i16) {
    let (x, y) = origin;
    match dir {
        Direction::H => (y, x),
        Direction::V => (x, y),
        Direction::DP => (x + y, x), // x + y constant along (1,-1)
        Direction::DN => (x - y, x), // x - y constant along (1,1)
    }
}

/// Validate `record`: replay its moves from the initial cross and check each is
/// a legal Morpion move. Returns the first violation, or `Ok(())`.
///
/// Legality of a move is: its `pos` is in range; the new point is currently
/// empty; the line's other `line_len - 1` points are already present; and the
/// line does not break the touch rule against any earlier collinear line. The
/// touch rule forbids two same-direction collinear lines whose track offset is
/// at most `line_len - 1 - max_overlap`, where `max_overlap` is 1 for touching
/// variants and 0 for disjoint ones.
pub fn validate(record: &Record) -> Result<(), ValidationError> {
    let n = record.variant.line_len();
    let max_overlap: i16 = if record.variant.disjoint() { 0 } else { 1 };
    let forbid = n as i16 - 1 - max_overlap;

    let mut points: HashSet<(i16, i16)> = initial_cross(record.variant).into_iter().collect();
    let mut lines: Vec<((i16, i16), Direction)> = Vec::with_capacity(record.moves.len());

    for (index, m) in record.moves.iter().enumerate() {
        if m.pos >= n {
            return Err(ValidationError::BadLinePosition { index, pos: m.pos });
        }
        let new_point = (m.x, m.y);
        if points.contains(&new_point) {
            return Err(ValidationError::PointOccupied {
                index,
                point: new_point,
            });
        }
        // Every other point of the line must already be present.
        for p in m.line_points(n) {
            if p != new_point && !points.contains(&p) {
                return Err(ValidationError::MissingPoint { index, point: p });
            }
        }
        // Touch rule against earlier collinear lines.
        let origin = m.origin();
        let (track, position) = track_position(origin, m.dir);
        for (lo, ld) in &lines {
            if *ld == m.dir {
                let (t2, p2) = track_position(*lo, *ld);
                if t2 == track && (position - p2).abs() <= forbid {
                    return Err(ValidationError::TouchRule { index });
                }
            }
        }
        points.insert(new_point);
        lines.push((origin, m.dir));
    }
    Ok(())
}
