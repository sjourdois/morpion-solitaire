//! The neural registry plugins, co-located with the neural engine ([`super`]): the move
//! prior as a `BiasModifier` (`--neural-scale`), the feature-space head (`--feat-*`), and
//! the PUCT method (`--algo puct`, `c-puct`). The engines (nets, prior, the φ head, the
//! PUCT tree) are the parent module. `neural` feature, native-only.
//!
//! The prior is shared by two methods: NRPA reads it through the `BiasModifier` hook,
//! PUCT reads the armed prior directly — so the engine is a shared subsystem, while these
//! registrations live with it. The bias/feature-space options scope to the NRPA family.

use std::sync::Arc;

use crate::search::plugin::{
    registry, Method, OptionKind, OptionSpec, Plugin, Registry, Scope, StartCtx,
};
use crate::search::SearchState;

use super::feat;

// ── the neural move prior (a BiasModifier) ─────────────────────────────────────

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
        reg.add_bias(&super::NEURAL_BIAS);
        reg.add_option(OptionSpec {
            key: "neural-scale",
            label_key: "opt-neural-scale",
            help_key: "opt-neural-scale-hint",
            help: "Neural-prior strength (β scale). Sweet spot ≈ 4; only applies with \
                   --prior. Read once per search.",
            kind: OptionKind::Float {
                default: super::DEFAULT_SCALE,
                min: 0.1,
                max: 10.0,
                step: 0.5,
            },
            scope: Scope::NrpaFamily,
        });
    }
}
pub static NEURAL_BIAS_PLUGIN: NeuralBiasPlugin = NeuralBiasPlugin;

// ── feature-space NRPA (the adaptive head θ over frozen φ) ──────────────────────

pub struct FeatureSpacePlugin;
impl Plugin for FeatureSpacePlugin {
    fn id(&self) -> &'static str {
        "feature-space"
    }
    fn deps(&self) -> &'static [&'static str] {
        &["nrpa"]
    }
    fn experimental(&self) -> bool {
        true
    }
    fn register(&self, reg: &mut Registry) {
        reg.add_option(OptionSpec {
            key: "feat-adapt",
            label_key: "opt-feat-adapt",
            help_key: "opt-feat-adapt-hint",
            help: "Feature-space NRPA: adapt a linear head θ over the net's frozen \
                   features online (φ-B) instead of a frozen prior bias. Needs --prior. \
                   Experimental.",
            kind: OptionKind::Toggle { default: false },
            scope: Scope::NrpaFamily,
        });
        reg.add_option(OptionSpec {
            key: "feat-alpha",
            label_key: "opt-feat-alpha",
            help_key: "opt-feat-alpha-hint",
            help: "Feature-space head step size α_θ (default 0.1). Only with --feat-adapt.",
            kind: OptionKind::Float {
                default: feat::DEFAULT_FEAT_ALPHA,
                min: 0.01,
                max: 1.0,
                step: 0.01,
            },
            scope: Scope::NrpaFamily,
        });
        // The adaptation-regime variants (validated defaults == φ-B). All experimental
        // (the whole plugin is), so hidden unless --experimental. See feat.rs.
        reg.add_option(OptionSpec {
            key: "feat-table",
            label_key: "opt-feat-table",
            help_key: "opt-feat-table-hint",
            help: "Keep the one-hot policy table alongside θ·φ (φ-B, default). \
                   --no-feat-table drops it for the head-only φ-A. Only with --feat-adapt.",
            kind: OptionKind::Toggle { default: true },
            scope: Scope::NrpaFamily,
        });
        reg.add_option(OptionSpec {
            key: "feat-warm",
            label_key: "opt-feat-warm",
            help_key: "opt-feat-warm-hint",
            help: "Warm-init θ₀ = scale·head to reproduce the frozen prior at step 0 \
                   (default). --no-feat-warm starts cold (θ₀ = 0). Only with --feat-adapt.",
            kind: OptionKind::Toggle { default: true },
            scope: Scope::NrpaFamily,
        });
        reg.add_option(OptionSpec {
            key: "feat-lambda",
            label_key: "opt-feat-lambda",
            help_key: "opt-feat-lambda-hint",
            help: "Feature-space L2 decay λ: θ ← (1−λ)θ after each adapt (default 0 = off). \
                   Only with --feat-adapt.",
            kind: OptionKind::Float {
                default: 0.0,
                min: 0.0,
                max: 0.1,
                step: 0.001,
            },
            scope: Scope::NrpaFamily,
        });
        reg.add_option(OptionSpec {
            key: "feat-clamp",
            label_key: "opt-feat-clamp",
            help_key: "opt-feat-clamp-hint",
            help: "Feature-space head clamp C: |θ_j| ≤ C after each adapt (default 0 = off). \
                   Only with --feat-adapt.",
            kind: OptionKind::Float {
                default: 0.0,
                min: 0.0,
                max: 10.0,
                step: 0.5,
            },
            scope: Scope::NrpaFamily,
        });
        reg.add_option(OptionSpec {
            key: "feat-norm",
            label_key: "opt-feat-norm",
            help_key: "opt-feat-norm-hint",
            help: "L2-normalize each cached φ to unit length (default off). Breaks warm \
                   reproduction; for cold-init sweeps. Only with --feat-adapt.",
            kind: OptionKind::Toggle { default: false },
            scope: Scope::NrpaFamily,
        });
    }
}
pub static FEATURE_SPACE_PLUGIN: FeatureSpacePlugin = FeatureSpacePlugin;

// ── PUCT (policy + value tree search) ──────────────────────────────────────────

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
        std::thread::spawn(move || super::run_puct_armed(search, variant));
    }
    fn method_desc(&self, _ctx: &StartCtx) -> String {
        let c = registry().value_f64("c-puct", 1.5);
        let policy = if super::is_armed() { "neural" } else { "uniform" };
        let leaf = if super::armed_value().is_some() {
            "value"
        } else {
            "rollout"
        };
        format!("puct c={c:.2} policy={policy} leaf={leaf}")
    }
    fn checkpoint_kind(&self) -> Option<&'static str> {
        None
    }
}
static PUCT_METHOD: PuctMethod = PuctMethod;

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
pub static PUCT_PLUGIN: PuctPlugin = PuctPlugin;
