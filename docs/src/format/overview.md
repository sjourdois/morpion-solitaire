# The MSR format — overview

**MSR** (Morpion Solitaire Record) is a small, self-describing interchange format
for games: the move list plus optional provenance, in either a compact `MS1:`
envelope or plain JSON. It is designed to supersede the older **Pentasol** text
format.

## Why MSR

| | Pentasol | MSR |
|---|---|---|
| Variants | 5T / 5D only | **all four** (4T/4D/5T/5D) |
| Payload | moves only | moves **+ provenance metadata** |
| Forms | text | compact (`MS1:`) **and** readable JSON |
| Validation | — | a **reference validator** |

A record carries who/what/when produced a game (producer, author, method, seed,
timestamps, search effort, tags) and a few derived facts, so a file is meaningful
on its own.

## Reusable library

The format is implemented by the standalone
[`morpion-solitaire-record`](https://crates.io/crates/morpion-solitaire-record)
crate (imported as `msr`) — a reader, writer and validator with no dependency on
any solver, so any tool can adopt it:

```rust
let record = msr::decode(text)?;   // MS1: envelope or raw JSON
msr::validate(&record)?;           // is it a legal game?
let compact = msr::encode(&record)?;
```

The normative definition is the [specification](spec.md) and the
[JSON Schema](schema.md). A Pentasol bridge is provided for
[migration](pentasol.md).
