//! Perturbation — a large-neighbourhood **variant** of NRPA (`parent = "nrpa"`): it
//! drives time-bounded inner NRPA searches via OS threads, so the whole module is
//! native-only. Its `crossover` modifier is the sibling submodule.

pub mod crossover;

use std::sync::Arc;

use crate::search::{nrpa, SearchState};

use super::{Method, Plugin, Registry, StartCtx};

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
        std::thread::spawn(move || nrpa::run_perturbation(search, level, seed_history, variant));
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
