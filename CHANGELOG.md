# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
