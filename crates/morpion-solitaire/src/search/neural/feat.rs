//! Feature-space NRPA (`docs/feature-space-nrpa.md`) — the adaptive head θ over the
//! net's frozen penultimate features φ(s,m).
//!
//! Instead of a frozen scalar bias `β = scale·net(s,m)`, freeze the net's penultimate
//! representation φ(s,m) (the `[h]`-dim activation) and adapt a linear head θ over it
//! *online during the search*: the per-move logit contribution becomes `θ·φ`, and one
//! adapt update `θ += α_θ(φ_chosen − Σ p_m φ_m)` moves **every** move sharing features —
//! the in-search generalization a one-hot policy table cannot give. When `φ = one-hot`
//! this reduces exactly to plain NRPA, so it is a strict generalization.
//!
//! This is the **φ-B** configuration: θ·φ rides *alongside* the one-hot policy table
//! (both adapt). The frozen net is shared (the armed prior); θ and the φ cache are
//! per-island (thread-local, generation-invalidated like the β cache). It is off by
//! default — the `feat-adapt` option (and a prior armed) turns it on; every other path
//! stays byte-for-byte unchanged.
//!
//! Two levers are exposed: `feat-adapt` (the switch) and `feat-alpha` (the head step,
//! smaller than the table's α=1 because φ has norm ≫ 1). The niche knobs the research
//! archive carried (φ-A head-only, cold init, L2 decay, head clamp, φ normalization)
//! are fixed at their validated defaults here (table kept, warm init, no decay/clamp).

use std::cell::RefCell;
use std::sync::atomic::Ordering;

use rustc_hash::FxHashMap;

use crate::game::{moves::Move, state::GameState};
use crate::search::plugin::registry;

use super::features::PatchKey;
use super::net::FeatureSource;
use super::{armed, is_armed, DEFAULT_SCALE, GENERATION};

/// Default head step size α_θ (smaller than the table's α=1: φ has norm ≫ 1).
pub const DEFAULT_FEAT_ALPHA: f64 = 0.1;

/// Per-island feature-space state: the φ cache (penultimate per local pattern,
/// generation-tagged) and the adaptive head θ; the head step resolved at restart.
struct FeatState {
    generation: u64,
    phi: FxHashMap<PatchKey, Vec<f32>>,
    theta: Vec<f64>,
    alpha: f64,
}

thread_local! {
    static FEAT: RefCell<FeatState> = RefCell::new(FeatState {
        generation: 0,
        phi: FxHashMap::default(),
        theta: Vec::new(),
        alpha: DEFAULT_FEAT_ALPHA,
    });
}

/// Is feature-space adaptation live (the `feat-adapt` option AND a prior armed)?
pub fn active() -> bool {
    registry().value_bool("feat-adapt", false) && is_armed()
}

#[inline]
fn dot(theta: &[f64], phi: &[f32]) -> f64 {
    theta.iter().zip(phi).map(|(&t, &p)| t * p as f64).sum()
}

/// Re-seed θ at an island restart (fresh policy ⇒ fresh θ): warm init
/// θ₀ = scale·head reproduces the frozen prior at step 0. Resolves the head step into
/// thread-local state and drops a stale φ cache (prior changed). No-op unless active.
pub fn restart() {
    if !active() {
        return;
    }
    let guard = armed().read().unwrap();
    let Some(prior) = guard.as_ref() else {
        return;
    };
    let src: &dyn FeatureSource = prior.as_ref();
    let h = src.feat_dim();
    let reg = registry();
    let scale = reg.value_f64("neural-scale", DEFAULT_SCALE);
    // Warm θ₀ = scale·l3.weight (reproduces the frozen prior); fall back to cold on a
    // shape mismatch.
    let theta = {
        let w = src.warm_theta(scale);
        if w.len() == h {
            w
        } else {
            vec![0.0; h]
        }
    };
    let alpha = reg.value_f64("feat-alpha", DEFAULT_FEAT_ALPHA);
    let gen = GENERATION.load(Ordering::Relaxed);
    FEAT.with(|f| {
        let mut f = f.borrow_mut();
        if f.generation != gen {
            f.phi.clear();
            f.generation = gen;
        }
        f.theta = theta;
        f.alpha = alpha;
    });
}

/// Fill `out` with the adaptive logit θ·φ(s,m) for each move (in order). φ is the
/// frozen net's penultimate, cached per local pattern (one batched forward over cache
/// misses); θ is this island's head.
pub fn logits(scratch: &GameState, moves: &[Move], out: &mut Vec<f64>) {
    out.clear();
    let guard = armed().read().unwrap();
    let Some(prior) = guard.as_ref() else {
        out.resize(moves.len(), 0.0);
        return;
    };
    let src: &dyn FeatureSource = prior.as_ref();
    let gen = GENERATION.load(Ordering::Relaxed);

    let mut keys: Vec<PatchKey> = Vec::with_capacity(moves.len());
    let mut miss_keys: Vec<PatchKey> = Vec::new();
    let mut miss_inputs: Vec<Vec<f32>> = Vec::new();
    FEAT.with(|f| {
        let mut f = f.borrow_mut();
        if f.generation != gen {
            f.phi.clear();
            f.generation = gen;
        }
        for mv in moves {
            let (key, input) = src.key_and_input(scratch, mv);
            if !f.phi.contains_key(&key) {
                miss_keys.push(key);
                miss_inputs.push(input);
            }
            keys.push(key);
        }
    });

    if !miss_inputs.is_empty() {
        let vs = src.compute_features(&miss_inputs); // φ for misses (one forward)
        FEAT.with(|f| {
            let mut f = f.borrow_mut();
            for (k, v) in miss_keys.iter().zip(vs) {
                f.phi.insert(*k, v);
            }
        });
    }

    FEAT.with(|f| {
        let f = f.borrow();
        let theta = &f.theta;
        out.extend(keys.iter().map(|k| match f.phi.get(k) {
            Some(phi) => dot(theta, phi),
            None => 0.0,
        }));
    });
}

/// Apply the head update θ += α·(φ_chosen − Σ_m p_m·φ_m). `probs` are the softmax
/// probabilities the step was sampled with (in `moves` order); every φ is read from
/// this island's cache (filled by the [`logits`] call that produced `probs`).
pub fn adapt(scratch: &GameState, moves: &[Move], chosen: &Move, probs: &[f64]) {
    if moves.is_empty() {
        return;
    }
    let guard = armed().read().unwrap();
    let Some(prior) = guard.as_ref() else {
        return;
    };
    let src: &dyn FeatureSource = prior.as_ref();
    let (ckey, _) = src.key_and_input(scratch, chosen);
    FEAT.with(|f| {
        let mut f = f.borrow_mut();
        let h = f.theta.len();
        if h == 0 {
            return;
        }
        let alpha = f.alpha;
        // grad = φ_chosen − Σ p_m φ_m
        let mut grad = vec![0.0f64; h];
        if let Some(phi_c) = f.phi.get(&ckey) {
            for (g, &p) in grad.iter_mut().zip(phi_c.iter()) {
                *g += p as f64;
            }
        }
        for (mv, &prob) in moves.iter().zip(probs) {
            let (k, _) = src.key_and_input(scratch, mv);
            if let Some(phi) = f.phi.get(&k) {
                for (g, &p) in grad.iter_mut().zip(phi.iter()) {
                    *g -= prob * p as f64;
                }
            }
        }
        for (t, &g) in f.theta.iter_mut().zip(grad.iter()) {
            *t += alpha * g;
        }
    });
}
