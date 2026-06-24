//! Embedded from-scratch artifacts: a corpus of self-found games and a prior
//! pre-trained on it — **no human records**. Both are generated on the fleet
//! (cold-start Expert Iteration) and committed under `assets/neural/`, so the
//! product ships a ready move prior that owes nothing to human play.
//!
//! Until a variant's artifacts are generated they are empty placeholders
//! (`corpus-*.json` = `[]`, an empty `prior-*.safetensors`); the loaders treat
//! empty as "not available" so the GUI greys the matching sources. Native +
//! `neural` only.

use super::net::NeuralPrior;
use crate::game::moves::Move;
use crate::game::rules::Variant;
use candle_core::Device;

/// Per-variant embedded corpus JSON. Only 5T is generated so far; the rest fall
/// back to the empty array. `include_str!` needs the file to exist at build time.
fn corpus_json(variant: Variant) -> &'static str {
    match variant.name() {
        "5T" => include_str!("../../../assets/neural/corpus-5T.json"),
        _ => "[]",
    }
}

/// Per-variant embedded prior bytes (safetensors). Empty until generated.
fn prior_bytes(variant: Variant) -> &'static [u8] {
    match variant.name() {
        "5T" => include_bytes!("../../../assets/neural/prior-5T.safetensors"),
        _ => &[],
    }
}

/// The embedded from-scratch corpus for `variant` — the games the bundled prior
/// trained on. Empty if none is committed yet.
pub fn corpus(variant: Variant) -> Vec<Vec<Move>> {
    serde_json::from_str(corpus_json(variant)).unwrap_or_default()
}

/// Whether a non-empty bundled corpus is committed for `variant` (cheap — the
/// placeholder is exactly `[]`).
pub fn has_corpus(variant: Variant) -> bool {
    corpus_json(variant).trim() != "[]"
}

/// Load the bundled pre-trained prior for `variant`, if one is committed. The
/// `VarMap` loader takes a path, so stage the embedded bytes in a temp file.
pub fn prior(variant: Variant) -> Option<NeuralPrior> {
    let bytes = prior_bytes(variant);
    if bytes.is_empty() {
        return None;
    }
    let path = std::env::temp_dir().join(format!(
        "mso-bundled-{}-{}.safetensors",
        variant.name(),
        std::process::id()
    ));
    std::fs::write(&path, bytes).ok()?;
    let res = NeuralPrior::load(&path.to_string_lossy(), Device::Cpu);
    let _ = std::fs::remove_file(&path);
    res.ok()
}

/// Whether a bundled prior is committed for `variant` (cheap — byte length).
pub fn has_prior(variant: Variant) -> bool {
    !prior_bytes(variant).is_empty()
}
