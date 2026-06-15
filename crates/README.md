# Workspace crates

The members of this Cargo workspace. See the [project README](../README.md) for
an overview and the [contributing guide](../CONTRIBUTING.md) for how to build.

| Crate | Role | crates.io | License |
|-------|------|-----------|---------|
| [`morpion-solitaire`](morpion-solitaire/) | The GUI + CLI application (library + native binary). | [morpion-solitaire](https://crates.io/crates/morpion-solitaire) | GPL-3.0-or-later |
| [`morpion-solitaire-wasm`](morpion-solitaire-wasm/) | The WebAssembly entry point — a thin web shell over the app library, built with Trunk. | — (not published) | GPL-3.0-or-later |
| [`morpion-solitaire-record`](morpion-solitaire-record/) | The **MSR** record format: reader, writer, validator. Imported as `msr`. | [morpion-solitaire-record](https://crates.io/crates/morpion-solitaire-record) | MIT OR Apache-2.0 |
| [`morpion-solitaire-records`](morpion-solitaire-records/) | A curated corpus of record games, embedded as MSR strings. | [morpion-solitaire-records](https://crates.io/crates/morpion-solitaire-records) | MIT OR Apache-2.0 |

The application is GPL; the two reusable libraries are permissively licensed so
any tool can read/write/validate MSR records and use the corpus. Permissive code
is GPLv3-compatible, so the application may depend on both.

The application depends on both libraries; the libraries depend on neither each
other nor the app (`morpion-solitaire-records` is dependency-free — its embedded
records are decoded with `msr` by whoever consumes them):

```
morpion-solitaire-wasm ──▶ morpion-solitaire ──▶ morpion-solitaire-record   (msr)
                                            └──▶ morpion-solitaire-records
```
