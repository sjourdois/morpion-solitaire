//! Plugin framework for search methods, modifiers & options (`docs/plugin-framework.md`).
//!
//! A **plugin** is a generic contribution unit (a method, a modifier of a search at a
//! named hook, an option, UI) with **dependencies** on other plugins. The core itself
//! is expressed as plugins; experimental ones register only under their Cargo feature.
//! The CLI and GUI dispatch through the [`Registry`] and name no specific plugin.
//!
//! **Phase 1** (this file): the `Plugin`/`Registry`/`Method` scaffolding + the four core
//! method plugins. Hooks/modifiers and option specs arrive in later phases.

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
    /// Perturbation crossover rate (becomes a `PerturbModifier` in a later phase).
    pub crossover: f64,
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

/// Where plugins register their contributions. Phase 1 holds methods only; modifier
/// hooks and option specs are added in later phases.
#[derive(Default)]
pub struct Registry {
    methods: Vec<&'static dyn Method>,
}

impl Registry {
    pub fn add_method(&mut self, m: &'static dyn Method) {
        self.methods.push(m);
    }
    /// All registered methods, in registration (dependency) order.
    pub fn methods(&self) -> &[&'static dyn Method] {
        &self.methods
    }
    /// Look up a method by its id (the CLI `--algo` value).
    pub fn method(&self, id: &str) -> Option<&'static dyn Method> {
        self.methods.iter().copied().find(|m| m.id() == id)
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
            crossover,
            ..
        } = ctx;
        std::thread::spawn(move || {
            // The crossover rate is a per-thread override; set it on the loop's thread.
            nrpa::set_crossover_override(crossover);
            nrpa::run_perturbation(search, level, seed_history, variant);
        });
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
core_method_plugin!(NrpaPlugin, "nrpa", &NRPA);
core_method_plugin!(PerturbationPlugin, "perturbation", &PERTURBATION);
core_method_plugin!(SystematicPlugin, "systematic", &SYSTEMATIC);
core_method_plugin!(BeamPlugin, "beam", &BEAM);

static NRPA_PLUGIN: NrpaPlugin = NrpaPlugin;
static PERTURBATION_PLUGIN: PerturbationPlugin = PerturbationPlugin;
static SYSTEMATIC_PLUGIN: SystematicPlugin = SystematicPlugin;
static BEAM_PLUGIN: BeamPlugin = BeamPlugin;

/// Every plugin compiled into this build: the core set, plus experimental ones
/// appended under their feature in later phases.
fn all_plugins() -> Vec<&'static dyn Plugin> {
    #[allow(unused_mut)]
    let mut v: Vec<&'static dyn Plugin> = vec![
        &NRPA_PLUGIN,
        &PERTURBATION_PLUGIN,
        &SYSTEMATIC_PLUGIN,
        &BEAM_PLUGIN,
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
