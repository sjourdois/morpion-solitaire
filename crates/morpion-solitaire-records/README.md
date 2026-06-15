# morpion-solitaire-records

A curated corpus of Morpion Solitaire **record games**, embedded as
[MSR](https://crates.io/crates/morpion-solitaire-record) records. It has no
dependencies and works without a filesystem (e.g. in WebAssembly), so it is
usable both as the application's bundled reference games and by any other tool.

```rust
for (name, id, record) in morpion_solitaire_records::RECORDS {
    // `record` is the MSR JSON; decode it with the `morpion-solitaire-record`
    // crate (which also reads the compact MS1: form) if you want typed records.
    println!("{name} [{id}]");
}
```

**MSRV:** Rust 1.74+ (verified in CI). No dependencies, no `unsafe`.

## Corpus

The **source of truth** is one JSON file per record, organised by variant under
`records/<variant>/<id>.json`. All are legal, terminal games. Each record's
`author` names the record holder, `source` links to the original Pentasol file or
grid image on morpionsolitaire.com (where the game originates), and
`transcribed_by` is `morpion-solitaire.io` (this project, which put it into MSR
form).

The other formats are **derived**, not committed: the workspace's
`gen_record_artifacts` example renders each record to a compact `.msr`, a
`.png`/`.svg` (with the record embedded), and — for 5T/5D — a `.psol`, all
published under [morpion-solitaire.io/records/](https://morpion-solitaire.io/records/).
A format change therefore only touches the JSON.

| Name | Variant | Score | Creator |
|------|---------|-------|---------|
| Rosin 178 | 5T | 178 | Christopher D. Rosin (world record) |
| Rosin 177A | 5T | 177 | Christopher D. Rosin |
| Rosin 177B | 5T | 177 | Christopher D. Rosin |
| Tishchenko 172 | 5T | 172 | Tishchenko |
| Rosin 172 | 5T | 172 | Christopher D. Rosin |
| Tishchenko 171 | 5T | 171 | Tishchenko |
| Bruneau 170 | 5T | 170 | Charles-Henri Bruneau (by hand, 1976) |
| Rosin 170A | 5T | 170 | Christopher D. Rosin |
| Akiyama 146 | 5T | 146 | Akiyama |
| Akiyama 145 | 5T | 145 | Akiyama |
| Rosin 82 | 5D | 82 | Christopher D. Rosin |
| Hyyrö–Poranen 62 | 4T | 62 | Heikki Hyyrö & Timo Poranen (optimal) |
| Demaine 56 | 4T | 56 | Demaine, Demaine, Langerman & Langerman (2006) |
| Hyyrö–Poranen 35 | 4D | 35 | Heikki Hyyrö & Timo Poranen (optimal) |
| Demaine 31 | 4D | 31 | Demaine, Demaine, Langerman & Langerman (2006) |

> The 4T/4D records are published only as *images* on morpionsolitaire.com (no
> Pentasol files). The 4T/4D grids above were transcribed from their images (with
> `tools/grid_to_msr.py`) and re-verified as legal games. The remaining records
> (5D-102 and several older 4T/4D grids) are hand-drawn grids photographed on
> graph paper — too noisy for reliable automatic transcription.

**Attribution.** These games come from the community record collection at
[morpionsolitaire.com](http://morpionsolitaire.com/English/RecordsTable.htm)
(maintained by Christian Boyer); each record keeps a `source` link to its
original Pentasol grid or grid image, and `author` credits the record holder.

## Converting & rendering records

Use the [`morpion-solitaire`](https://github.com/sjourdois/morpion-solitaire)
tool to emit any record in any format with `convert --to
ascii|msr|json|pentasol|svg|png` — for example to/from the legacy **Pentasol**
text, or to a board image with the record embedded:

```sh
# JSON record (this corpus) → legacy Pentasol
morpion-solitaire convert records/5T/rosin178.json --to pentasol -o rosin178.psol

# Pentasol → MSR (supply the variant, which Pentasol doesn't record)
morpion-solitaire convert rosin178.psol --variant 5T --to msr

# JSON record → board image (PNG with the record embedded in a tEXt chunk)
morpion-solitaire convert records/5T/rosin178.json --to png -o rosin178.png
```

To contribute a game, see the
[contributing guide](https://github.com/sjourdois/morpion-solitaire/blob/main/CONTRIBUTING.md).

## License

Licensed under either of MIT or Apache-2.0 at your option.
