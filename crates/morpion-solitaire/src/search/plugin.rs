//! Plugin framework for search methods, modifiers & options (`docs/plugin-framework.md`).
//!
//! A **plugin** is a generic contribution unit (a method, a modifier of a search at a
//! named hook, an option, UI) with **dependencies** on other plugins. The core itself
//! is expressed as plugins; experimental ones register only under their Cargo feature.
//! The CLI and GUI dispatch through the [`Registry`] and name no specific plugin.
//!
//! In place: the `Plugin`/`Registry`/`Method` scaffolding + core method plugins; the
//! `CodingModifier`/`AdaptModifier`/`PerturbModifier` hooks (resolved once per search
//! into a scalar — no per-node cost) with core modifier plugins (symmetry, clamp/α,
//! crossover); and declarative [`OptionSpec`]s for generic CLI/GUI rendering. Still to
//! come: the dynamic CLI + generic GUI consuming the specs, and experimental plugins.

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};

use crate::game::{moves::Move, rules::Variant, state::GameState};

use super::{beam, nrpa, systematic, SearchState};

/// Resolved launch context: the start position and the seed material a method needs.
pub struct StartCtx {
    pub initial: GameState,
    pub variant: Variant,
    pub level: usize,
    pub width: usize,
    /// NRPA policy warm-start sequence (the loaded game), if any.
    pub warm_seq: Option<Vec<Move>>,
    /// Perturbation seed (the loaded game's history; empty ⇒ bootstrap from the cross).
    pub seed_history: Vec<Move>,
    /// Length of the loaded game (for provenance).
    pub seed_len: usize,
}

/// A runnable search method — one kind of plugin contribution. Implementors spawn
/// their own search thread.
pub trait Method: Sync {
    /// Stable id: the CLI `--algo` value and the checkpoint engine name.
    fn id(&self) -> &'static str;
    /// i18n key for the GUI label.
    fn label_key(&self) -> &'static str;
    /// Launch the search on its own thread, driving `search`.
    fn spawn(&self, ctx: StartCtx, search: Arc<SearchState>);
    /// Provenance string stored in the output metadata.
    fn method_desc(&self, ctx: &StartCtx) -> String;
    /// Checkpoint engine name for the periodic auto-checkpoint, or `None`.
    fn checkpoint_kind(&self) -> Option<&'static str>;
}

/// Where plugins register their contributions: methods and modifier hooks. Option
/// specs and further hooks are added in later phases.
#[derive(Default)]
pub struct Registry {
    methods: Vec<&'static dyn Method>,
    coding: Option<&'static dyn CodingModifier>,
    adapt: Option<&'static dyn AdaptModifier>,
    perturb: Option<&'static dyn PerturbModifier>,
    options: Vec<OptionSpec>,
}

impl Registry {
    pub fn add_method(&mut self, m: &'static dyn Method) {
        self.methods.push(m);
    }
    pub fn add_coding(&mut self, m: &'static dyn CodingModifier) {
        self.coding = Some(m);
    }
    pub fn add_adapt(&mut self, m: &'static dyn AdaptModifier) {
        self.adapt = Some(m);
    }
    pub fn add_perturb(&mut self, m: &'static dyn PerturbModifier) {
        self.perturb = Some(m);
    }
    pub fn add_option(&mut self, spec: OptionSpec) {
        self.options.push(spec);
    }
    /// All option specs contributed by registered plugins (for CLI/GUI rendering).
    pub fn options(&self) -> &[OptionSpec] {
        &self.options
    }
    /// All registered methods, in registration (dependency) order.
    pub fn methods(&self) -> &[&'static dyn Method] {
        &self.methods
    }
    /// Look up a method by its id (the CLI `--algo` value).
    pub fn method(&self, id: &str) -> Option<&'static dyn Method> {
        self.methods.iter().copied().find(|m| m.id() == id)
    }

    // Hooks resolved once per search into a scalar (no per-node cost). Defaults
    // match plain NRPA when no modifier is registered.
    pub fn sym_on(&self) -> bool {
        self.coding.is_none_or(|c| c.sym_on())
    }
    pub fn clamp(&self) -> Option<f64> {
        self.adapt.map_or(Some(3.0), |a| a.clamp())
    }
    pub fn alpha(&self) -> f64 {
        self.adapt.map_or(1.0, |a| a.alpha())
    }
    /// Perturbation crossover rate (0 = off) — read once per perturbation round.
    pub fn crossover_rate(&self) -> f64 {
        self.perturb.map_or(0.0, |p| p.crossover_rate())
    }
}

// ---- modifier hooks (resolved once per search) ----------------------------

/// Move-coding hook: symmetry-invariant (canonical D4) coding vs the identity frame.
pub trait CodingModifier: Sync {
    fn sym_on(&self) -> bool;
}
/// Adapt hook: the policy-update hyperparameters — logit clamp C and step α.
pub trait AdaptModifier: Sync {
    fn clamp(&self) -> Option<f64>;
    fn alpha(&self) -> f64;
}
/// Perturbation-round hook: probability a round recombines two archived games
/// (`crossover_games`) instead of destroy/repair of one.
pub trait PerturbModifier: Sync {
    fn crossover_rate(&self) -> f64;
}

const F64_UNSET: u64 = u64::MAX; // f64-bits sentinel ⇒ "not set ⇒ default"

/// Core symmetry modifier. Owns the `--no-symmetry` state. On (default): canonical
/// D4 coding, all 8 Zobrist hashes maintained. Off: identity frame only (one hash),
/// ~+16% throughput at neutral score — for cold record runs.
struct CoreCoding {
    off: AtomicU8, // 0 unset · 1 off · 2 on
}
impl CodingModifier for CoreCoding {
    fn sym_on(&self) -> bool {
        self.off.load(Ordering::Relaxed) != 1
    }
}
static CORE_CODING: CoreCoding = CoreCoding {
    off: AtomicU8::new(0),
};

/// Core adapt modifier. Owns `--clamp`/`--alpha`. The logit clamp (Stabilized-NRPA)
/// is on by default at C=3 (tight sweet spot; 5T L4/120 s ~112 vs ~95 unclamped);
/// `--clamp 0` disables. α default 1.0 (0.5/2.0 regressed unclamped).
struct CoreAdapt {
    clamp_bits: AtomicU64,
    alpha_bits: AtomicU64,
}
impl AdaptModifier for CoreAdapt {
    fn clamp(&self) -> Option<f64> {
        let o = self.clamp_bits.load(Ordering::Relaxed);
        if o != F64_UNSET {
            let c = f64::from_bits(o);
            return if c > 0.0 { Some(c) } else { None };
        }
        Some(3.0)
    }
    fn alpha(&self) -> f64 {
        let o = self.alpha_bits.load(Ordering::Relaxed);
        if o != F64_UNSET {
            let a = f64::from_bits(o);
            if a > 0.0 {
                return a;
            }
        }
        1.0
    }
}
static CORE_ADAPT: CoreAdapt = CoreAdapt {
    clamp_bits: AtomicU64::new(F64_UNSET),
    alpha_bits: AtomicU64::new(F64_UNSET),
};

/// CLI/GUI setters (no env vars). Set before a search launches; every island reads
/// the modifier via the registry.
#[allow(dead_code)] // used by the CLI/GUI
pub fn set_symmetry(on: bool) {
    CORE_CODING.off.store(if on { 2 } else { 1 }, Ordering::Relaxed);
}
#[allow(dead_code)]
pub fn set_clamp(c: f64) {
    CORE_ADAPT.clamp_bits.store(c.to_bits(), Ordering::Relaxed);
}
#[allow(dead_code)]
pub fn set_alpha(a: f64) {
    CORE_ADAPT.alpha_bits.store(a.to_bits(), Ordering::Relaxed);
}

/// Core crossover modifier. Owns the `--crossover` rate (0 = off). Genetic
/// recombination of archived games can reach combinations a single-game
/// destroy/repair can't (the only perturbation lever with a positive signal).
struct CoreCrossover {
    rate_bits: AtomicU64,
}
impl PerturbModifier for CoreCrossover {
    fn crossover_rate(&self) -> f64 {
        let o = self.rate_bits.load(Ordering::Relaxed);
        if o != F64_UNSET {
            let r = f64::from_bits(o);
            if (0.0..=1.0).contains(&r) {
                return r;
            }
        }
        0.0
    }
}
static CORE_CROSSOVER: CoreCrossover = CoreCrossover {
    rate_bits: AtomicU64::new(F64_UNSET),
};

#[allow(dead_code)]
pub fn set_crossover(rate: f64) {
    CORE_CROSSOVER.rate_bits.store(rate.to_bits(), Ordering::Relaxed);
}

// ---- declarative options --------------------------------------------------

/// The type + bounds of a tunable option — drives generic CLI/GUI rendering.
#[derive(Clone, Copy, Debug)]
pub enum OptionKind {
    Toggle {
        default: bool,
    },
    Float {
        default: f64,
        min: f64,
        max: f64,
        step: f64,
    },
    Int {
        default: i64,
        min: i64,
        max: i64,
    },
}

/// Which methods an option applies to.
#[derive(Clone, Copy, Debug)]
pub enum Scope {
    /// Applies to the whole NRPA family (every method that adapts a policy).
    NrpaFamily,
    /// Applies only to the listed method ids.
    Methods(&'static [&'static str]),
}

/// A declarative description of a tunable option. The CLI and GUI render from
/// these (a plugin contributes the specs for the levers it owns).
#[derive(Clone, Copy, Debug)]
pub struct OptionSpec {
    pub key: &'static str,
    pub label_key: &'static str,
    pub help_key: &'static str,
    pub kind: OptionKind,
    pub scope: Scope,
}

/// A plugin: a unit of contribution with dependencies. The core is itself plugins.
pub trait Plugin: Sync {
    fn id(&self) -> &'static str;
    /// Ids of plugins this one requires; it is skipped if any is absent in the build.
    fn deps(&self) -> &'static [&'static str] {
        &[]
    }
    #[allow(dead_code)] // used by the GUI/registry to mark lab-only plugins
    fn experimental(&self) -> bool {
        false
    }
    /// Contribute methods/modifiers/options to the registry.
    fn register(&self, reg: &mut Registry);
}

// ---- core method plugins --------------------------------------------------

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

struct Perturbation;
impl Method for Perturbation {
    fn id(&self) -> &'static str {
        "perturbation"
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
        // run_perturbation — set via `set_crossover` before launching.
        std::thread::spawn(move || nrpa::run_perturbation(search, level, seed_history, variant));
    }
    fn method_desc(&self, ctx: &StartCtx) -> String {
        format!("perturbation L{}", ctx.level)
    }
    fn checkpoint_kind(&self) -> Option<&'static str> {
        Some("perturbation")
    }
}

struct Systematic;
impl Method for Systematic {
    fn id(&self) -> &'static str {
        "systematic"
    }
    fn label_key(&self) -> &'static str {
        "algo-systematic"
    }
    fn spawn(&self, ctx: StartCtx, search: Arc<SearchState>) {
        let initial = ctx.initial;
        std::thread::spawn(move || systematic::run(&initial, search));
    }
    fn method_desc(&self, _ctx: &StartCtx) -> String {
        "systematic".to_owned()
    }
    fn checkpoint_kind(&self) -> Option<&'static str> {
        Some("systematic")
    }
}

struct BeamMethod;
impl Method for BeamMethod {
    fn id(&self) -> &'static str {
        "beam"
    }
    fn label_key(&self) -> &'static str {
        "algo-beam"
    }
    fn spawn(&self, ctx: StartCtx, search: Arc<SearchState>) {
        let StartCtx { initial, width, .. } = ctx;
        std::thread::spawn(move || beam::run(&initial, search, width));
    }
    fn method_desc(&self, ctx: &StartCtx) -> String {
        format!("beam w={}", ctx.width)
    }
    fn checkpoint_kind(&self) -> Option<&'static str> {
        None
    }
}

static NRPA: Nrpa = Nrpa;
static PERTURBATION: Perturbation = Perturbation;
static SYSTEMATIC: Systematic = Systematic;
static BEAM: BeamMethod = BeamMethod;

// Each core method is wrapped in a plugin (no deps). Experimental method/modifier
// plugins append to `all_plugins` under their Cargo feature in later phases.
macro_rules! core_method_plugin {
    ($plugin:ident, $id:literal, $method:expr) => {
        struct $plugin;
        impl Plugin for $plugin {
            fn id(&self) -> &'static str {
                $id
            }
            fn register(&self, reg: &mut Registry) {
                reg.add_method($method);
            }
        }
    };
}
core_method_plugin!(PerturbationPlugin, "perturbation", &PERTURBATION);
core_method_plugin!(SystematicPlugin, "systematic", &SYSTEMATIC);

struct NrpaPlugin;
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
            kind: OptionKind::Int {
                default: 3,
                min: 1,
                max: 6,
            },
            scope: Scope::NrpaFamily,
        });
    }
}

struct BeamPlugin;
impl Plugin for BeamPlugin {
    fn id(&self) -> &'static str {
        "beam"
    }
    fn register(&self, reg: &mut Registry) {
        reg.add_method(&BEAM);
        reg.add_option(OptionSpec {
            key: "width",
            label_key: "opt-width",
            help_key: "opt-width-hint",
            kind: OptionKind::Int {
                default: 64,
                min: 1,
                max: 100_000,
            },
            scope: Scope::Methods(&["beam"]),
        });
    }
}

static NRPA_PLUGIN: NrpaPlugin = NrpaPlugin;
static PERTURBATION_PLUGIN: PerturbationPlugin = PerturbationPlugin;
static SYSTEMATIC_PLUGIN: SystematicPlugin = SystematicPlugin;
static BEAM_PLUGIN: BeamPlugin = BeamPlugin;

// Core modifier plugins (apply to the NRPA family).
struct SymmetryPlugin;
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
            kind: OptionKind::Toggle { default: true },
            scope: Scope::NrpaFamily,
        });
    }
}
struct AdaptPlugin;
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
static SYMMETRY_PLUGIN: SymmetryPlugin = SymmetryPlugin;
static ADAPT_PLUGIN: AdaptPlugin = AdaptPlugin;

// Crossover modifies the perturbation method, so it depends on it: the plugin is
// skipped (and `--crossover` has no effect) in a build without `perturbation`.
struct CrossoverPlugin;
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
static CROSSOVER_PLUGIN: CrossoverPlugin = CrossoverPlugin;

/// Every plugin compiled into this build: the core set, plus experimental ones
/// appended under their feature in later phases.
fn all_plugins() -> Vec<&'static dyn Plugin> {
    #[allow(unused_mut)]
    let mut v: Vec<&'static dyn Plugin> = vec![
        &NRPA_PLUGIN,
        &PERTURBATION_PLUGIN,
        &SYSTEMATIC_PLUGIN,
        &BEAM_PLUGIN,
        &SYMMETRY_PLUGIN,
        &ADAPT_PLUGIN,
        &CROSSOVER_PLUGIN,
    ];
    // e.g. #[cfg(feature = "neural")] v.push(&NEURAL_PLUGIN); — later phases.
    v
}

/// The process-wide registry, built once: plugins are registered in dependency
/// order, and any plugin whose deps are absent in this build is skipped.
pub fn registry() -> &'static Registry {
    static REG: OnceLock<Registry> = OnceLock::new();
    REG.get_or_init(|| {
        use std::collections::HashSet;
        let plugins = all_plugins();
        let present: HashSet<&str> = plugins.iter().map(|p| p.id()).collect();
        let mut reg = Registry::default();
        let mut done: HashSet<&str> = HashSet::new();
        // Drop plugins whose deps aren't compiled in, then register the rest in
        // dependency order (a plugin registers once all its deps have).
        let mut remaining: Vec<&'static dyn Plugin> = plugins
            .into_iter()
            .filter(|p| p.deps().iter().all(|d| present.contains(d)))
            .collect();
        while !remaining.is_empty() {
            let mut progressed = false;
            let mut still: Vec<&'static dyn Plugin> = Vec::new();
            for p in remaining {
                if p.deps().iter().all(|d| done.contains(d)) {
                    p.register(&mut reg);
                    done.insert(p.id());
                    progressed = true;
                } else {
                    still.push(p);
                }
            }
            remaining = still;
            if !progressed {
                break; // unsatisfiable cycle — leave the rest unregistered
            }
        }
        reg
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_methods_registered() {
        let reg = registry();
        for id in ["nrpa", "perturbation", "systematic", "beam"] {
            assert!(reg.method(id).is_some(), "method {id} missing");
        }
        assert!(reg.method("neural-nrpa").is_none(), "no experimental method on a core build");
    }

    #[test]
    fn core_option_specs_present_with_defaults() {
        let opts = registry().options();
        let by = |k: &str| opts.iter().find(|o| o.key == k);
        // The core tuning levers each contribute a spec.
        for k in ["level", "width", "clamp", "alpha", "symmetry", "crossover"] {
            assert!(by(k).is_some(), "option spec {k} missing");
        }
        // Defaults match the engine defaults (clamp C=3, symmetry on, crossover off).
        assert!(matches!(by("clamp").unwrap().kind, OptionKind::Float { default, .. } if default == 3.0));
        assert!(matches!(by("symmetry").unwrap().kind, OptionKind::Toggle { default: true }));
        assert!(matches!(by("crossover").unwrap().kind, OptionKind::Float { default, .. } if default == 0.0));
    }

    #[test]
    fn defaults_match_resolved_hooks() {
        // The registry's resolved hook values equal the spec defaults when unset.
        let reg = registry();
        assert_eq!(reg.clamp(), Some(3.0));
        assert_eq!(reg.alpha(), 1.0);
        assert!(reg.sym_on());
        assert_eq!(reg.crossover_rate(), 0.0);
    }
}
