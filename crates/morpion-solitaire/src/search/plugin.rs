//! Plugin framework for search methods, modifiers & options (`docs/plugin-framework.md`).
//!
//! A **plugin** is a generic contribution unit (a method, a modifier of a search at a
//! named hook, an option, UI) with **dependencies** on other plugins. The core itself
//! is expressed as plugins, split logically by method and modifier under this module:
//! one file per plugin (`nrpa/`, `perturbation/`, `systematic`, `beam`, `puct/`), with a
//! method's modifiers as its submodules (e.g. `nrpa/{symmetry,adapt,macros,…}`). The CLI
//! and GUI dispatch through the [`Registry`] and name no specific plugin.
//!
//! The framework itself lives here: the `Plugin`/`Method`/`*Modifier` traits, the
//! hooks (presence markers resolved once per search into a scalar — no per-node cost),
//! declarative [`OptionSpec`]s, and the **values map** ([`OptionValue`] per key) — the
//! single source of truth the CLI/GUI write and the engine reads. Adding a plugin is one
//! file plus a line in `all_plugins`.

// This module is the plugin *framework* only (traits, the [`Registry`], declarative
// options, the experimental gate, the hooks). Each plugin's *registration* lives with
// its engine: `search::beam`, `search::systematic`,
// `search::nrpa::{plugin,perturbation,macros}`, `search::neural::plugin`. [`all_plugins`]
// gathers them.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::game::{moves::Move, rules::Variant, state::GameState};

use super::SearchState;

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
    /// The id of the method this is a **variant** of, if any — e.g. perturbation is a
    /// large-neighbourhood variant of `nrpa`, not a standalone engine. The GUI renders
    /// root methods (`None`) as engine tabs and variants as a toggle within the parent's
    /// tab; `None` (the default) is a top-level engine.
    fn parent(&self) -> Option<&'static str> {
        None
    }
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
    /// Method ids / option keys contributed by an [`experimental`](Plugin::experimental)
    /// plugin. Tagged by [`add_method`](Self::add_method)/[`add_option`](Self::add_option)
    /// as the plugin registers (see `building_experimental`), then read by the CLI/GUI to
    /// hide lab-only surface unless `--experimental` is set. The plugins still register —
    /// the engine is built once; this is a visibility gate only.
    experimental_methods: HashSet<&'static str>,
    experimental_options: HashSet<&'static str>,
    /// Set by the build loop to the currently-registering plugin's `experimental()` flag,
    /// so `add_method`/`add_option` can tag what it contributes directly — robust to a
    /// plugin's registration order, unlike diffing the vec lengths around `register()`.
    building_experimental: bool,
}

impl Registry {
    pub fn add_method(&mut self, m: &'static dyn Method) {
        if self.building_experimental {
            self.experimental_methods.insert(m.id());
        }
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
        if self.building_experimental {
            self.experimental_options.insert(spec.key);
        }
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

    // ---- experimental visibility ------------------------------------------
    //
    // An experimental plugin's methods/options are tagged at build time; the CLI/GUI
    // ask whether each is *visible* (core always; experimental only under the global
    // `--experimental` flag). The plugins are still registered, so the engine itself is
    // unchanged — this only controls what the user surfaces expose.

    /// Whether method `id` was contributed by an experimental plugin.
    pub fn is_method_experimental(&self, id: &str) -> bool {
        self.experimental_methods.contains(id)
    }
    /// Whether option `key` was contributed by an experimental plugin.
    pub fn is_option_experimental(&self, key: &str) -> bool {
        self.experimental_options.contains(key)
    }
    /// Whether method `id` should be exposed in the current run: always for core
    /// methods, only under `--experimental` for lab-only ones.
    pub fn method_visible(&self, id: &str) -> bool {
        !self.is_method_experimental(id) || experimental_enabled()
    }
    /// Whether option `key` should be exposed in the current run (see [`method_visible`](Self::method_visible)).
    pub fn option_visible(&self, key: &str) -> bool {
        !self.is_option_experimental(key) || experimental_enabled()
    }

    // ---- the values map (single source of truth) --------------------------

    /// Set an option's live value (CLI/GUI). A key with no registered spec — or a value
    /// whose variant doesn't match the option's declared kind (e.g. a stale persisted
    /// value after a spec change) — is a no-op, so the map can't be poisoned.
    pub fn set_value(&self, key: &str, val: OptionValue) {
        let ok = self
            .options
            .iter()
            .any(|s| s.key == key && s.kind.accepts(val));
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

    /// Reset every experimental option's live value to its spec default. The GUI calls
    /// this when it disables the experimental surface, so a lab-only knob set while it
    /// was on stops taking effect — the engine reads the values map directly, and the
    /// CLI/GUI no longer expose the option to change it back.
    pub fn reset_experimental_values(&self) {
        let mut vals = self.values.lock().unwrap();
        for spec in &self.options {
            if self.experimental_options.contains(spec.key) {
                vals.insert(spec.key, spec.kind.default_value());
            }
        }
    }
    pub fn value_bool(&self, key: &str, default: bool) -> bool {
        self.value(key)
            .and_then(OptionValue::as_bool)
            .unwrap_or(default)
    }
    pub fn value_f64(&self, key: &str, default: f64) -> f64 {
        self.value(key)
            .and_then(OptionValue::as_f64)
            .unwrap_or(default)
    }
    pub fn value_int(&self, key: &str, default: i64) -> i64 {
        self.value(key)
            .and_then(OptionValue::as_int)
            .unwrap_or(default)
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

/// Whether experimental (lab-only) methods & options are surfaced this run. Off by
/// default; the CLI `--experimental` flag and the GUI toggle flip it.
static EXPERIMENTAL: AtomicBool = AtomicBool::new(false);

/// Enable/disable the experimental surface (CLI flag / GUI toggle).
#[allow(dead_code)] // used by the CLI/GUI
pub fn set_experimental(on: bool) {
    EXPERIMENTAL.store(on, Ordering::Relaxed);
}
/// Whether the experimental surface is currently enabled.
pub fn experimental_enabled() -> bool {
    EXPERIMENTAL.load(Ordering::Relaxed)
}

// ---- modifier hooks (presence markers) ------------------------------------
//
// State lives in the registry's values map; a modifier trait marks that a plugin owns
// a hook and is compiled into this build. The registry resolves the hook from the map,
// gated by the slot's presence.

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
                | (
                    OptionKind::Float { .. },
                    OptionValue::Float(_) | OptionValue::Int(_)
                )
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
    /// Whether this plugin's methods & options are lab-only — surfaced by the CLI/GUI
    /// only under `--experimental`. The registry tags what it contributes (see
    /// [`Registry::method_visible`]); the plugin still registers either way.
    fn experimental(&self) -> bool {
        false
    }
    /// Contribute methods/modifiers/options to the registry.
    fn register(&self, reg: &mut Registry);
}

/// Every plugin compiled into this build, in a sensible registration order (the
/// dependency resolver in [`registry`] re-orders as needed). Each plugin lives with its
/// engine module; this is the one place that enumerates them. Adding a plugin = one new
/// `*_PLUGIN` static next to its engine, plus one line here.
fn all_plugins() -> Vec<&'static dyn Plugin> {
    use crate::search::{beam, nrpa, systematic};
    #[allow(unused_mut)]
    let mut v: Vec<&'static dyn Plugin> = vec![
        &nrpa::plugin::NRPA_PLUGIN,
        &systematic::SYSTEMATIC_PLUGIN,
        &beam::BEAM_PLUGIN,
        &nrpa::plugin::SYMMETRY_PLUGIN,
        &nrpa::plugin::ADAPT_PLUGIN,
    ];
    // Perturbation (OS threads) + its crossover modifier + macros are native-only.
    #[cfg(not(target_arch = "wasm32"))]
    {
        v.push(&nrpa::perturbation::PERTURBATION_PLUGIN);
        v.push(&nrpa::perturbation::CROSSOVER_PLUGIN);
        v.push(&nrpa::macros::MACROS_PLUGIN);
    }
    // The neural prior + feature-space head + PUCT (feature `neural`, native-only).
    #[cfg(all(feature = "neural", not(target_arch = "wasm32")))]
    {
        use crate::search::neural;
        v.push(&neural::plugin::NEURAL_BIAS_PLUGIN);
        v.push(&neural::plugin::FEATURE_SPACE_PLUGIN);
        v.push(&neural::plugin::PUCT_PLUGIN);
    }
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
                    // Tag whatever this plugin registers as experimental (or not) at the
                    // point of add_method/add_option — order-independent, no len-diffing.
                    reg.building_experimental = p.experimental();
                    p.register(&mut reg);
                    reg.building_experimental = false;
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
        assert!(
            reg.method("neural-nrpa").is_none(),
            "no experimental method on a core build"
        );
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
        assert!(
            matches!(by("clamp").unwrap().kind, OptionKind::Float { default, .. } if default == 3.0)
        );
        assert!(matches!(
            by("symmetry").unwrap().kind,
            OptionKind::Toggle { default: true }
        ));
        assert!(
            matches!(by("crossover").unwrap().kind, OptionKind::Float { default, .. } if default == 0.0)
        );
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
    fn core_surface_is_never_experimental() {
        let reg = registry();
        // Core methods and tuning levers are always visible, regardless of the flag.
        for id in ["nrpa", "perturbation", "systematic", "beam"] {
            assert!(
                !reg.is_method_experimental(id),
                "{id} wrongly tagged experimental"
            );
            assert!(reg.method_visible(id), "{id} should always be visible");
        }
        for k in ["level", "width", "clamp", "alpha", "symmetry", "crossover"] {
            assert!(
                !reg.is_option_experimental(k),
                "{k} wrongly tagged experimental"
            );
        }
    }

    // The experimental plugins (macros, neural, feature-space, PUCT) compile only with
    // the `neural` feature, on native; check their surface is tagged & gated there.
    #[cfg(all(feature = "neural", not(target_arch = "wasm32")))]
    #[test]
    fn experimental_surface_is_tagged_and_gated() {
        let _g = REG_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let reg = registry();
        assert!(
            reg.is_method_experimental("puct"),
            "puct should be tagged experimental"
        );
        assert!(
            reg.is_option_experimental("macros"),
            "macros should be tagged experimental"
        );
        // Default (flag off): hidden.
        set_experimental(false);
        assert!(!reg.method_visible("puct"));
        assert!(!reg.option_visible("macros"));
        // Flag on: visible.
        set_experimental(true);
        assert!(reg.method_visible("puct"));
        assert!(reg.option_visible("macros"));
        set_experimental(false); // restore for other tests
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
