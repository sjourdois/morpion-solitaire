//! NRPA — the nested rollout policy adaptation method, and the modifiers that act on
//! the NRPA family (symmetry coding, the adapt step, macro-actions, and the neural
//! prior / feature-space head). Perturbation, a large-neighbourhood *variant* of NRPA,
//! lives in the sibling `perturbation` module.

pub mod adapt;
pub mod symmetry;

// Macro-actions use `nrpa::move_playable` (native-only).
#[cfg(not(target_arch = "wasm32"))]
pub mod macros;

// The neural prior (BiasModifier) and the feature-space head (AdaptModifier) — they
// pull candle, so they are compiled only under the `neural` feature.
#[cfg(all(feature = "neural", not(target_arch = "wasm32")))]
pub mod feature_space;
#[cfg(all(feature = "neural", not(target_arch = "wasm32")))]
pub mod neural_bias;

use std::sync::Arc;

use crate::search::{nrpa, SearchState};

use super::{Method, OptionKind, OptionSpec, Plugin, Registry, Scope, StartCtx};

struct Nrpa;
impl Method for Nrpa {
    fn id(&self) -> &'static str {
        "nrpa"
    }
    fn label_key(&self) -> &'static str {
        "algo-nrpa"
    }
    fn spawn(&self, ctx: StartCtx, search: Arc<SearchState>) {
        let StartCtx {
            initial,
            level,
            warm_seq,
            ..
        } = ctx;
        match warm_seq {
            Some(seq) => {
                std::thread::spawn(move || {
                    nrpa::run_warm(&initial, search, level, &seq, nrpa::WARM_ITERS)
                });
            }
            None => {
                std::thread::spawn(move || nrpa::run(&initial, search, level));
            }
        }
    }
    fn method_desc(&self, ctx: &StartCtx) -> String {
        if ctx.warm_seq.is_some() {
            format!(
                "nrpa-seeded L{} warm-from={} warm={}",
                ctx.level, ctx.seed_len, nrpa::WARM_ITERS
            )
        } else {
            format!("nrpa L{}", ctx.level)
        }
    }
    fn checkpoint_kind(&self) -> Option<&'static str> {
        Some("nrpa")
    }
}
static NRPA: Nrpa = Nrpa;

pub struct NrpaPlugin;
impl Plugin for NrpaPlugin {
    fn id(&self) -> &'static str {
        "nrpa"
    }
    fn register(&self, reg: &mut Registry) {
        reg.add_method(&NRPA);
        // Nesting level applies to the whole NRPA family (perturbation wraps NRPA).
        reg.add_option(OptionSpec {
            key: "level",
            label_key: "opt-level",
            help_key: "opt-level-hint",
            help: "NRPA nesting level (recursion depth). 3 is the fast default; 4+ \
                   searches more deeply but only pays off over long runs.",
            kind: OptionKind::Int {
                default: 3,
                min: 1,
                max: 6,
            },
            scope: Scope::NrpaFamily,
        });
    }
}
pub static NRPA_PLUGIN: NrpaPlugin = NrpaPlugin;
