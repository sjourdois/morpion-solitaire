# Morpion Solitaire

A fast **Morpion Solitaire** player and solver: a desktop/Web GUI plus a
headless CLI, built for hunting records (the 5T world record is **178**). It
ships with several search engines (NRPA, large-neighbourhood *perturbation*,
exhaustive) and a self-describing record format, **MSR**, meant to supersede the
older Pentasol text format.

> **Play online:** <https://morpion-solitaire.io> · **Source:** this repository

[![CI](https://github.com/sjourdois/morpion-solitaire/actions/workflows/ci.yml/badge.svg)](https://github.com/sjourdois/morpion-solitaire/actions/workflows/ci.yml)
[![Play online](https://img.shields.io/badge/play-morpion--solitaire.io-2a2b3c)](https://morpion-solitaire.io)
[![License](https://img.shields.io/badge/license-GPL--3.0%20%C2%B7%20MIT%2FApache-blue)](LICENSE)

## Features

- **Variants:** 4T, 4D, 5T, 5D (line length 4/5 × touching/disjoint).
- **GUI** (egui/eframe), native and in the browser (WebAssembly, multi-threaded).
- **Solvers:** NRPA (nested rollout policy adaptation), perturbation /
  large-neighbourhood search, exhaustive search with D4-symmetry pruning, beam.
- **Headless CLI:** search, replay, convert, records, bench.
- **MSR format:** lossless for all variants, carries provenance, with a
  reference reader/writer/**validator** — see [`docs/spec`](docs/spec/0.1/msr.md).
- **Languages:** the GUI is translated into English, French, German, Spanish,
  Italian, Portuguese, Dutch and Japanese (the CLI is English).

## Install

**Pre-built binaries** for Linux (x86_64), macOS (Intel & Apple Silicon) and
Windows (x86_64) are attached to every
[release](https://github.com/sjourdois/morpion-solitaire/releases/latest). Each
asset is the bare executable:

- **Linux / macOS** — download the file for your platform, then
  `chmod +x morpion-solitaire-* && ./morpion-solitaire-*`.
- **Windows** — download and run the `.exe`.
- **macOS** binaries are not notarized yet, so Gatekeeper warns on first launch:
  right-click → **Open**, or `xattr -d com.apple.quarantine <file>`.

Every release also ships a `SHA256SUMS` file and signed build-provenance
attestations. To check a download:

```sh
sha256sum -c SHA256SUMS --ignore-missing
gh attestation verify <file> --repo sjourdois/morpion-solitaire
```

Or build from crates.io (the GUI and CLI):

```sh
cargo install morpion-solitaire
```

## Build & run

Native (stable Rust):

```sh
cargo run --release          # launches the GUI
cargo run --release -- --help   # the CLI
```

Web build (the **only** part that needs nightly — it rebuilds `std` with the
wasm atomics feature for shared-memory threads):

```sh
cd crates/morpion-solitaire-wasm
trunk build --release
```

_The wasm crate pins nightly itself; `rust-src` and the wasm target auto-install on first build._

**Rust version.** The reusable libraries (`morpion-solitaire-record`,
`morpion-solitaire-records`) support **Rust 1.74+** (their MSRV, verified in CI).
The application tracks current stable, and only the web build needs nightly.

## CLI quickstart

```sh
morpion-solitaire search --algo nrpa --time 30s -o best.msr   # search, save best
morpion-solitaire replay best.msr                             # metadata + board

# Convert between formats (--to msr|json|pentasol):
morpion-solitaire convert game.psol --variant 5T --to msr -o game.msr   # Pentasol → MSR
morpion-solitaire convert game.msr --to json -o game.json              # MSR → JSON (readable)
morpion-solitaire convert game.json --to msr -o game.msr               # JSON → MSR (compact)
morpion-solitaire convert game.msr --to pentasol -o game.psol          # MSR → Pentasol (5T/5D)

# Image export — the PNG and SVG embed the MSR record:
morpion-solitaire convert game.msr --to png -o game.png                # PNG image + embedded record
morpion-solitaire convert game.msr --to svg -o game.svg                # SVG image + embedded record
```

## Indicative performance

A few reference figures, **purely indicative**: they depend heavily on the
hardware (measured here on a 32-core machine) and, for the heuristic searches, on
a short sample — longer runs go considerably further.

| Variant | Known record | Search | Reference |
|---------|--------------|--------|-----------|
| 4D | 35 (optimal) | systematic (exhaustive) | proves the optimum **35**; full key-space traversal: _to be measured_ |
| 4T | 62 (optimal) | NRPA (level 3) | 62 — the optimum — in 45 s |
| 5T | 178 (world record) | NRPA (level 3) | 106 in 45 s |
| 5D | 82 | NRPA (level 3) | 69 in 45 s |

In 4D the game space is small enough to be **explored exhaustively**: the
systematic search proves the optimum, and the app announces it (elapsed time +
optimal score) once the tree is drained.

### The board grid

The board is a fixed square **bitboard**: `GRID × GRID` cells where `GRID` is the
bit width of one row word (`Row` in `game::board`). It is currently `u64`, i.e. a
**64×64** grid. The width is a deliberate speed knob — `u64` bit-ops are
single-register on 64-bit hosts, giving roughly **1.6× the NRPA throughput** of a
128-bit grid, while 64×64 still holds every known record with margin (the largest,
rosin178, reaches grid coordinate ~19 of 60). `u32` (32×32) is no faster on a
64-bit host and is too small for 5T.

If a move would ever fall outside the grid, the board **refuses it, sets a
`GRID_OVERFLOW` flag, and the search stops and saves the game** — it never crashes
or silently corrupts a result. So if you push a search far past today's records
and hit an overflow, the fix is simply to **widen the grid**: change the one-line
`Row` alias (`u64` → `u128` for 128×128; beyond that, a `Row` newtype over
`[u128; k]`). Everything else derives from `Row`, so nothing else changes.

## The MSR record format

MSR is specified independently of this application so other tools can adopt it:

- **Specification:** [`docs/spec/0.1/msr.md`](docs/spec/0.1/msr.md) +
  [JSON Schema](docs/spec/0.1/msr.schema.json).
- **Reference crate:** [`crates/morpion-solitaire-record/`](crates/morpion-solitaire-record/),
  published as [`morpion-solitaire-record`](https://crates.io/crates/morpion-solitaire-record)
  (imported as `msr`).

## Repository layout

This is a Cargo workspace:

| Path | Crate | License |
|------|-------|---------|
| `crates/morpion-solitaire/` | the GUI + CLI application (library + native binary) | **GPL-3.0-or-later** |
| `crates/morpion-solitaire-wasm/` | the WebAssembly entry point (web shell) | **GPL-3.0-or-later** |
| `crates/morpion-solitaire-record/` | the MSR format library (imported as `msr`) | **MIT OR Apache-2.0** |
| `crates/morpion-solitaire-records/` | the embedded record-games corpus | **MIT OR Apache-2.0** |

The format library is permissively licensed so anyone can read/write/validate
MSR records; the application is GPL. (Permissive code is GPLv3-compatible, so the
app may depend on the library.)

## References

The search methods build on published research; see
[`docs/BIBLIOGRAPHY.md`](docs/BIBLIOGRAPHY.md).

## License

- Application (`morpion-solitaire`, `morpion-solitaire-wasm`): [GPL-3.0-or-later](LICENSE).
- Libraries (`morpion-solitaire-record`, `morpion-solitaire-records`): MIT OR
  Apache-2.0, at your option (see each crate's `LICENSE-MIT` / `LICENSE-APACHE`).
- Bundled fonts: **Atkinson Hyperlegible Next** and a subset of **Noto Sans CJK
  JP** (Japanese), both under the SIL Open Font License — see
  [`crates/morpion-solitaire/assets/fonts/`](crates/morpion-solitaire/assets/fonts/README.md).
