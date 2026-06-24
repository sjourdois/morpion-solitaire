//! Feature-space NRPA — the adaptive head θ over the net's frozen penultimate
//! features φ(s,m).
//!
//! Instead of a frozen scalar bias `β = scale·net(s,m)`, freeze the net's penultimate
//! representation φ(s,m) (the `[h]`-dim activation) and adapt a linear head θ over it
//! *online during the search*: the per-move logit contribution becomes `θ·φ`, and one
//! adapt update `θ += α_θ(φ_chosen − Σ p_m φ_m)` moves **every** move sharing features —
//! the in-search generalization a one-hot policy table cannot give. When `φ = one-hot`
//! this reduces exactly to plain NRPA, so it is a strict generalization.
//!
//! The default is the **φ-B** configuration: θ·φ rides *alongside* the one-hot policy
//! table (both adapt). The frozen net is shared (the armed prior); θ and the φ cache are
//! per-island (thread-local, generation-invalidated like the β cache). It is off by
//! default — the `feat-adapt` option (and a prior armed) turns it on; every other path
//! stays byte-for-byte unchanged.
//!
//! The primary levers are `feat-adapt` (the switch) and `feat-alpha` (the head step,
//! smaller than the table's α=1 because φ has norm ≫ 1). Five further knobs reshape the
//! adaptation regime and are exposed as **experimental** advanced options (validated
//! defaults reproduce φ-B exactly, so leaving them be is the φ-B path):
//!   - `feat-table` (default on) — keep the one-hot table alongside θ·φ (φ-B); off ⇒
//!     the head-only **φ-A** (θ·φ is the whole policy logit, no table).
//!   - `feat-warm` (default on) — warm init θ₀ = scale·head reproduces the frozen prior
//!     at step 0; off ⇒ cold θ₀ = 0 (pure generalization).
//!   - `feat-lambda` (default 0 = off) — L2 decay θ ← (1−λ)θ after each adapt.
//!   - `feat-clamp` (default 0 = off) — clamp |θ_j| ≤ C after each adapt.
//!   - `feat-norm` (default off) — L2-normalize each cached φ to unit length (φ has
//!     norm ≫ 1, which makes θ·φ touchy). Breaks the warm-reproduction property, so it
//!     is meant for cold-init / pure-generalization sweeps.

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
/// generation-tagged) and the adaptive head θ. The adaptation knobs (`alpha`/`lambda`/
/// `clamp`/`norm`) are resolved once at restart so the hot loop takes no registry lock.
struct FeatState {
    generation: u64,
    phi: FxHashMap<PatchKey, Vec<f32>>,
    theta: Vec<f64>,
    alpha: f64,
    /// L2 decay λ (0 = off): θ ← (1−λ)θ after each adapt.
    lambda: f64,
    /// Head clamp C (`None` = off): |θ_j| ≤ C after each adapt.
    clamp: Option<f64>,
    /// L2-normalize each cached φ to unit length (default off).
    norm: bool,
}

thread_local! {
    static FEAT: RefCell<FeatState> = RefCell::new(FeatState {
        generation: 0,
        phi: FxHashMap::default(),
        theta: Vec::new(),
        alpha: DEFAULT_FEAT_ALPHA,
        lambda: 0.0,
        clamp: None,
        norm: false,
    });
}

/// Is feature-space adaptation live (the `feat-adapt` option AND a prior armed)?
pub fn active() -> bool {
    registry().value_bool("feat-adapt", false) && is_armed()
}

/// Does φ-B keep the one-hot policy table alongside θ·φ (`feat-table`, default on)? Off
/// ⇒ the head-only φ-A (θ·φ is the whole logit). Read by the NRPA hot loop, which drops
/// the table lookup & update when this is false.
pub fn keep_table() -> bool {
    registry().value_bool("feat-table", true)
}

#[inline]
fn dot(theta: &[f64], phi: &[f32]) -> f64 {
    theta.iter().zip(phi).map(|(&t, &p)| t * p as f64).sum()
}

/// Re-seed θ at an island restart (fresh policy ⇒ fresh θ) and resolve the adaptation
/// knobs into thread-local state (so the hot loop takes no registry lock); drops a stale
/// φ cache (prior changed). Warm init θ₀ = scale·head reproduces the frozen prior at step
/// 0; cold (`feat-warm` off) sets θ₀ = 0. No-op unless active.
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
    // Warm θ₀ = scale·l3.weight (reproduces the frozen prior); cold (or a shape
    // mismatch on the warm read) falls back to θ₀ = 0.
    let theta = if reg.value_bool("feat-warm", true) {
        let w = src.warm_theta(scale);
        if w.len() == h {
            w
        } else {
            vec![0.0; h]
        }
    } else {
        vec![0.0; h]
    };
    let alpha = reg.value_f64("feat-alpha", DEFAULT_FEAT_ALPHA);
    let lambda = reg.value_f64("feat-lambda", 0.0).max(0.0);
    let clamp = {
        let c = reg.value_f64("feat-clamp", 0.0);
        (c > 0.0).then_some(c)
    };
    let norm = reg.value_bool("feat-norm", false);
    let gen = GENERATION.load(Ordering::Relaxed);
    FEAT.with(|f| {
        let mut f = f.borrow_mut();
        if f.generation != gen {
            f.phi.clear();
            f.generation = gen;
        }
        f.theta = theta;
        f.alpha = alpha;
        f.lambda = lambda;
        f.clamp = clamp;
        f.norm = norm;
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
            let norm = f.norm;
            for (k, mut v) in miss_keys.iter().zip(vs) {
                if norm {
                    normalize_phi(&mut v); // once per pattern; identity when off
                }
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
        let (alpha, lambda, clamp) = (f.alpha, f.lambda, f.clamp);
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
        // θ += α·grad, then optional L2 decay (1−λ) and clamp to ±C.
        for (t, &g) in f.theta.iter_mut().zip(grad.iter()) {
            let mut v = *t + alpha * g;
            if lambda > 0.0 {
                v *= 1.0 - lambda;
            }
            if let Some(c) = clamp {
                v = v.clamp(-c, c);
            }
            *t = v;
        }
    });
}

/// L2-normalize φ in place to unit length (`feat-norm`). Applied once per cached pattern
/// at first sight; an all-zero φ is left untouched.
fn normalize_phi(v: &mut [f32]) {
    let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 1e-12 {
        for x in v.iter_mut() {
            *x /= n;
        }
    }
}
