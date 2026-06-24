//! Plugin framework for search methods, modifiers & options (`docs/plugin-framework.md`).
//!
//! A **plugin** is a generic contribution unit (a method, a modifier of a search at a
//! named hook, an option, UI) with **dependencies** on other plugins. The core itself
//! is expressed as plugins; experimental ones register only under their Cargo feature.
//! The CLI and GUI dispatch through the [`Registry`] and name no specific plugin.
//!
//! In place: the `Plugin`/`Registry`/`Method` scaffolding + core method plugins; the
//! `CodingModifier`/`AdaptModifier`/`PerturbModifier` hooks (presence markers) resolved
//! once per search into a scalar — no per-node cost; declarative [`OptionSpec`]s; and the
//! **values map** ([`OptionValue`] per key) that is the single source of truth the CLI
//! and GUI write and the engine reads. The dynamic CLI and generic GUI both render from
//! the specs and push values through [`Registry::set_value`] — naming no specific plugin.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

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

/// A tunable option's live value. The CLI/GUI write these (parsed from a flag or a
/// widget) into the registry's values map; the engine reads them back at search start.
/// `Serialize`/`Deserialize` let the GUI persist them across launches.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum OptionValue {
    Toggle(bool),
    Float(f64),
    Int(i64),
}

impl OptionValue {
    pub fn as_bool(self) -> Option<bool> {
        match self {
            OptionValue::Toggle(b) => Some(b),
            _ => None,
        }
    }
    pub fn as_f64(self) -> Option<f64> {
        match self {
            OptionValue::Float(f) => Some(f),
            OptionValue::Int(i) => Some(i as f64),
            OptionValue::Toggle(_) => None,
        }
    }
    pub fn as_int(self) -> Option<i64> {
        match self {
            OptionValue::Int(i) => Some(i),
            _ => None,
        }
    }
}

/// Where plugins register their contributions: methods, modifier hooks, option specs,
/// and the [`values`](Registry::set_value) map seeded from those specs' defaults.
#[derive(Default)]
pub struct Registry {
    methods: Vec<&'static dyn Method>,
    bias: Option<&'static dyn BiasModifier>,
    coding: Option<&'static dyn CodingModifier>,
    adapt: Option<&'static dyn AdaptModifier>,
    perturb: Option<&'static dyn PerturbModifier>,
    options: Vec<OptionSpec>,
    /// Live option values, keyed by [`OptionSpec::key`]. Seeded with each spec's
    /// default as the plugin registers; overwritten by the CLI/GUI before a search.
    values: Mutex<HashMap<&'static str, OptionValue>>,
}

impl Registry {
    pub fn add_method(&mut self, m: &'static dyn Method) {
        self.methods.push(m);
    }
    #[allow(dead_code)] // used by the neural plugin (feature-gated)
    pub fn add_bias(&mut self, m: &'static dyn BiasModifier) {
        self.bias = Some(m);
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
        // Seed the live value with the spec's default (no lock needed during build).
        self.values
            .get_mut()
            .unwrap()
            .insert(spec.key, spec.kind.default_value());
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
    /// The active move-bias modifier (e.g. a neural prior), if any plugin contributed
    /// one. Resolved once at search start; the hot loop branches on the `Option`, so a
    /// core build (no bias plugin) pays nothing.
    pub fn bias_modifier(&self) -> Option<&'static dyn BiasModifier> {
        self.bias
    }

    // ---- the values map (single source of truth) --------------------------

    /// Set an option's live value (CLI/GUI). A key with no registered spec — or a value
    /// whose variant doesn't match the option's declared kind (e.g. a stale persisted
    /// value after a spec change) — is a no-op, so the map can't be poisoned.
    pub fn set_value(&self, key: &str, val: OptionValue) {
        let ok = self.options.iter().any(|s| s.key == key && s.kind.accepts(val));
        if ok {
            if let Some(slot) = self.values.lock().unwrap().get_mut(key) {
                *slot = val;
            }
        }
    }
    /// The current value for `key`, or `None` if no plugin registered that option.
    pub fn value(&self, key: &str) -> Option<OptionValue> {
        self.values.lock().unwrap().get(key).copied()
    }
    pub fn value_bool(&self, key: &str, default: bool) -> bool {
        self.value(key).and_then(OptionValue::as_bool).unwrap_or(default)
    }
    pub fn value_f64(&self, key: &str, default: f64) -> f64 {
        self.value(key).and_then(OptionValue::as_f64).unwrap_or(default)
    }
    pub fn value_int(&self, key: &str, default: i64) -> i64 {
        self.value(key).and_then(OptionValue::as_int).unwrap_or(default)
    }

    // ---- hooks resolved once per search into a scalar ---------------------
    //
    // Each reads the values map, gated by whether the owning modifier plugin is
    // compiled in. Defaults match plain NRPA when the modifier is absent. Read once
    // at search start into a local — the hot loop pays nothing new.

    pub fn sym_on(&self) -> bool {
        if self.coding.is_some() {
            self.value_bool("symmetry", true)
        } else {
            true
        }
    }
    pub fn clamp(&self) -> Option<f64> {
        if self.adapt.is_some() {
            let c = self.value_f64("clamp", 3.0);
            (c > 0.0).then_some(c)
        } else {
            Some(3.0)
        }
    }
    pub fn alpha(&self) -> f64 {
        if self.adapt.is_some() {
            let a = self.value_f64("alpha", 1.0);
            if a > 0.0 {
                a
            } else {
                1.0
            }
        } else {
            1.0
        }
    }
    /// Perturbation crossover rate (0 = off) — read once per perturbation round.
    pub fn crossover_rate(&self) -> f64 {
        if self.perturb.is_some() {
            let r = self.value_f64("crossover", 0.0);
            if (0.0..=1.0).contains(&r) {
                r
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    /// NRPA nesting level (the whole NRPA family), clamped to the spec's range.
    pub fn level(&self) -> usize {
        self.value_int("level", 3).clamp(1, 6) as usize
    }
    /// Beam width, at least 1.
    pub fn width(&self) -> usize {
        self.value_int("width", 64).max(1) as usize
    }
}

/// Process-wide convenience: push a value into the global registry's map.
#[allow(dead_code)] // used by the CLI/GUI
pub fn set_option(key: &str, val: OptionValue) {
    registry().set_value(key, val);
}

// ---- modifier hooks (presence markers) ------------------------------------
//
// State now lives in the registry's values map; a modifier trait marks that a plugin
// owns a hook and is compiled into this build. The registry resolves the hook from
// the map, gated by the slot's presence.

/// Move-bias hook: a per-move, log-space bias added into the NRPA softmax (β in the
/// policy logit). A neural prior is the canonical implementor. Called once per state
/// per playout/adapt step on every island thread, so it must be cheap (or internally
/// cached) and must never panic — fill `out` with one bias per move in `moves` order,
/// or leave it cleared (treated as all-zero) on error.
pub trait BiasModifier: Sync {
    /// Whether the modifier is currently armed. Resolved once at search start: when
    /// false the engine takes its no-bias fast path, so a registered-but-unarmed
    /// modifier (e.g. the neural plugin with no prior loaded) costs nothing.
    fn active(&self) -> bool {
        true
    }
    fn biases(&self, state: &GameState, moves: &[Move], out: &mut Vec<f64>);
}

/// Move-coding hook: symmetry-invariant (canonical D4) coding vs the identity frame.
pub trait CodingModifier: Sync {}
/// Adapt hook: the policy-update hyperparameters — logit clamp C and step α.
pub trait AdaptModifier: Sync {}
/// Perturbation-round hook: probability a round recombines two archived games
/// (`crossover_games`) instead of destroy/repair of one.
pub trait PerturbModifier: Sync {}

/// Core symmetry modifier (owns `--symmetry`/`--no-symmetry`). On (default): canonical
/// D4 coding, all 8 Zobrist hashes maintained. Off: identity frame only (one hash),
/// ~+16% throughput at neutral score — for cold record runs.
struct CoreCoding;
impl CodingModifier for CoreCoding {}
static CORE_CODING: CoreCoding = CoreCoding;

/// Core adapt modifier (owns `--clamp`/`--alpha`). The logit clamp (Stabilized-NRPA)
/// is on by default at C=3 (tight sweet spot; 5T L4/120 s ~112 vs ~95 unclamped);
/// `--clamp 0` disables. α default 1.0 (0.5/2.0 regressed unclamped).
struct CoreAdapt;
impl AdaptModifier for CoreAdapt {}
static CORE_ADAPT: CoreAdapt = CoreAdapt;

/// Core crossover modifier (owns `--crossover`, 0 = off). Genetic recombination of
/// archived games can reach combinations a single-game destroy/repair can't (the only
/// perturbation lever with a positive signal).
struct CoreCrossover;
impl PerturbModifier for CoreCrossover {}
static CORE_CROSSOVER: CoreCrossover = CoreCrossover;

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

impl OptionKind {
    /// The spec's default as a live [`OptionValue`] (used to seed the values map).
    fn default_value(self) -> OptionValue {
        match self {
            OptionKind::Toggle { default } => OptionValue::Toggle(default),
            OptionKind::Float { default, .. } => OptionValue::Float(default),
            OptionKind::Int { default, .. } => OptionValue::Int(default),
        }
    }
    /// Whether `val`'s variant matches this kind (a Float kind accepts Int too, since an
    /// integer is a valid float value).
    fn accepts(self, val: OptionValue) -> bool {
        matches!(
            (self, val),
            (OptionKind::Toggle { .. }, OptionValue::Toggle(_))
                | (OptionKind::Float { .. }, OptionValue::Float(_) | OptionValue::Int(_))
                | (OptionKind::Int { .. }, OptionValue::Int(_))
        )
    }
}

/// Which methods an option applies to.
#[derive(Clone, Copy, Debug)]
pub enum Scope {
    /// Applies to the whole NRPA family (every method that adapts a policy).
    NrpaFamily,
    /// Applies only to the listed method ids.
    Methods(&'static [&'static str]),
}

impl Scope {
    /// Whether this option is relevant to the method with id `method_id`. The NRPA
    /// family is nrpa + perturbation (perturbation drives inner NRPA searches).
    pub fn applies_to(self, method_id: &str) -> bool {
        match self {
            Scope::NrpaFamily => matches!(method_id, "nrpa" | "perturbation"),
            Scope::Methods(ids) => ids.contains(&method_id),
        }
    }
}

/// A declarative description of a tunable option. The CLI and GUI render from
/// these (a plugin contributes the specs for the levers it owns).
#[derive(Clone, Copy, Debug)]
pub struct OptionSpec {
    pub key: &'static str,
    /// GUI label (fluent i18n key).
    pub label_key: &'static str,
    /// GUI tooltip (fluent i18n key).
    pub help_key: &'static str,
    /// CLI help text. English — the CLI is the project's English-only surface, so its
    /// `--help` carries the canonical English description while the GUI translates via
    /// `help_key`.
    pub help: &'static str,
    pub kind: OptionKind,
    pub scope: Scope,
}

impl OptionSpec {
    /// The clap flag name for this option. A toggle defaulting *on* becomes a
    /// `--no-<key>` opt-out (preserving e.g. `--no-symmetry`); everything else is
    /// `--<key>`.
    pub fn cli_flag(&self) -> String {
        match self.kind {
            OptionKind::Toggle { default: true } => format!("no-{}", self.key),
            _ => self.key.to_owned(),
        }
    }
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

// Perturbation drives time-bounded inner NRPA searches via OS threads
// (`nrpa::run_perturbation`), which is native-only — so the whole method plugin is
// gated off wasm. The crossover modifier depends on it, so on wasm the dependency
// resolver drops crossover too (a nice end-to-end check of the dep mechanism).
#[cfg(not(target_arch = "wasm32"))]
struct Perturbation;
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
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
            help: "Beam width (kept candidates per depth).",
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
#[cfg(not(target_arch = "wasm32"))]
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
            help: "Drop symmetry-invariant move coding (identity frame only): ~+16% \
                   throughput at neutral score — recommended for cold record runs \
                   without warm-start. (The flag is `--no-symmetry`.)",
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
static CROSSOVER_PLUGIN: CrossoverPlugin = CrossoverPlugin;

/// Every plugin compiled into this build: the core set, plus experimental ones
/// appended under their feature in later phases.
fn all_plugins() -> Vec<&'static dyn Plugin> {
    #[allow(unused_mut)]
    let mut v: Vec<&'static dyn Plugin> = vec![
        &NRPA_PLUGIN,
        &SYSTEMATIC_PLUGIN,
        &BEAM_PLUGIN,
        &SYMMETRY_PLUGIN,
        &ADAPT_PLUGIN,
        // Crossover declares a dependency on "perturbation"; on wasm (where perturbation
        // is absent) the resolver drops it automatically.
        &CROSSOVER_PLUGIN,
    ];
    // Perturbation is native-only (OS threads); on wasm it isn't compiled in.
    #[cfg(not(target_arch = "wasm32"))]
    v.push(&PERTURBATION_PLUGIN);
    // The experimental neural prior (feature `neural`, native-only): a BiasModifier
    // depending on nrpa. Absent ⇒ no bias hook, no --neural-scale option.
    #[cfg(all(feature = "neural", not(target_arch = "wasm32")))]
    v.push(&crate::search::neural::NEURAL_PLUGIN);
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

    // Tests that read or mutate the process-global registry values must not run
    // concurrently (the values map is shared, and Rust runs tests in parallel).
    static REG_TEST_LOCK: Mutex<()> = Mutex::new(());

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
        let _g = REG_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // The registry's resolved hook values equal the spec defaults when unset.
        let reg = registry();
        assert_eq!(reg.clamp(), Some(3.0));
        assert_eq!(reg.alpha(), 1.0);
        assert!(reg.sym_on());
        assert_eq!(reg.crossover_rate(), 0.0);
        assert_eq!(reg.level(), 3);
        assert_eq!(reg.width(), 64);
    }

    #[test]
    fn scope_membership() {
        // NRPA-family options reach nrpa + perturbation; method-scoped ones only their
        // method.
        assert!(Scope::NrpaFamily.applies_to("nrpa"));
        assert!(Scope::NrpaFamily.applies_to("perturbation"));
        assert!(!Scope::NrpaFamily.applies_to("beam"));
        assert!(Scope::Methods(&["beam"]).applies_to("beam"));
        assert!(!Scope::Methods(&["beam"]).applies_to("nrpa"));
    }

    #[test]
    fn set_value_round_trips_through_hooks() {
        let _g = REG_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Writing the map changes what the hooks resolve; restore defaults after so
        // the process-global registry isn't left perturbed for other tests.
        let reg = registry();
        reg.set_value("clamp", OptionValue::Float(0.0));
        assert_eq!(reg.clamp(), None, "clamp 0 disables clamping");
        reg.set_value("symmetry", OptionValue::Toggle(false));
        assert!(!reg.sym_on());
        reg.set_value("crossover", OptionValue::Float(0.25));
        assert_eq!(reg.crossover_rate(), 0.25);
        // restore
        reg.set_value("clamp", OptionValue::Float(3.0));
        reg.set_value("symmetry", OptionValue::Toggle(true));
        reg.set_value("crossover", OptionValue::Float(0.0));
    }
}
