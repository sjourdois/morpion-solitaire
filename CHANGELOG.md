# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] — unreleased

### Added

- **Pluggable search architecture.** Search methods, their modifiers, and every
  tuning option are now declared in a registry; the CLI flags and the GUI controls
  are generated from it, so adding a lever is one self-contained plugin. The GUI
  gains a redesigned **search-setup overlay** — engine tabs, per-engine options, a
  live equivalent-CLI line — driven entirely by that registry.
- **Headless CLI search operations.** Engine-tuning flags
  (`--clamp`/`--alpha`/`--no-symmetry`/`--kmin`/`--kmax`/`--window`/`--crossover`)
  and operational options for long runs (`--run-dir`, `--progress-log`,
  `--max-memory`, `--nice`, `--checkpoint-interval`/`--resume`, `--ignore-overflow`).
- **Genetic crossover** for perturbation search (`--crossover`).
- **`--experimental` flag** (and a matching GUI toggle) gating a second tier of
  lab-only engines and options. Off by default; the standard surface is unchanged.
- **Neural search** — an optional `neural` build (native-only), all behind
  `--experimental`: a neural **move prior** biasing NRPA (`--prior
  bundled|corpus|scratch|FILE`, `--neural-scale`); from-scratch training via the
  `tabula-rasa` command; **feature-space NRPA** (`--feat-adapt` and variants);
  **macro-actions** (`--macros`); and **PUCT** policy+value tree search
  (`--algo puct`, `--value-net`) with a `train-value` command.

### Changed

- **NRPA throughput ≈ ×3.6** via exact, behaviour-preserving changes: incremental
  move generation, a symmetry-off fast path, and a cold-policy fast path.

## [0.1.5] — 2026-06-17

### Changed

- **Systematic search throughput ≈ 2.5× faster** (75.5M → 186M nodes/s on a
  32-thread host; 240 → 83.5 ns/node single-thread), via exact, behaviour-
  preserving hot-loop changes: the trace-canonical flag is now carried with the
  legal set and updated in O(1) per move (eliminating `canonical_ok`'s O(depth)
  history scan from the hot loop — it runs only at chunk roots); a carried move's
  conflict check is a direct two-line test instead of an index lookup; the
  horizontal occupancy strip in the incremental move generator is read as one
  row shift+mask; and frame buffers are recycled through a pool.
- The systematic worker pool is capped to the **physical**-core count on hybrid /
  SMT hosts (HyperThreading and Intel E-cores add little to this compute-bound
  work), keeping throughput while freeing logical cores for the UI.

### Added

- Experimental NRPA tuning knobs for offline search-quality campaigns, **all off
  by default** (no change to normal `nrpa` / `perturbation` runs): per-level
  iteration count, adaptation step, a corpus-learned local move prior, a
  logit-clamp portfolio across islands, perturbation destroy-size / repair
  warm-start controls, and self-improving warm restarts.

## [0.1.4] — 2026-06-16

Maintenance release: same application as 0.1.3, re-tagged to ship the pre-built
binaries with verifiable provenance. The 0.1.3 binary release could not be
completed — its assets only build on the (now retired) Intel macOS runner, and
the partly-published release left the `v0.1.3` tag unusable for a GitHub Release.

### Added

- Release assets now ship a `SHA256SUMS` file and **SLSA build-provenance
  attestations**, signed keyless via Sigstore. Verify a download with
  `gh attestation verify <file> --repo sjourdois/morpion-solitaire`.

### Changed

- Release binaries are published as **bare executables** (a raw binary on Unix —
  `chmod +x` after download — and a `.exe` on Windows) instead of single-file
  `.tar.gz` / `.zip` archives.

### Fixed

- The release-binaries workflow cross-compiles the macOS Intel build on the Apple
  Silicon runner (no Intel runner needed), so all four targets — Linux (x86_64),
  macOS (Intel + Apple Silicon) and Windows (x86_64) — publish reliably.

## [0.1.3] — 2026-06-16

### Added

- A second line picker for points where several lines complete: **Click** mode —
  click to lock the point, aim by moving the cursor, click again to play — toggled
  on the board, with per-mode tooltips. The original cursor-aim + scroll-wheel
  mode stays.
- UI preferences now persist between sessions **on the web too**: the line picker,
  the view toggles, and the search configuration, in addition to theme and author.
- A localized landing page in all eight app languages, plus a sitemap, social
  cards (Open Graph), canonical/`hreflang` links, and a per-language redirect.
- Privacy-friendly, cookie-free analytics (GoatCounter) on the site.
- Pre-built binaries for Linux (x86_64), macOS (Intel + Apple Silicon) and
  Windows (x86_64), attached to each GitHub Release.

### Changed

- Board grid is now a **64×64 `u64` bitboard** (was 128×128 `u128`): ~1.6× the
  NRPA throughput, still holding every known record with margin. On overflow the
  search saves and stops; widen the `Row` alias for a larger grid.
- The algorithm selector and the docs are ordered systematic → beam → NRPA →
  perturbation.
- Updated to the latest dependencies, notably **egui / eframe 0.34** (and
  `rand` 0.10, `rfd` 0.17, `getrandom` 0.4), migrated to their current APIs.
- NRPA is **stabilized** by clamping its policy logits (a Stabilized-NRPA
  flavour, on by default; `NRPA_CLAMP=0` disables): markedly higher and steadier
  scores, with the gain *growing* as the search runs longer (5T best ≈ +10 at
  level 3 over 2 minutes). Its rollout/adapt hot loop is also ~12–17% faster.

### Fixed

- Touch input on phones and tablets (the canvas treated taps as scroll/zoom), and
  the controls panel is now usable on narrow screens.

[0.1.5]: https://github.com/sjourdois/morpion-solitaire/releases/tag/v0.1.5
[0.1.4]: https://github.com/sjourdois/morpion-solitaire/releases/tag/v0.1.4
[0.1.3]: https://github.com/sjourdois/morpion-solitaire/releases/tag/v0.1.3

## [0.1.2] — 2026-06-15

### Added

- Web build: "save to file" now downloads the record from the browser (a `Blob`
  behind a throwaway `<a download>`) for the MSR, JSON, Pentasol and SVG formats.
  The export button is no longer native-only. PNG export stays native (no wasm
  rasteriser).

### Fixed

- Theme desync: eframe could apply the platform/browser theme over the choice
  made at startup, leaving the menu and the board out of step. egui's visuals are
  now reasserted from the current `dark_mode` whenever they drift.

### Changed

- Footer: shows the version, the SPDX license and "© Stéphane Jourdois". The
  "Play" link (which pointed back at the site from inside the app) was removed;
  Docs and Source remain.

[0.1.2]: https://github.com/sjourdois/morpion-solitaire/releases/tag/v0.1.2

## [0.1.1] — 2026-06-15

### Added

- A README for the `morpion-solitaire` application crate (its crates.io page was
  otherwise bare).

[0.1.1]: https://github.com/sjourdois/morpion-solitaire/releases/tag/v0.1.1

## [0.1.0] — 2026-06-15

Initial release.

[0.1.0]: https://github.com/sjourdois/morpion-solitaire/releases/tag/v0.1.0
