# Install

## From crates.io

```sh
cargo install morpion-solitaire   # the GUI + CLI
```

## From source (native)

Use **stable** as your default toolchain:

```sh
git clone https://github.com/sjourdois/morpion-solitaire
cd morpion-solitaire
cargo run --release            # launches the GUI
cargo run --release -- --help  # the CLI
```

## The web build

The browser build is the **only** part that needs **nightly** — it rebuilds
`std` with the WebAssembly atomics feature (`-Z build-std`) for shared-memory
threads. The wasm crate pins nightly and that config itself, so just build from
its directory:

```sh
cd crates/morpion-solitaire-wasm
trunk serve   # or: trunk build --release
```

(nightly, `rust-src` and the wasm target auto-install on first build.)

Threaded WebAssembly needs the page to be *cross-origin isolated*; the bundled
`coi-serviceworker.js` arranges this on static hosts such as GitHub Pages.

> A hosted build is at <https://morpion-solitaire.io>.
