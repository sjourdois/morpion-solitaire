//! Whole-position encoding for the **value** net (policy+value / PUCT line).
//!
//! Unlike [`super::features`] (which encodes a single candidate *move* by its local
//! patch), the value net judges a whole *position* — "how long a 5T game can this
//! still become" — so it needs a board-level view. We crop a fixed window of the
//! occupancy grid centred on the cross and feed it as one occupancy plane plus a
//! few global scalars.
//!
//! The window `x, y ∈ [LO, HI]` with `LO + HI = K` (= 2·len−1 = 9 for 5T) is exactly
//! the symmetry centre's invariant range, so the eight D4 transforms
//! ([`apply_transform`]) map it onto itself — letting us **augment** value training
//! with all eight orientations (the value of a position equals the value of its
//! mirror). The span comfortably covers every known 5T game (the record reaches
//! ~19–20 cells from centre; this window reaches 20/29).

use crate::game::rules::Variant;
use crate::game::state::GameState;
use crate::search::symmetry::apply_transform;

/// Window bounds (internal coords). `LO + HI = 9` keeps it D4-invariant for 5T.
pub const LO: i16 = -20;
pub const HI: i16 = 29;
/// Window side and cell count.
pub const PSIDE: usize = (HI - LO + 1) as usize; // 50
pub const PCELLS: usize = PSIDE * PSIDE; // 2500
/// Value feature length: the occupancy plane plus a few global scalars.
pub const VALUE_LEN: usize = PCELLS + 3;

#[inline]
fn pindex(x: i16, y: i16) -> usize {
    (x - LO) as usize * PSIDE + (y - LO) as usize
}

/// Encode `state` as a value feature vector under D4 transform `t` (0 = natural).
/// The occupancy plane carries every placed cell (cross + moves); the scalars are
/// game progress, and the bounding-box width/height (normalised) — cheap global
/// shape cues. `t` is used only for training-time augmentation.
pub fn encode_value(state: &GameState, t: usize) -> Vec<f32> {
    let k = 2 * state.variant.len() as i16 - 1; // 9 for 5T
    let mut plane = vec![0f32; PCELLS];
    let (mut minx, mut miny, mut maxx, mut maxy) = (i16::MAX, i16::MAX, i16::MIN, i16::MIN);
    for &cell in &state.board.cells {
        let (x, y) = apply_transform(t, cell, k);
        if (LO..=HI).contains(&x) && (LO..=HI).contains(&y) {
            plane[pindex(x, y)] = 1.0;
        }
        minx = minx.min(x);
        miny = miny.min(y);
        maxx = maxx.max(x);
        maxy = maxy.max(y);
    }
    let mut f = plane;
    f.push(state.history.len() as f32 / 200.0);
    let (w, h) = if state.board.cells.is_empty() {
        (0.0, 0.0)
    } else {
        ((maxx - minx) as f32 / 60.0, (maxy - miny) as f32 / 60.0)
    };
    f.push(w);
    f.push(h);
    f
}

/// Natural-orientation value encoding.
pub fn encode_value_natural(state: &GameState) -> Vec<f32> {
    encode_value(state, 0)
}

/// The value target for a position: its game's final length, normalised by 200 so
/// the net's sigmoid output lives in ~[0.18, 0.9] for 5T. (A proxy for the true
/// max-achievable length — good enough for the learnability check; search-generated
/// best-completion labels come later.)
#[inline]
pub fn value_target(final_len: u32) -> f32 {
    final_len as f32 / 200.0
}

/// Convenience: is this a 5T position (the campaign variant)?
#[inline]
pub fn is_t5(variant: Variant) -> bool {
    variant == Variant::T5
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::moves::legal_moves;

    #[test]
    fn window_is_d4_invariant_and_holds_the_cross() {
        // The initial cross must sit fully inside the window in every orientation.
        let st = GameState::new(Variant::T5);
        for t in 0..8 {
            let f = encode_value(&st, t);
            assert_eq!(f.len(), VALUE_LEN);
            let occ: f32 = f[..PCELLS].iter().sum();
            assert_eq!(
                occ as usize,
                st.board.cells.len(),
                "transform {t} lost cells"
            );
        }
    }

    #[test]
    fn occupancy_grows_with_moves() {
        let mut st = GameState::new(Variant::T5);
        let before: f32 = encode_value(&st, 0)[..PCELLS].iter().sum();
        for _ in 0..10 {
            let ms = legal_moves(&st);
            if ms.is_empty() {
                break;
            }
            st.apply(ms[0]);
        }
        let after: f32 = encode_value(&st, 0)[..PCELLS].iter().sum();
        assert!(after > before, "occupancy should grow as moves are played");
    }
}
