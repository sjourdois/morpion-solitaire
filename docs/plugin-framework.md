# Plugin framework for search methods, modifiers & options

Goal: the public `main` build ships the **core** methods + modifiers + options; **experimental**
ones (neural prior, PUCT, value-net, φ-space, macros) are plugins compiled only with
`--features experimental`. The CLI and GUI are **generic over a registry** — they name no specific
plugin — so the GUI revamp lives on `main` experimental-free *by construction*, and adding a future
method/modifier/option is purely additive (one plugin module).

## Validated decisions (2026-06-24, with the author)

- **A plugin is a generic contribution unit**: it may contribute a **method**, a **modifier** of a
  search (at a named hook), an **option**, and/or **UI** — and declares **dependencies** on other
  plugins. The **core itself is expressed as plugins** (nrpa, clamp, symmetry, crossover, …);
  experiments are plugins that **depend on** core plugins.
- **Extension = a fixed set of named hooks** (not a general middleware). Zero-cost when inactive:
  the active modifier per hook is resolved **once per search** into a local, exactly like today's
  `sym_on`/`prior`/`clamp`/`alpha` — so the hot loop pays nothing new.
- **Plugins are activated via Cargo features** — potentially **one feature per plugin**
  (`neural`, `puct`, `macros`, …), optionally under an `experimental` umbrella that enables the
  whole lab set. Default features = core plugins only (public build). The registry assembles
  whatever is compiled in; each plugin's registration is `#[cfg(feature = "…")]`.
- **The GUI overlay + dashboard are CORE** (always present), not plugins. They render
  **generically** from the registry: engine tabs ← registered methods; option widgets ← option
  specs; live-games/gauge/sparkline are core. Plugins **fill** this frame — a plugin contributes
  its options (and optionally a custom panel); the neural prior UI becomes such a contribution,
  not core code.
- **Dynamic CLI**: the clap `Command` is built at runtime from the option registry.
- **Migration: methods + modifiers.** Core becomes method-plugins (nrpa/perturbation/systematic/
  beam) + modifier-plugins (clamp/symmetry/crossover) with their options — validating the
  abstraction on known-good code before porting experiments.

## Core types (sketch)

```rust
trait Plugin: Sync {
    fn id(&self) -> &'static str;
    fn deps(&self) -> &'static [&'static str] { &[] }
    fn experimental(&self) -> bool { false }
    fn register(&self, reg: &mut Registry);  // contribute methods/modifiers/options
}

// Fixed hooks. Each has its OWN typed trait + registry slot (signatures differ),
// so "named hooks" stays concrete and zero-cost.
trait Method: Sync { fn id(&self) -> &str; fn spawn(&self, ctx: StartCtx, st: Arc<SearchState>); … }
trait BiasModifier: Sync { fn bias(&self, st: &GameState, pos: Pos) -> f64; }     // β in the softmax
trait CodingModifier: Sync { fn code(&self, …) -> u64; }                          // move coding
trait AdaptModifier: Sync { fn clamp(&self) -> Option<f64>; fn alpha(&self) -> f64; } // adapt step
trait ActionModifier: Sync { … }                                                 // action space (macros)
trait PerturbModifier: Sync { … }                                                // perturbation round (crossover)

struct OptionSpec { key, label_key, help_key, kind, scope }   // GUI/CLI render from these
```

`SearchConfig` = typed core fields + a `values: Map<key, OptionValue>` for plugin options. A
modifier reads its option values to decide whether it's active and how strong.

**Resolution & throughput.** At search start the engine asks the registry for the active modifier
on each hook (filtered by the config) and stores it in a local (`Option<&dyn BiasModifier>`, etc.).
The hot loop branches on the `Option` — identical to the current fast paths. No per-node lookup.

**Dependencies.** Plugins register in dependency order; a plugin is included iff its deps are
present in the build. A modifier names the method(s) it targets (e.g. crossover → perturbation).
Two modifiers contending for one exclusive hook resolve by declared priority (or error).

**Composition/wrapping.** Some methods use others (perturbation runs NRPA inside; tabula-rasa wraps
perturbation). That's just a method-plugin calling another engine's code — no special abstraction.

## Phases (each builds green + commits; default behaviour byte-for-byte unchanged)

1. **Plugin/Registry/Method scaffolding** + the 4 core method-plugins; dispatch (cli spawn/bench/
   checkpoint) goes through the registry. clamp/symmetry/crossover still hardcoded.
2. **Hooks** (Bias/Coding/Adapt traits + slots); engine resolves them once at start; migrate
   **clamp + symmetry** to core modifier-plugins reading their options.
3. **PerturbModifier hook** + **crossover** as a modifier-plugin (dep: perturbation).
4. **Options framework + dynamic CLI** + GUI generic rendering (the revamp, experimental-free).
5. **Experimental plugins** (`#[cfg(feature="experimental")]`): port neural prior / PUCT / etc.
   from the `neural-guide` archive onto the hooks; validated with `--features experimental`.

Built on `plugin-framework` (off `engine-improvements`); merges to the main-bound line when proven.
`neural-guide` stays the immutable archive (source for phase 5).
