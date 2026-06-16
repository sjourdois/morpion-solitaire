# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.4] — 2026-06-16

Maintenance release: same application as 0.1.3, re-tagged to ship the pre-built
binaries. The 0.1.3 binary release could not be completed — its assets only
build on the (now retired) Intel macOS runner, and the partly-published release
left the `v0.1.3` tag unusable for a GitHub Release.

### Fixed

- The release-binaries workflow now cross-compiles the macOS Intel build on the
  Apple Silicon runner (no Intel runner needed), so all four targets — Linux
  (x86_64), macOS (Intel + Apple Silicon) and Windows (x86_64) — publish
  reliably.

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
