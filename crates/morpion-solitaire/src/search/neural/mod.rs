//! Neural move prior for NRPA (feature `neural`, native-only) — a [`BiasModifier`]
//! plugin ported from the `neural-guide` archive onto the plugin hooks.
//!
//! Scaffold only for now: enough to pull in candle and verify it builds. The inference
//! core (features/position/net/embedded), the prior plugin, training, PUCT and
//! tabula-rasa land in subsequent phase-5 commits.

#[allow(unused_imports)]
use candle_core::{Device, Tensor};

/// Smoke check that candle is wired in (the CPU device is always available).
#[allow(dead_code)]
pub(crate) fn candle_available() -> bool {
    Tensor::from_slice(&[0.0f32], (1,), &Device::Cpu).is_ok()
}
