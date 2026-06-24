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
    }
}
pub static FEATURE_SPACE_PLUGIN: FeatureSpacePlugin = FeatureSpacePlugin;
