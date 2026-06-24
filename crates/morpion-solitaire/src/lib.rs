//! Morpion Solitaire — a fast player and solver.
//!
//! This is the shared library behind both the native binary (GUI + headless
//! [`cli`]) and the WebAssembly build (the `morpion-solitaire-wasm` crate). The
//! [`game`] model and [`search`] engines are format-agnostic; the self-describing
//! record format itself lives in the separate `morpion-solitaire-record` (`msr`)
//! crate. See the project [README] and book for usage.
//!
//! [README]: https://github.com/sjourdois/morpion-solitaire
#![forbid(unsafe_code)]

#[cfg(feature = "gui")]
pub mod app;
#[cfg(not(target_arch = "wasm32"))]
pub mod cli;
pub mod game;
// i18n (translations) is only used by the GUI — the CLI is English-only — so the
// headless build drops it and its embed deps.
#[cfg(feature = "gui")]
pub mod i18n;
pub mod render;
pub mod search;
#[cfg(feature = "gui")]
pub mod ui;

#[cfg(feature = "gui")]
use app::MorpionApp;

/// Construct the eframe application. Shared by the native binary ([`run_native`])
/// and the WebAssembly entry point (the `morpion-solitaire-wasm` crate). GUI-only.
#[cfg(feature = "gui")]
pub fn create_app(cc: &eframe::CreationContext<'_>) -> Box<dyn eframe::App> {
    Box::new(MorpionApp::new(cc))
}

#[cfg(all(feature = "gui", not(target_arch = "wasm32")))]
pub fn run_native() -> eframe::Result<()> {
    env_logger::init();
    i18n::set_language(&i18n::detect_locale());
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_app_id("morpion-solitaire"),
        ..Default::default()
    };
    eframe::run_native(
        "Morpion Solitaire",
        native_options,
        Box::new(|cc| Ok(create_app(cc))),
    )
}
