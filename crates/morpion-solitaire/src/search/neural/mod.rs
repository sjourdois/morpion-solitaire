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
pub mod features;
pub mod net;
pub mod position;
pub mod puct;
pub mod selfplay;
pub mod tabula_rasa;
pub mod train;

use std::sync::{Arc, OnceLock, RwLock};

use crate::game::{moves::Move, state::GameState};
use crate::search::plugin::{
    self, BiasModifier, Method, OptionKind, OptionSpec, Plugin, Registry, Scope, StartCtx,
};
use crate::search::SearchState;
use net::{MovePrior, NeuralPrior};

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

/// Set the prior strength (β scale) for subsequent searches by writing the registry's
/// `neural-scale` option — the same value the [`NeuralBias`] reads. Used by tabula-rasa
/// to anneal the scale across Expert-Iteration rounds.
pub fn set_scale(scale: f64) {
    plugin::registry().set_value("neural-scale", plugin::OptionValue::Float(scale));
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
    let c_puct = plugin::registry().value_f64("c-puct", 1.5);
    let cfg = puct::PuctConfig {
        c_puct,
        leaf: puct::LeafEval::Rollout,
        rollout_inv_temp: 1.0,
    };
    match armed_arc() {
        Some(p) => puct::run_puct(search, variant, p.as_ref(), None, &cfg),
        None => puct::run_puct(search, variant, &UNIFORM_PRIOR, None, &cfg),
    }
}

/// PUCT (policy + value tree search) as a registry method: the armed neural prior is
/// the policy (uniform if none), with rollout-grounded leaf evaluation. The value-net
/// leaf (`LeafEval::Value`) is wired in the net/training code but not yet exposed here.
struct PuctMethod;
impl Method for PuctMethod {
    fn id(&self) -> &'static str {
        "puct"
    }
    fn label_key(&self) -> &'static str {
        "algo-puct"
    }
    fn spawn(&self, ctx: StartCtx, search: Arc<SearchState>) {
        let StartCtx { variant, .. } = ctx;
        std::thread::spawn(move || run_puct_armed(search, variant));
    }
    fn method_desc(&self, _ctx: &StartCtx) -> String {
        let c = plugin::registry().value_f64("c-puct", 1.5);
        let policy = if is_armed() { "neural" } else { "uniform" };
        format!("puct c={c:.2} policy={policy}")
    }
    fn checkpoint_kind(&self) -> Option<&'static str> {
        None
    }
}
static PUCT_METHOD: PuctMethod = PuctMethod;

/// The PUCT plugin: contributes the method + its `c-puct` exploration option. Compiled
/// only under the `neural` feature (it needs the policy net).
pub struct PuctPlugin;
impl Plugin for PuctPlugin {
    fn id(&self) -> &'static str {
        "puct"
    }
    fn experimental(&self) -> bool {
        true
    }
    fn register(&self, reg: &mut Registry) {
        reg.add_method(&PUCT_METHOD);
        reg.add_option(OptionSpec {
            key: "c-puct",
            label_key: "opt-c-puct",
            help_key: "opt-c-puct-hint",
            help: "PUCT exploration constant (higher = more exploration). Default 1.5.",
            kind: OptionKind::Float {
                default: 1.5,
                min: 0.1,
                max: 5.0,
                step: 0.1,
            },
            scope: Scope::Methods(&["puct"]),
        });
    }
}
/// The static PUCT plugin, pushed into the registry under the `neural` feature.
pub static PUCT_PLUGIN: PuctPlugin = PuctPlugin;

/// The registry's neural move-bias modifier: encodes each candidate move locally,
/// runs the armed prior, and returns the scaled per-move logits. Inactive (and
/// zero-cost) when no prior is armed.
struct NeuralBias;
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
        let scale = plugin::registry().value_f64("neural-scale", DEFAULT_SCALE);
        // Encode each legal move's local patch, then one batched net forward. (No
        // per-pattern cache yet — correctness first; the neural-guide cache is a later
        // throughput commit.)
        let feats: Vec<Vec<f32>> = moves.iter().map(|m| features::encode(state, m)).collect();
        out.extend(prior.biases(&feats).into_iter().map(|b| b * scale));
    }
}
static NEURAL_BIAS: NeuralBias = NeuralBias;

/// The neural plugin: contributes the move-bias hook and the `--neural-scale` option.
/// Depends on `nrpa` (the bias only means anything inside the NRPA softmax), so it is
/// dropped in a build without it.
pub struct NeuralPlugin;
impl Plugin for NeuralPlugin {
    fn id(&self) -> &'static str {
        "neural"
    }
    fn deps(&self) -> &'static [&'static str] {
        &["nrpa"]
    }
    fn experimental(&self) -> bool {
        true
    }
    fn register(&self, reg: &mut Registry) {
        reg.add_bias(&NEURAL_BIAS);
        reg.add_option(OptionSpec {
            key: "neural-scale",
            label_key: "opt-neural-scale",
            help_key: "opt-neural-scale-hint",
            help: "Neural-prior strength (β scale). Sweet spot ≈ 4; only applies with \
                   --prior. Read once per search.",
            kind: OptionKind::Float {
                default: DEFAULT_SCALE,
                min: 0.1,
                max: 10.0,
                step: 0.5,
            },
            scope: Scope::NrpaFamily,
        });
    }
}
/// The static plugin instance, pushed into the registry under the `neural` feature.
pub static NEURAL_PLUGIN: NeuralPlugin = NeuralPlugin;

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
    /// the search.
    pub fn arm(prior: Option<Arc<NeuralPrior>>) {
        *armed().write().unwrap() = prior;
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

    /// The bundled 5T prior loads and produces one finite bias per legal move, and
    /// arming/disarming flips the modifier's active state.
    #[test]
    fn bundled_prior_biases_legal_moves() {
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
}
