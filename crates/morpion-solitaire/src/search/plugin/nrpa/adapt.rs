//! The policy-update hyperparameters — logit clamp C and step α (an `AdaptModifier`).

use crate::search::plugin::{AdaptModifier, OptionKind, OptionSpec, Plugin, Registry, Scope};

/// Owns `--clamp`/`--alpha`. The logit clamp (Stabilized-NRPA) is on by default at C=3;
/// `--clamp 0` disables. α default 1.0.
struct CoreAdapt;
impl AdaptModifier for CoreAdapt {}
static CORE_ADAPT: CoreAdapt = CoreAdapt;

pub struct AdaptPlugin;
impl Plugin for AdaptPlugin {
    fn id(&self) -> &'static str {
        "adapt"
    }
    fn register(&self, reg: &mut Registry) {
        reg.add_adapt(&CORE_ADAPT);
        reg.add_option(OptionSpec {
            key: "clamp",
            label_key: "opt-clamp",
            help_key: "opt-clamp-hint",
            help: "Stabilized-NRPA logit clamp C (default 3; 0 disables clamping). The \
                   tight sweet spot for record hunting; only re-tune for experiments.",
            kind: OptionKind::Float {
                default: 3.0,
                min: 0.0,
                max: 10.0,
                step: 0.5,
            },
            scope: Scope::NrpaFamily,
        });
        reg.add_option(OptionSpec {
            key: "alpha",
            label_key: "opt-alpha",
            help_key: "opt-alpha-hint",
            help: "Policy adaptation step size α (default 1.0).",
            kind: OptionKind::Float {
                default: 1.0,
                min: 0.1,
                max: 3.0,
                step: 0.05,
            },
            scope: Scope::NrpaFamily,
        });
    }
}
pub static ADAPT_PLUGIN: AdaptPlugin = AdaptPlugin;
