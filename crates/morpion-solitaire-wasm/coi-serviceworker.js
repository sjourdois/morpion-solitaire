/**
 * coi-serviceworker — adds Cross-Origin-Opener-Policy / Cross-Origin-Embedder-Policy
 * headers to every response so that SharedArrayBuffer (required by wasm-bindgen-rayon)
 * is available even on static hosts that do not send those headers (e.g. GitHub Pages).
 *
 * Technique: the script registers itself as a Service Worker on the first load;
 * the SW intercepts all fetches and injects the COI headers; the page is then
 * automatically reloaded to run under the SW's augmented headers.
 *
 * Source: inspired by https://github.com/gzuidhof/coi-serviceworker (MIT)
 */

/* ── Service Worker scope ─────────────────────────────────────────────────── */
if (typeof window === "undefined") {
    self.addEventListener("install", () => self.skipWaiting());
    self.addEventListener("activate", event => event.waitUntil(self.clients.claim()));

    self.addEventListener("fetch", event => {
        const req = event.request;

        // Passthrough for opaque "only-if-cached" cross-origin requests
        // (Chrome quirk that causes a TypeError if we try to fetch them).
        if (req.cache === "only-if-cached" && req.mode !== "same-origin") return;

        event.respondWith(
            fetch(req)
                .then(response => {
                    if (response.status === 0) return response; // opaque
                    const headers = new Headers(response.headers);
                    headers.set("Cross-Origin-Opener-Policy",   "same-origin");
                    headers.set("Cross-Origin-Embedder-Policy", "require-corp");
                    headers.set("Cross-Origin-Resource-Policy", "cross-origin");
                    return new Response(response.body, {
                        status:     response.status,
                        statusText: response.statusText,
                        headers,
                    });
                })
                .catch(() => fetch(req)) // network error: let browser handle it
        );
    });
}

/* ── Main-page scope ──────────────────────────────────────────────────────── */
else {
    // Nothing to do if the page is already cross-origin isolated.
    if (self.crossOriginIsolated) {
        console.debug("[coi-sw] Already cross-origin isolated, no action needed.");
    } else {
        (async () => {
            if (!("serviceWorker" in navigator)) {
                console.warn("[coi-sw] Service Workers not supported — SharedArrayBuffer will be unavailable.");
                return;
            }

            try {
                const reg = await navigator.serviceWorker.register(
                    document.currentScript.src,
                    { scope: "./" }
                );

                // Wait for the SW to become active (installing → waiting → active).
                await new Promise((resolve, reject) => {
                    const sw = reg.installing || reg.waiting || reg.active;
                    if (!sw) return reject(new Error("No SW found"));

                    if (sw.state === "activated") {
                        resolve();
                    } else {
                        sw.addEventListener("statechange", e => {
                            if (e.target.state === "activated") resolve();
                            if (e.target.state === "redundant")  reject(new Error("SW became redundant"));
                        });
                    }
                });

                console.debug("[coi-sw] Service Worker active — reloading for COI headers.");
                location.reload();
            } catch (err) {
                console.warn("[coi-sw] Failed to register Service Worker:", err);
            }
        })();
    }
}
