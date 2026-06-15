# morpion-solitaire-record (`msr`)

A small, self-describing **interchange format for [Morpion Solitaire] games** —
the move list plus optional provenance (who/what/when produced it, how hard, and
a few derived facts) — with a reference **reader, writer and validator**.

The crate is published as `morpion-solitaire-record` and imported as `msr`:

```toml
[dependencies]
morpion-solitaire-record = "0.1"
```

**MSRV:** Rust 1.74+ (verified in CI). No `unsafe` (`#![forbid(unsafe_code)]`).

```rust
use msr::{decode, encode, validate, Record, RecordMove, Variant, Direction};

// Read either form (compact `MS1:` envelope or plain JSON):
let record = decode(text)?;

// Verify it is a legal game, independently of any solver:
validate(&record)?;

// Write it back out (compact, or `encode_json` for the readable form):
let compact = encode(&record)?;
```

## Why MSR (and not Pentasol)

The community **Pentasol** text format covers only the 5T/5D variants and stores
the moves alone. **MSR**:

- is lossless for **all four** variants (4T / 4D / 5T / 5D);
- carries **provenance metadata** (producer, author, method, seed, timestamps,
  search effort, free-form tags) so a file is meaningful on its own;
- offers a **compact** form (`MS1:` = Base64 of DEFLATE-compressed JSON) and a
  **human-readable** JSON form, both carrying the same data;
- ships a **reference validator** so any tool can check a record is legal.

A Pentasol bridge is provided for migration.

## Tooling

This crate is the library; the [`morpion-solitaire`](https://github.com/sjourdois/morpion-solitaire)
application — a desktop/web GUI **and** a headless CLI — is the reference tool
built on it. The CLI reads any MSR (compact or JSON) or Pentasol file and emits
it in any format with `convert --to ascii|msr|json|pentasol|svg|png` (default
`ascii`):

```sh
morpion-solitaire convert game.psol --variant 5T --to msr -o game.msr  # Pentasol → MSR
morpion-solitaire convert game.msr  --to json                          # MSR → readable JSON
morpion-solitaire convert game.msr  --to pentasol                      # MSR → legacy Pentasol (5T/5D)
morpion-solitaire convert game.msr  --to png -o game.png               # rendered board image
morpion-solitaire convert game.msr                                     # ASCII board on the terminal

morpion-solitaire replay game.msr                                      # metadata + board + verdict
morpion-solitaire replay game.msr -q                                   # just the legality verdict
```

The SVG and PNG **embed the full record** (PNG in a `tEXt` chunk, SVG in a
`<metadata>` element), so a rendered image is itself a loadable save — the GUI
reads them back (and accepts drag-and-dropped `.png`/`.svg`/`.msr`/`.json`/`.psol`
files). `replay` re-derives the game move by move, so it doubles as a validator
(non-zero exit on an illegal game). The GUI additionally **searches** for record
games (NRPA, large-neighbourhood perturbation, exhaustive) and runs in the
browser via WebAssembly.

## Specification

The normative definition is the **MSR 0.1** specification and JSON Schema in the
[`docs/spec`](https://github.com/sjourdois/morpion-solitaire/tree/main/docs/spec)
directory of the repository. This crate is the reference implementation.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at
your option. (The Morpion Solitaire application that uses this crate is licensed
separately under GPL-3.0-or-later.)

[Morpion Solitaire]: https://en.wikipedia.org/wiki/Join_Five
