//! Neural move prior for NRPA — a learned move prior (NeuralNRPA-style), ported from
//! the `neural-guide` archive onto the plugin [`BiasModifier`] hook.
//!
//! NRPA samples moves by `softmax(w/τ + β)` (the GNRPA form), where `w` is the
//! online-adapted policy table and `β` a fixed per-move bias. The neural guide
//! supplies `β` from a small network trained on strong games: a [`NeuralPrior`]
//! armed via [`prior::arm`] becomes the registry's active [`BiasModifier`], so every
//! playout/adapt step adds the net's learned per-move bias. The search stays NRPA —
//! only the prior becomes *learned*.
//!
//! Feature-gated (`neural`, off by default — it pulls candle) and native-only
//! (inference targets CPU, the record hunt runs natively). This commit ports the
//! inference core (features, the policy net, the bundled prior) and the bias plugin;
//! training, PUCT and tabula-rasa land in later phase-5 commits.

pub mod dataset;
pub mod embedded;
pub mod feat;
pub mod features;
pub mod global_features;
pub mod net;
// The neural registry plugins (move prior, feature-space head, PUCT), co-located here
// with the engine they wire in. The framework itself lives in `search::plugin`.
pub mod plugin;
pub mod position;
pub mod puct;
pub mod selfplay;
pub mod tabula_rasa;
pub mod train;

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use rustc_hash::FxHashMap;

use crate::game::{moves::Move, state::GameState};
// The neural *plugins* (registration) live in the sibling `plugin` submodule; this
// module is the neural *engine* they wire in. The framework is `search::plugin`.
use crate::search::plugin::{registry, BiasModifier, OptionValue};
use crate::search::SearchState;
use features::PatchKey;
use net::{MovePrior, NeuralPrior, ValuePredictor};

/// Default neural-prior strength (β scale). The sweet spot measured on 5T ≈ 4.
pub const DEFAULT_SCALE: f64 = 4.0;

/// The prior armed for the next searches (shared across island threads). `None` ⇒ the
/// bias modifier is inactive and NRPA runs as plain (the hot loop takes its fast path).
fn armed() -> &'static RwLock<Option<Arc<NeuralPrior>>> {
    static A: OnceLock<RwLock<Option<Arc<NeuralPrior>>>> = OnceLock::new();
    A.get_or_init(|| RwLock::new(None))
}

/// Is a prior currently armed? (Read at search start to decide the hot-loop path.)
pub fn is_armed() -> bool {
    armed().read().unwrap().is_some()
}

/// Cache generation, bumped whenever the armed prior changes. Each thread's bias cache
/// (keyed by local pattern) carries the generation it was filled at and clears itself
/// when it falls behind — so a new prior (or disarm) never serves stale logits.
static GENERATION: AtomicU64 = AtomicU64::new(1);

/// Bump the cache generation — invalidates every thread's bias cache lazily.
fn bump_generation() {
    GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// Per-thread cache of the net's **raw** per-move logit, keyed by the move's local
/// pattern ([`PatchKey`]). The scale is applied at read time (so annealing it never
/// invalidates the cache), and the whole map is dropped when the prior changes (the
/// generation moves). Two `(state, move)` pairs with the same local pattern share an
/// entry — a lazy NN→table distillation: the net forward (the costly part) runs once
/// per distinct pattern instead of once per node. The pattern key ignores the two
/// global scalars (game progress, density), so a recurring pattern reuses the logit
/// from its first sighting — the small approximation the throughput win is worth.
struct BiasCache {
    generation: u64,
    map: FxHashMap<PatchKey, f64>,
}

thread_local! {
    static BIAS_CACHE: RefCell<BiasCache> = RefCell::new(BiasCache {
        generation: 0,
        map: FxHashMap::default(),
    });
}

/// Set the prior strength (β scale) for subsequent searches by writing the registry's
/// `neural-scale` option — the same value the [`NeuralBias`] reads. Used by tabula-rasa
/// to anneal the scale across Expert-Iteration rounds.
pub fn set_scale(scale: f64) {
    registry().set_value("neural-scale", OptionValue::Float(scale));
}

/// Restore the default prior strength (see [`set_scale`]).
pub fn reset_scale() {
    set_scale(DEFAULT_SCALE);
}

/// The currently armed prior as a shared handle (for methods that consume it directly,
/// e.g. PUCT's policy). `None` ⇒ no prior armed.
pub fn armed_arc() -> Option<Arc<NeuralPrior>> {
    armed().read().unwrap().clone()
}

/// The value net armed for PUCT's leaf evaluation. When set, PUCT uses
/// `LeafEval::Value` (the net's position estimate) instead of a rollout.
fn value_slot() -> &'static RwLock<Option<Arc<ValuePredictor>>> {
    static V: OnceLock<RwLock<Option<Arc<ValuePredictor>>>> = OnceLock::new();
    V.get_or_init(|| RwLock::new(None))
}

/// The currently armed value net (shared handle), or `None`.
pub fn armed_value() -> Option<Arc<ValuePredictor>> {
    value_slot().read().unwrap().clone()
}

/// Arm (or clear) the PUCT value net. Call before launching a PUCT search.
pub fn install_value(v: Option<ValuePredictor>) {
    *value_slot().write().unwrap() = v.map(Arc::new);
}

/// Load a value net saved by [`train_value_net`] (safetensors), on CPU.
pub fn load_value(path: &str) -> candle_core::Result<ValuePredictor> {
    ValuePredictor::load(path, candle_core::Device::Cpu)
}

/// Train a PUCT value net on CPU: generate length-varied games by self-play (uniform
/// rollouts plus prior-guided ones at several temperatures — the spread the value net
/// needs, since the record corpus is all long games), label each position with its
/// game's final length, and regress. `n` games per bucket; `prior` guides the rollouts.
pub fn train_value_net(
    variant: crate::game::rules::Variant,
    prior: Option<&NeuralPrior>,
    n: usize,
    epochs: usize,
) -> candle_core::Result<ValuePredictor> {
    use dataset::value_samples_from_games;
    use selfplay::varied_games;
    use train::{train_value, TrainConfig};
    // Inverse temperatures spanning short→long games (0 = uniform is added by varied_games).
    let temps = [0.5f64, 1.0, 2.0];
    let prior_dyn: Option<&dyn MovePrior> = prior.map(|p| p as &dyn MovePrior);
    let games = varied_games(variant, n, prior_dyn, &temps);
    let samples = value_samples_from_games(variant, &games, true);
    let (varmap, net) = train_value(
        &samples,
        &TrainConfig { epochs, lr: 1e-3 },
        256,
        candle_core::Device::Cpu,
    )?;
    Ok(ValuePredictor::new(net, varmap, candle_core::Device::Cpu))
}

// ---- PUCT method plugin ---------------------------------------------------

/// A zero-bias policy: every move equally likely. PUCT uses this when no neural prior
/// is armed — it then degrades to rollout-grounded MCTS, still a valid search.
struct UniformPrior;
impl MovePrior for UniformPrior {
    fn biases(&self, features: &[Vec<f32>]) -> Vec<f64> {
        vec![0.0; features.len()]
    }
}
static UNIFORM_PRIOR: UniformPrior = UniformPrior;

/// Run PUCT to completion on the current thread, using the armed neural prior as the
/// policy (uniform if none) and the `c-puct` option, with rollout-grounded leaves.
/// Shared by the PUCT method (CLI) and the GUI dispatch.
pub fn run_puct_armed(search: Arc<SearchState>, variant: crate::game::rules::Variant) {
    let c_puct = registry().value_f64("c-puct", 1.5);
    // A value net armed (--value-net) ⇒ value-guided leaves; otherwise rollout leaves.
    let value = armed_value();
    let cfg = puct::PuctConfig {
        c_puct,
        leaf: if value.is_some() {
            puct::LeafEval::Value
        } else {
            puct::LeafEval::Rollout
        },
        rollout_inv_temp: 1.0,
    };
    let vp = value.as_deref();
    match armed_arc() {
        Some(p) => puct::run_puct(search, variant, p.as_ref(), vp, &cfg),
        None => puct::run_puct(search, variant, &UNIFORM_PRIOR, vp, &cfg),
    }
}

/// The registry's neural move-bias modifier: encodes each candidate move locally,
/// runs the armed prior, and returns the scaled per-move logits. Inactive (and
/// zero-cost) when no prior is armed.
pub struct NeuralBias;
impl BiasModifier for NeuralBias {
    fn active(&self) -> bool {
        is_armed()
    }
    fn biases(&self, state: &GameState, moves: &[Move], out: &mut Vec<f64>) {
        out.clear();
        let guard = armed().read().unwrap();
        let Some(prior) = guard.as_ref() else {
            return; // disarmed between resolve and call — treat as all-zero
        };
        let scale = registry().value_f64("neural-scale", DEFAULT_SCALE);
        let generation = GENERATION.load(Ordering::Relaxed);
        BIAS_CACHE.with(|cell| {
            let mut cache = cell.borrow_mut();
            if cache.generation != generation {
                cache.generation = generation;
                cache.map.clear();
            }
            // Key each move by its local pattern; the net forward runs only on the
            // patterns not already cached this generation.
            let mut keys: Vec<PatchKey> = Vec::with_capacity(moves.len());
            let mut miss_keys: Vec<PatchKey> = Vec::new();
            let mut miss_feats: Vec<Vec<f32>> = Vec::new();
            for mv in moves {
                let (key, feat) = features::encode_keyed(state, mv);
                if !cache.map.contains_key(&key) {
                    miss_keys.push(key);
                    miss_feats.push(feat);
                }
                keys.push(key);
            }
            if !miss_feats.is_empty() {
                let logits = prior
                    .logits(&miss_feats)
                    .unwrap_or_else(|_| vec![0.0; miss_feats.len()]);
                for (k, l) in miss_keys.iter().zip(logits) {
                    cache.map.insert(*k, l as f64);
                }
            }
            // Read every move's (scaled) logit from the cache, in order.
            out.extend(
                keys.iter()
                    .map(|k| cache.map.get(k).copied().unwrap_or(0.0) * scale),
            );
        });
    }
}
/// The neural move-bias modifier, wired into the registry by the `NeuralBiasPlugin`
/// (`search::neural::plugin`).
pub static NEURAL_BIAS: NeuralBias = NeuralBias;

/// Convenience API to load, persist, and arm a move prior — the plumbing the CLI/GUI
/// use. The search infers on CPU (per-state, many threads), so a prior is always
/// loaded on CPU. (Training entry points land in a later phase-5 commit.)
pub mod prior {
    use super::dataset::{augmented_samples_from_corpus, augmented_samples_from_games, StateSample};
    use super::net::NeuralPrior;
    use super::train::{train, TrainConfig};
    use super::{armed, Arc};
    use crate::game::moves::Move;
    use crate::game::rules::Variant;
    use candle_core::Device;

    /// Train on `samples`, then round-trip the net through a temp safetensors so the
    /// returned prior holds plain (non-`Var`) CPU tensors — safe for the concurrent
    /// per-island inference in the hot loop. A freshly trained net keeps trainable
    /// `Var`s that aren't sound to share across island threads; the round-trip strips
    /// them. Shared by every `train_on_*`.
    fn train_and_freeze(samples: &[StateSample], epochs: usize, lr: f64) -> candle_core::Result<NeuralPrior> {
        let pr = train(samples, &TrainConfig { epochs, lr }, Device::Cpu)?;
        let tmp = std::env::temp_dir().join(format!("morpion_prior_{}.safetensors", std::process::id()));
        let tmps = tmp.to_string_lossy().into_owned();
        pr.save(&tmps)?;
        let loaded = NeuralPrior::load(&tmps, Device::Cpu);
        let _ = std::fs::remove_file(&tmp);
        loaded
    }

    /// Train a move prior on the human record corpus (D4-augmented), on CPU. ~40 s
    /// for the h64 net; the result is ready to [`arm`]/[`install`] or [`save`].
    pub fn train_on_corpus(variant: Variant, epochs: usize, lr: f64) -> candle_core::Result<NeuralPrior> {
        train_and_freeze(&augmented_samples_from_corpus(variant), epochs, lr)
    }

    /// Train a move prior on a set of games (D4-augmented), on CPU — the tabula-rasa
    /// path: only games the search produced, no human records. `games` must be
    /// non-empty (an empty set trains nothing useful).
    pub fn train_on_games(variant: Variant, games: &[Vec<Move>], epochs: usize, lr: f64) -> candle_core::Result<NeuralPrior> {
        train_and_freeze(&augmented_samples_from_games(variant, games), epochs, lr)
    }

    /// Train a move prior on the **embedded from-scratch corpus** for `variant` (the
    /// bundled self-found games — no human records), on CPU. Errors if none committed.
    pub fn train_on_bundled_corpus(variant: Variant, epochs: usize, lr: f64) -> candle_core::Result<NeuralPrior> {
        let games = super::embedded::corpus(variant);
        if games.is_empty() {
            return Err(candle_core::Error::Msg(format!(
                "no bundled from-scratch corpus for {} yet",
                variant.name()
            )));
        }
        train_on_games(variant, &games, epochs, lr)
    }

    /// The bundled pre-trained from-scratch prior for `variant`, if one is committed
    /// (instant — no training). See [`super::embedded`].
    pub fn bundled(variant: Variant) -> Option<NeuralPrior> {
        super::embedded::prior(variant)
    }

    /// Load a prior saved by [`save`] (safetensors), on CPU.
    pub fn load(path: &str) -> candle_core::Result<NeuralPrior> {
        NeuralPrior::load(path, Device::Cpu)
    }

    /// Save a trained prior to `path` (safetensors) for reuse across runs.
    pub fn save(prior: &NeuralPrior, path: &str) -> candle_core::Result<()> {
        prior.save(path)
    }

    /// Arm (or clear, with `None`) the prior for subsequent NRPA searches — every
    /// playout/adapt then adds its learned per-move bias. Call **before** launching
    /// the search. Bumps the cache generation so no thread serves stale logits.
    pub fn arm(prior: Option<Arc<NeuralPrior>>) {
        *armed().write().unwrap() = prior;
        super::bump_generation();
    }

    /// Like [`arm`] but takes ownership of a fresh prior (the common case).
    pub fn install(prior: Option<NeuralPrior>) {
        arm(prior.map(Arc::new));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::moves::legal_moves;
    use crate::game::rules::Variant;
    use std::sync::Mutex;

    // Tests that arm the process-global prior must not run concurrently (the armed slot
    // is shared); serialize them on this lock.
    static ARM_TEST_LOCK: Mutex<()> = Mutex::new(());

    /// The bundled 5T prior loads and produces one finite bias per legal move, and
    /// arming/disarming flips the modifier's active state.
    #[test]
    fn bundled_prior_biases_legal_moves() {
        let _g = ARM_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let Some(p) = prior::bundled(Variant::T5) else {
            panic!("bundled 5T prior should be committed");
        };
        assert!(!is_armed());
        prior::install(Some(p));
        assert!(is_armed());

        let st = GameState::new(Variant::T5);
        let moves = legal_moves(&st);
        let mut out = Vec::new();
        NEURAL_BIAS.biases(&st, &moves, &mut out);
        assert_eq!(out.len(), moves.len(), "one bias per legal move");
        assert!(out.iter().all(|b| b.is_finite()), "biases must be finite");

        prior::arm(None);
        assert!(!is_armed());
    }

    /// The train→freeze (save→reload) round-trip yields a usable prior: train on a
    /// couple of short self-played games and check it produces finite per-move biases.
    /// Fast (tiny set, 1 epoch) — it guards the freeze path `train_on_*` all use.
    #[test]
    fn trained_prior_round_trips() {
        let mut games = Vec::new();
        for _ in 0..2 {
            let mut st = GameState::new(Variant::T5);
            let mut h = Vec::new();
            for _ in 0..12 {
                let ms = legal_moves(&st);
                if ms.is_empty() {
                    break;
                }
                h.push(ms[0]);
                st.apply(ms[0]);
            }
            games.push(h);
        }
        let p = prior::train_on_games(Variant::T5, &games, 1, 1e-3).expect("train");
        let st = GameState::new(Variant::T5);
        let moves = legal_moves(&st);
        let feats: Vec<Vec<f32>> = moves.iter().map(|m| features::encode(&st, m)).collect();
        let b = net::MovePrior::biases(&p, &feats);
        assert_eq!(b.len(), moves.len());
        assert!(b.iter().all(|x| x.is_finite()));
    }

    /// The per-pattern bias cache is transparent: a second call for the same position
    /// (served from the cache) matches the first (computed by the net), and re-arming
    /// bumps the generation so the cache is rebuilt without serving stale logits.
    #[test]
    fn bias_cache_is_transparent_and_generation_aware() {
        let _g = ARM_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let p = prior::bundled(Variant::T5).expect("bundled prior");
        prior::install(Some(p));
        let st = GameState::new(Variant::T5);
        let moves = legal_moves(&st);

        let mut first = Vec::new();
        NEURAL_BIAS.biases(&st, &moves, &mut first); // misses → net forward, fills cache
        let mut second = Vec::new();
        NEURAL_BIAS.biases(&st, &moves, &mut second); // hits → from cache
        assert_eq!(first, second, "cache hit must match the computed result");
        assert!(first.iter().all(|x| x.is_finite()));

        // Re-arm the same prior: generation bumps, cache clears, result is consistent.
        prior::arm(None);
        prior::install(prior::bundled(Variant::T5));
        let mut third = Vec::new();
        NEURAL_BIAS.biases(&st, &moves, &mut third);
        assert_eq!(first, third, "a fresh generation reproduces the same logits");

        prior::arm(None);
    }

    /// Feature-space warm init reproduces the frozen prior: with θ₀ = scale·head, the
    /// adaptive logit θ·φ equals the frozen logit scale·β up to a per-move-constant
    /// (the head bias, which cancels in the softmax). So the move-to-move *differences*
    /// must match — the property the whole "warm start = no regression at step 0" rests on.
    #[test]
    fn feat_warm_reproduces_frozen_prior() {
        let _g = ARM_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let reg = registry();
        prior::install(prior::bundled(Variant::T5));
        reg.set_value("feat-adapt", OptionValue::Toggle(true));
        let scale = reg.value_f64("neural-scale", DEFAULT_SCALE);

        feat::restart(); // seeds this thread's warm θ₀ = scale·head
        let st = GameState::new(Variant::T5);
        let moves = legal_moves(&st);
        let mut adaptive = Vec::new();
        feat::logits(&st, &moves, &mut adaptive); // θ·φ per move

        // Frozen prior logits (raw net), what θ·φ should reproduce up to a constant.
        let p = prior::bundled(Variant::T5).unwrap();
        let feats: Vec<Vec<f32>> = moves.iter().map(|m| features::encode(&st, m)).collect();
        let frozen = net::MovePrior::biases(&p, &feats);

        // (θ·φ[i] − θ·φ[0]) must equal scale·(β[i] − β[0]) for every move.
        for i in 1..moves.len() {
            let lhs = adaptive[i] - adaptive[0];
            let rhs = scale * (frozen[i] - frozen[0]);
            assert!(
                (lhs - rhs).abs() < 1e-2,
                "warm θ·φ should track scale·frozen: move {i} {lhs} vs {rhs}"
            );
        }

        reg.set_value("feat-adapt", OptionValue::Toggle(false));
        prior::arm(None);
    }

    /// A logits()→adapt() round moves the head θ by a finite amount, exercising the
    /// keys-reuse path (adapt reads the keys stashed by the preceding logits call). The
    /// chosen move's contribution must push its own logit up relative to the rest.
    #[test]
    fn feat_adapt_updates_head_finitely() {
        let _g = ARM_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let reg = registry();
        prior::install(prior::bundled(Variant::T5));
        reg.set_value("feat-adapt", OptionValue::Toggle(true));
        feat::restart();

        let st = GameState::new(Variant::T5);
        let moves = legal_moves(&st);
        let mut before = Vec::new();
        feat::logits(&st, &moves, &mut before);
        // Uniform probs, pick move 0 as chosen; adapt must reuse the stashed keys.
        let probs = vec![1.0 / moves.len() as f64; moves.len()];
        feat::adapt(&st, &moves, &moves[0], &probs);
        let mut after = Vec::new();
        feat::logits(&st, &moves, &mut after);

        assert!(after.iter().all(|x| x.is_finite()), "θ·φ must stay finite after adapt");
        assert!(
            after.iter().zip(&before).any(|(a, b)| (a - b).abs() > 1e-9),
            "adapt should move at least one logit"
        );
        // The chosen move gains relative to the mean (its φ was reinforced).
        let mean_delta =
            after.iter().zip(&before).map(|(a, b)| a - b).sum::<f64>() / moves.len() as f64;
        assert!(
            (after[0] - before[0]) >= mean_delta - 1e-9,
            "the chosen move's logit should rise at least as much as the mean"
        );

        reg.set_value("feat-adapt", OptionValue::Toggle(false));
        prior::arm(None);
    }
}
