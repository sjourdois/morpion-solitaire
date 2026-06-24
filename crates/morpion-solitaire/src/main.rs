fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // A CLI subcommand runs headless and exits inside `dispatch`; with no
        // subcommand it returns `None` and we launch the GUI (when built with it).
        if morpion_solitaire::cli::dispatch().is_none() {
            #[cfg(feature = "gui")]
            morpion_solitaire::run_native().expect("failed to run");
            #[cfg(not(feature = "gui"))]
            eprintln!(
                "headless build (no GUI): pass a CLI subcommand — e.g. `search`, \
                 `replay`, `convert`. Run with `--help` for the full list."
            );
        }
    }
    // On WASM the entry point is #[wasm_bindgen(start)] in lib.rs.
}
