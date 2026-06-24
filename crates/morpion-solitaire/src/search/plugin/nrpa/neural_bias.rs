//! The neural move prior as a `BiasModifier` of the NRPA family (`neural` feature).
//! The engine (the net, arming, the per-pattern cache) lives in `search::neural`; this
//! file is the thin plugin that wires it into the registry and owns `--neural-scale`.

use crate::search::neural;
use crate::search::plugin::{OptionKind, OptionSpec, Plugin, Registry, Scope};

pub struct NeuralBiasPlugin;
impl Plugin for NeuralBiasPlugin {
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
        reg.add_bias(&neural::NEURAL_BIAS);
        reg.add_option(OptionSpec {
            key: "neural-scale",
            label_key: "opt-neural-scale",
            help_key: "opt-neural-scale-hint",
            help: "Neural-prior strength (β scale). Sweet spot ≈ 4; only applies with \
                   --prior. Read once per search.",
            kind: OptionKind::Float {
                default: neural::DEFAULT_SCALE,
                min: 0.1,
                max: 10.0,
                step: 0.5,
            },
            scope: Scope::NrpaFamily,
        });
    }
}
pub static NEURAL_BIAS_PLUGIN: NeuralBiasPlugin = NeuralBiasPlugin;
