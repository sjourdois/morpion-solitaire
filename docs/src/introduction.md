# Introduction

**Morpion Solitaire** is a fast player and solver for the
[pencil-and-paper puzzle](https://en.wikipedia.org/wiki/Join_Five) of the same
name: a desktop and Web GUI plus a headless CLI, built for hunting records (the
5T world record is **178**).

This book covers three things:

- **Using it** — installing, playing in the [GUI](guide/gui.md), and the
  [command line](guide/cli.md).
- **The MSR format** — a self-describing record format ([overview](format/overview.md),
  [specification](format/spec.md)) meant to supersede the older Pentasol text
  format, with a reusable reference library.
- **Internals** — the [architecture](internals/architecture.md), the
  [search algorithms](internals/algorithms.md), and the
  [fixed-grid board](internals/board.md).

> **Play online:** <https://morpion-solitaire.io> ·
> **Source:** <https://github.com/sjourdois/morpion-solitaire>

The project is a Cargo workspace of three crates — the GPL-3.0-or-later
application and two permissively licensed libraries (the MSR format and a record
corpus). See [Architecture](internals/architecture.md).
