//! The initial cross — the standard starting position of Morpion Solitaire.
//!
//! Points use the same internal coordinate frame as a record's moves: moves
//! extend outward from the cross (often into negative coordinates). The cross is
//! the outline of a Greek cross with arms `n-1` cells wide, centred so the whole
//! figure is D4-symmetric. Symmetry requires the arm band `[a, b]` to be centred
//! (`a + b = w`), which is only possible when the arm width and the grid have
//! matching parity, so the grid is `2n-1` wide for odd `n` and `2n-2` for even
//! `n`. For `n = 5` this is the classic 36-point Greek cross; for `n = 4`, a
//! centred 24-point cross on a `0..=6` grid. (Must match
//! `morpion_solitaire::game::state::GameState::setup_cross`.)

use crate::model::Variant;

/// The points of the initial cross for `variant`, in row-major order.
pub fn initial_cross(variant: Variant) -> Vec<(i16, i16)> {
    let n = variant.line_len() as i16;
    let arm = n - 1; // arm width (cells)
    let w = if n % 2 == 1 { 2 * n - 1 } else { 2 * n - 2 }; // max coordinate index
    let a = (w - (arm - 1)) / 2; // inner arm boundary (band centred: a + b = w)
    let b = a + arm - 1; // outer arm boundary

    let mut pts = Vec::new();
    for x in 0..=w {
        for y in 0..=w {
            let in_cross = ((y == 0 || y == w) && (a..=b).contains(&x))
                || ((x == 0 || x == w) && (a..=b).contains(&y))
                || ((x == a || x == b) && (y <= a || y >= b))
                || ((y == a || y == b) && (x <= a || x >= b));
            if in_cross {
                pts.push((x, y));
            }
        }
    }
    pts
}
