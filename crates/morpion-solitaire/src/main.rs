fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // A CLI subcommand runs headless and exits inside `dispatch`; with no
        // subcommand it returns `None` and we launch the GUI as before.
        if morpion_solitaire::cli::dispatch().is_none() {
            morpion_solitaire::run_native().expect("failed to run");
        }
    }
    // On WASM the entry point is #[wasm_bindgen(start)] in lib.rs.
}
