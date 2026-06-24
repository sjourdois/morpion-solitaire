//! Feature-space NRPA: an adaptive head θ over the net's frozen features φ (φ-B), in
//! place of the frozen prior bias (`neural` feature). The engine lives in
//! `search::neural::feat`; this file owns the `feat-adapt` + `feat-alpha` options.

use crate::search::neural::feat;
use crate::search::plugin::{OptionKind, OptionSpec, Plugin, Registry, Scope};

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
