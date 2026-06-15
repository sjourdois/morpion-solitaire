# Contributing

Thanks for your interest in Morpion Solitaire! Contributions of all kinds are
welcome — bug reports, fixes, features, docs, and new record games.

By participating you agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md).

## Project layout

This is a Cargo workspace with two crates under **different licenses** — this
matters for what you contribute and where:

| Path | Crate | License |
|------|-------|---------|
| `crates/morpion-solitaire/` | the GUI + CLI application (library + native binary) | **GPL-3.0-or-later** |
| `crates/morpion-solitaire-wasm/` | the WebAssembly entry point (web shell) | **GPL-3.0-or-later** |
| `crates/morpion-solitaire-record/` | the MSR format library (imported as `msr`) | **MIT OR Apache-2.0** |
| `crates/morpion-solitaire-records/` | the embedded record-games corpus | **MIT OR Apache-2.0** |

By submitting a change you agree to license your contribution under the license
of the crate(s) it touches (GPL-3.0-or-later for the app; MIT OR Apache-2.0 for
the format library). No separate CLA is required.

## Building

Everything but the web build runs on **stable** (use it as your default
toolchain):

```sh
cargo run --release            # GUI
cargo run --release -- --help  # CLI
cargo test --workspace
```

The **only** thing that needs **nightly** is the threaded WebAssembly build (it
rebuilds `std` with the wasm atomics feature via `-Z build-std`). The wasm crate
pins nightly and that config itself (its own `rust-toolchain.toml` and
`.cargo/config.toml`), so just build from its directory:

```sh
cd crates/morpion-solitaire-wasm
trunk serve                    # or: trunk build --release
```

(nightly, `rust-src` and the wasm target auto-install on first build.)

## Before you open a PR

CI enforces these; please run them locally first:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- **Match the surrounding style.** Keep comment density and naming consistent
  with nearby code.
- **Add tests** for behaviour changes. The format crate ships conformance tests;
  performance-sensitive engine changes should keep the differential move-gen test
  passing.
- **Touching the MSR format?** Update the spec in [`docs/spec`](docs/spec/) and
  the JSON Schema in lockstep — the format definition is normative, the crate is
  its reference implementation.
- **Keep the WASM build green** if you change shared code (it compiles for
  `wasm32` too; avoid native-only APIs outside `#[cfg(not(target_arch = "wasm32"))]`).

## Commit & PR

- Small, focused commits with clear messages.
- Describe the *why*, not just the *what*, in the PR description.
- Reference any issue it closes.

## Submitting a record game

Found or verified a strong game? Export it with the CLI and attach the `.msr`
file (it carries provenance — method, author, date):

```sh
morpion-solitaire verify your-game.msr   # must pass
```

## Questions

Open a [discussion or issue](https://github.com/sjourdois/morpion-solitaire/issues),
or email <stephane@jourdois.fr>.
