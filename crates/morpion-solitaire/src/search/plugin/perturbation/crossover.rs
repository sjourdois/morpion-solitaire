//! Genetic crossover of archived games — a `PerturbModifier` of the perturbation
//! method (`deps = ["perturbation"]`, so it's dropped in a build without it).

use crate::search::plugin::{OptionKind, OptionSpec, PerturbModifier, Plugin, Registry, Scope};

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
