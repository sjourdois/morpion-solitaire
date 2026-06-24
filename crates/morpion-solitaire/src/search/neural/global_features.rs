//! φ-D scaffold — a **global/spatial** feature source (INERT, not wired).
//!
//! φ-A/φ-B adapt a head over a **local** patch: the net's penultimate over a 9×9
//! window around the move. Anything the search needs to learn about *global* board
//! structure (overall shape, long-range interactions) is invisible to a local φ.
//! Generalizing the online adaptation over local features is therefore a lever, not a
//! ceiling-breaker on its own. The design mitigation is to make φ **pluggable** — the
//! [`super::net::FeatureSource`] trait, now in place — so a global encoder can feed the
//! **same** adapt rule `θ += α_θ(φ_chosen − Σ p_m φ_m)`. That is φ-D.
//!
//! This module is the **compilable proof that the trait is sufficient for φ-D**, and
//! the drop-in point. It is deliberately **inert**: the bodies are `unimplemented!()`
//! and nothing constructs or installs it. Building it for real is deferred until a
//! measurement justifies the investment (the project's *don't-assume-test* rule). When
//! that day comes:
//!
//! 1. Train a small whole-board encoder (a conv / occupancy crop over the
//!    D4-canonical board, reusing the encoder ideas already in
//!    [`super::position`]); its penultimate is `φ_global(s)` (per *state*) or
//!    `φ_global(s, m)` (per *move*, by encoding the post-move board).
//! 2. Fill the methods below: `key_and_input` keys by a **board hash** (the
//!    `PatchKey` slot is just an opaque 256-bit key, not necessarily a *patch*) and
//!    returns the board encoding; `compute_features` runs the encoder.
//! 3. Install it where the search reads its φ source — today the armed `NeuralPrior`
//!    in [`super::feat`]; φ-D generalises that slot to any `FeatureSource` (a small
//!    follow-up: hold `Arc<dyn FeatureSource>` there).
//!
//! This would also **unify with the value line**: a whole-board encoder is exactly the
//! representation [`super::position`] / the value net use, so φ-D could share it.

#![allow(dead_code)] // scaffold: defined to pin the φ-D seam, not yet constructed

use super::features::PatchKey;
use super::net::FeatureSource;
use crate::game::{moves::Move, state::GameState};

/// A global/spatial φ source (φ-D). Holds a trained whole-board encoder (TBD) and
/// the feature dimension. Inert until built — see the module note.
pub struct GlobalConvFeatures {
    /// Feature dimension `d` (= |θ|) the head adapts over.
    dim: usize,
    // A trained whole-board encoder (e.g. a small conv over the D4-canonical
    // occupancy crop from `position.rs`) + its device would live here.
}

impl GlobalConvFeatures {
    /// Sketch: a φ-D cache key is a **board-state hash** (post-move occupancy folded
    /// to the canonical D4 frame), not a local patch — the `PatchKey` is just a
    /// 256-bit opaque key the cache happens to use.
    fn board_key(_state: &GameState) -> PatchKey {
        unimplemented!("φ-D: hash the post-move canonical board occupancy into a 256-bit key")
    }
}

impl FeatureSource for GlobalConvFeatures {
    fn feat_dim(&self) -> usize {
        self.dim
    }

    /// A global encoder has no *frozen per-move prior* to reproduce, so warm-start is
    /// a cold θ₀ = 0 (pure generalization). (φ-B's warm init only makes sense for the
    /// net's own l3 head.)
    fn warm_theta(&self, _scale: f64) -> Vec<f64> {
        vec![0.0; self.dim]
    }

    fn key_and_input(&self, _scratch: &GameState, _mv: &Move) -> (PatchKey, Vec<f32>) {
        unimplemented!("φ-D: encode the whole board after `mv`; key by board hash")
    }

    fn compute_features(&self, _inputs: &[Vec<f32>]) -> Vec<Vec<f32>> {
        unimplemented!("φ-D: run the whole-board conv encoder over the board encodings")
    }
}
