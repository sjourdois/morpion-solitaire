# morpion-solitaire

A fast **Morpion Solitaire** player and solver: a desktop/web GUI plus a headless
CLI, built for hunting records (the 5T world record is **178**). It ships several
search engines (NRPA, large-neighbourhood perturbation, exhaustive) and reads and
writes the self-describing **MSR** record format.

- **Play online:** <https://morpion-solitaire.io>
- The **MSR format** lives in the separate, permissively-licensed
  [`morpion-solitaire-record`](https://crates.io/crates/morpion-solitaire-record)
  crate (imported as `msr`); the bundled record games are in
  [`morpion-solitaire-records`](https://crates.io/crates/morpion-solitaire-records).

## Install

```sh
cargo install morpion-solitaire
```

This installs a single binary that is **both** the GUI (run with no arguments)
and the CLI.

## Use

```sh
morpion-solitaire                                             # launch the GUI
morpion-solitaire --help                                     # CLI help

morpion-solitaire search --algo nrpa --time 30s -o best.msr  # search, save the best game
morpion-solitaire replay best.msr                            # replay + validate a game
morpion-solitaire convert game.msr --to png -o game.png      # render (the record is embedded)
```

CLI subcommands: `search`, `replay` (replays *and* validates a game), `convert`
(to `ascii`/`msr`/`json`/`pentasol`/`svg`/`png` — SVG and PNG embed the record so
a picture is also a save), `records`, and `bench`. The CLI is English; the GUI is
localised into 8 languages.

## Web build

The browser version (multi-threaded WebAssembly) is built from the
`morpion-solitaire-wasm` crate with [Trunk](https://trunkrs.dev). See the
[repository](https://github.com/sjourdois/morpion-solitaire) and the
[book](https://morpion-solitaire.io/docs/).

## License

GPL-3.0-or-later. (The `msr` format library and the record corpus are MIT OR
Apache-2.0, so any tool can read/write/validate MSR records without the GPL.)
