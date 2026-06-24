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

pub mod embedded;
pub mod features;
pub mod net;

use std::sync::{Arc, OnceLock, RwLock};

use crate::game::{moves::Move, state::GameState};
use crate::search::plugin::{
    self, BiasModifier, OptionKind, OptionSpec, Plugin, Registry, Scope,
};
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
    use super::net::NeuralPrior;
    use super::{armed, Arc};
    use crate::game::rules::Variant;
    use candle_core::Device;

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
}
