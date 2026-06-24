//! The NRPA-family **core** plugins, co-located with the engine ([`super`]): the NRPA
//! method itself (+ `--level`), the policy-update hyperparameters (`--clamp`/`--alpha`, an
//! `AdaptModifier`), and symmetry-invariant move coding (`--no-symmetry`, a
//! `CodingModifier`). Perturbation — a large-neighbourhood *variant* of NRPA — and its
//! crossover modifier live in the sibling [`super::perturbation`] module.
//!
//! `--clamp`/`--alpha`/`--symmetry` scope to the whole NRPA family (perturbation wraps
//! NRPA), expressed declaratively by `Scope::NrpaFamily` rather than by location.

use std::sync::Arc;

use crate::search::plugin::{
    AdaptModifier, CodingModifier, Method, OptionKind, OptionSpec, Plugin, Registry, Scope,
    StartCtx,
};
use crate::search::SearchState;

use super::{run, run_warm, WARM_ITERS};

// ── the NRPA method ───────────────────────────────────────────────────────────

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
                std::thread::spawn(move || run_warm(&initial, search, level, &seq, WARM_ITERS));
            }
            None => {
                std::thread::spawn(move || run(&initial, search, level));
            }
        }
    }
    fn method_desc(&self, ctx: &StartCtx) -> String {
        if ctx.warm_seq.is_some() {
            format!(
                "nrpa-seeded L{} warm-from={} warm={}",
                ctx.level, ctx.seed_len, WARM_ITERS
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

// ── the adapt hyperparameters (clamp C, step α) ────────────────────────────────

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

// ── symmetry-invariant move coding ─────────────────────────────────────────────

/// Owns `--symmetry`/`--no-symmetry`. On (default): canonical D4 coding, all 8 Zobrist
/// hashes maintained. Off: identity frame only (one hash), ~+16% throughput at neutral
/// score — for cold record runs.
struct CoreCoding;
impl CodingModifier for CoreCoding {}
static CORE_CODING: CoreCoding = CoreCoding;

pub struct SymmetryPlugin;
impl Plugin for SymmetryPlugin {
    fn id(&self) -> &'static str {
        "symmetry"
    }
    fn register(&self, reg: &mut Registry) {
        reg.add_coding(&CORE_CODING);
        reg.add_option(OptionSpec {
            key: "symmetry",
            label_key: "opt-symmetry",
            help_key: "opt-symmetry-hint",
            help: "Drop symmetry-invariant move coding (identity frame only): ~+16% \
                   throughput at neutral score — recommended for cold record runs \
                   without warm-start. (The flag is `--no-symmetry`.)",
            kind: OptionKind::Toggle { default: true },
            scope: Scope::NrpaFamily,
        });
    }
}
pub static SYMMETRY_PLUGIN: SymmetryPlugin = SymmetryPlugin;
