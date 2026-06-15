# Architecture

The project is a Cargo **virtual workspace** under `crates/`:

| Crate | Role | License |
|-------|------|---------|
| `morpion-solitaire` | GUI + CLI application (library + native binary) | GPL-3.0-or-later |
| `morpion-solitaire-wasm` | WebAssembly entry point — a thin web shell over the app library | GPL-3.0-or-later |
| `morpion-solitaire-record` (`msr`) | the [MSR format](../format/overview.md): reader, writer, validator | MIT OR Apache-2.0 |
| `morpion-solitaire-records` | the embedded record-games corpus | MIT OR Apache-2.0 |

Dependencies flow one way — the app depends on the two libraries, which depend on
neither each other nor the app:

```text
morpion-solitaire-wasm ─▶ morpion-solitaire ─┬─▶ morpion-solitaire-record (msr)
                                             └─▶ morpion-solitaire-records ─▶ (msr, dev-only)
```

The permissive libraries are GPLv3-compatible, so the GPL application may depend
on them. Keeping the format crate **free of the solver's `GameState`/bitboard**
is deliberate: any other tool can read, write and validate MSR records with it
standalone.

## Inside the application

- **`game`** — the core model: a bitboard `Board` (one `Row` word per grid row,
  O(1) `contains`; see [the fixed-grid board](board.md)), `Line`/`Move`, the
  `GameState` and rules, and `io` — conversion to/from `msr::Record`, the search
  checkpoint codec (`MSC1:`), the Pentasol bridge, and the PNG `tEXt` / SVG
  `<metadata>` record embedding shared by the GUI and `cli show`.
- **`search`** — the solvers (see [algorithms](algorithms.md)): NRPA and the
  perturbation search, the exhaustive systematic search, beam, plus the shared
  `SearchState` (atomics for best score / nodes / running / paused / exhausted),
  symmetry, bounds, and checkpointing.
- **`render`** — game → SVG, and via `resvg`/`tiny-skia` → PNG; the embed/extract
  helpers are pure and cross-platform, so the web build can *read* an embedded
  record even though it can't rasterise one.
- **`ui`** — the egui board view and side-panel controls (painter-drawn icons,
  themeable light/dark board).
- **`i18n`** — Fluent catalogues (8 locales); `app` and `cli` both localise.
- **`cli`** — the headless command line (native only).

## How a search runs

The solver runs on a background thread (or a `rayon` pool for the systematic
search) and communicates only through the lock-free `SearchState`. The UI/CLI
polls it each tick: it reads the best sequence to show a live preview, the node
rate, and the `exhausted` flag (which, for the systematic search, means the best
score is the **proven optimum**). The move history (`Vec<Move>`, absolute
coordinates) is the single source of truth — the bitboard is a reconstructible
projection — so a game survives a grid resize and a search can be checkpointed
and resumed.
