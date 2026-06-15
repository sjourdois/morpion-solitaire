//! WebAssembly entry point for the Morpion Solitaire GUI.
//!
//! The whole crate is gated to the `wasm32` target, so on native it is empty and
//! a no-op in workspace builds. The shared GUI lives in the `morpion-solitaire`
//! library; this crate only wires up the browser: threads, panic hook, logging,
//! and the canvas.
#![cfg(target_arch = "wasm32")]
#![forbid(unsafe_code)]

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn main_wasm() {
    use wasm_bindgen::JsCast as _;

    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).expect("logger init failed");

    // Locale detection lives here (not in the library) so the library carries no
    // wasm-specific dependency.
    if let Some(lang) = web_sys::window().and_then(|w| w.navigator().language()) {
        morpion_solitaire::i18n::set_locale(&lang);
    }

    wasm_bindgen_futures::spawn_local(async {
        let window = web_sys::window().expect("no window");
        let num_threads = window.navigator().hardware_concurrency() as usize;
        let num_threads = num_threads.max(1);

        // Rayon workers require SharedArrayBuffer which is only available when
        // the page is cross-origin isolated (COOP + COEP headers).  Skip the
        // thread pool when those headers are absent so the app still starts.
        let cross_origin_isolated = js_sys::Reflect::get(&window, &"crossOriginIsolated".into())
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if cross_origin_isolated {
            if let Err(e) = wasm_bindgen_rayon::init_thread_pool(num_threads).await {
                log::warn!("Thread pool init failed: {:?}", e);
            }
        } else {
            log::warn!("Page is not cross-origin isolated — running single-threaded");
        }

        let canvas = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("the_canvas_id"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            .expect("canvas #the_canvas_id not found");

        let web_options = eframe::WebOptions::default();
        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(morpion_solitaire::create_app(cc))),
            )
            .await
            .expect("failed to start eframe");
    });
}
