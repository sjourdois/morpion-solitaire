//! Perturbation — a large-neighbourhood **variant** of NRPA (`parent = "nrpa"`): it drives
//! time-bounded inner NRPA searches via OS threads, so this whole module is native-only.
//! The engine (`run_perturbation`) is the parent [`super`] module; this file is the method
//! registration plus its `crossover` modifier (genetic recombination of archived games).

use std::sync::Arc;

use crate::search::plugin::{
    Method, OptionKind, OptionSpec, PerturbModifier, Plugin, Registry, Scope, StartCtx,
};
use crate::search::SearchState;

struct Perturbation;
impl Method for Perturbation {
    fn id(&self) -> &'static str {
        "perturbation"
    }
    fn parent(&self) -> Option<&'static str> {
        Some("nrpa") // a large-neighbourhood variant of NRPA, not a standalone engine
    }
    fn label_key(&self) -> &'static str {
        "algo-perturbation"
    }
    fn spawn(&self, ctx: StartCtx, search: Arc<SearchState>) {
        let StartCtx {
            level,
            variant,
            seed_history,
            ..
        } = ctx;
        // The crossover rate is a PerturbModifier resolved from the registry inside
        // run_perturbation — set via the values map before launching.
        std::thread::spawn(move || super::run_perturbation(search, level, seed_history, variant));
    }
    fn method_desc(&self, ctx: &StartCtx) -> String {
        format!("perturbation L{}", ctx.level)
    }
    fn checkpoint_kind(&self) -> Option<&'static str> {
        Some("perturbation")
    }
}
static PERTURBATION: Perturbation = Perturbation;

pub struct PerturbationPlugin;
impl Plugin for PerturbationPlugin {
    fn id(&self) -> &'static str {
        "perturbation"
    }
    fn register(&self, reg: &mut Registry) {
        reg.add_method(&PERTURBATION);
    }
}
pub static PERTURBATION_PLUGIN: PerturbationPlugin = PerturbationPlugin;

// ── crossover modifier ─────────────────────────────────────────────────────────

/// Owns `--crossover` (0 = off). Genetic recombination of archived games can reach
/// combinations a single-game destroy/repair can't.
struct CoreCrossover;
impl PerturbModifier for CoreCrossover {}
static CORE_CROSSOVER: CoreCrossover = CoreCrossover;

pub struct CrossoverPlugin;
impl Plugin for CrossoverPlugin {
    fn id(&self) -> &'static str {
        "crossover"
    }
    fn deps(&self) -> &'static [&'static str] {
        &["perturbation"]
    }
    fn register(&self, reg: &mut Registry) {
        reg.add_perturb(&CORE_CROSSOVER);
        reg.add_option(OptionSpec {
            key: "crossover",
            label_key: "opt-crossover",
            help_key: "opt-crossover-hint",
            help: "Perturbation genetic-crossover rate (0 = off). Only used by \
                   `--algo perturbation`.",
            kind: OptionKind::Float {
                default: 0.0,
                min: 0.0,
                max: 1.0,
                step: 0.05,
            },
            scope: Scope::Methods(&["perturbation"]),
        });
    }
}
pub static CROSSOVER_PLUGIN: CrossoverPlugin = CrossoverPlugin;
