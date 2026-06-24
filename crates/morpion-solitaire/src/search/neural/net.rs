//! The policy network and the [`MovePrior`] abstraction.
//!
//! A compact MLP scores a single candidate move from its [`super::features`]
//! vector: `FEATURE_LEN → HIDDEN → HIDDEN → 1`. Per position, the scores over the
//! legal moves are a set of logits; a softmax over them is the move policy, and a
//! logit is exactly the GNRPA bias `β` the NRPA playout wants (it already takes
//! `exp(w/τ + β)`). Keeping the head at one logit per move — rather than a
//! fixed-size board-wide policy as in AlphaZero — lets the same net score any
//! number of legal moves and exploits the move-local encoding.
//!
//! [`MovePrior`] is the seam the search consumes: given the feature vectors of a
//! position's legal moves, return one bias per move. The PUCT method and the
//! feature-space / value-net machinery (ValueNet, penultimate φ) land in later
//! phase-5 commits; this file is the policy-prior core.

use candle_core::{Device, Result, Tensor};
use candle_nn::{linear, Linear, Module, VarBuilder, VarMap};

use super::features::FEATURE_LEN;

/// Hidden width `h`. Fixed at 64 — the width the bundled prior was trained at, and the
/// only one shipped. A loaded model's shapes must match (the safetensors encode it).
/// Not an env var (the project's knobs are CLI/GUI options); a width sweep would make
/// this an [`OptionSpec`](crate::search::plugin::OptionSpec) carried in the filename.
pub const HIDDEN: usize = 64;

/// A learned bias `β` over a position's candidate moves. The search calls this to
/// turn encoded moves into per-move priors; the implementation is free to batch.
pub trait MovePrior {
    /// One bias per candidate, in the order given. Higher ⇒ more preferred. Each
    /// `features[i]` is a [`FEATURE_LEN`]-long vector from [`super::features::encode`].
    fn biases(&self, features: &[Vec<f32>]) -> Vec<f64>;
}

/// The MLP move-scorer.
pub struct PolicyNet {
    l1: Linear,
    l2: Linear,
    l3: Linear,
}

impl PolicyNet {
    /// Build the net's layers under `vb` (weights live in the backing `VarMap`).
    pub fn new(vb: VarBuilder) -> Result<Self> {
        Ok(Self {
            l1: linear(FEATURE_LEN, HIDDEN, vb.pp("l1"))?,
            l2: linear(HIDDEN, HIDDEN, vb.pp("l2"))?,
            l3: linear(HIDDEN, 1, vb.pp("l3"))?,
        })
    }

    /// Forward `[N, FEATURE_LEN]` → `[N]`: one logit per input move.
    pub fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let x = self.l1.forward(x)?.relu()?;
        let x = self.l2.forward(&x)?.relu()?; // [N, h]
        let x = self.l3.forward(&x)?; // [N, 1]
        x.squeeze(1) // [N]
    }
}

/// A [`MovePrior`] backed by a trained [`PolicyNet`] on a device. Owns its
/// `VarMap` so the weights stay alive (and can be saved/loaded later).
pub struct NeuralPrior {
    net: PolicyNet,
    /// Retained so the trained weights can be saved/loaded. Not read for inference —
    /// the `PolicyNet` layers hold their own (Arc-backed) tensor refs.
    #[allow(dead_code)]
    varmap: VarMap,
    device: Device,
}

impl NeuralPrior {
    /// Stack feature vectors into an `[N, FEATURE_LEN]` tensor on the net's device.
    fn stack(&self, features: &[Vec<f32>]) -> Result<Tensor> {
        let n = features.len();
        let flat: Vec<f32> = features.iter().flat_map(|f| f.iter().copied()).collect();
        Tensor::from_vec(flat, (n, FEATURE_LEN), &self.device)
    }

    /// Wrap a freshly trained net + its varmap (used by training in a later commit).
    #[allow(dead_code)]
    pub fn new(net: PolicyNet, varmap: VarMap, device: Device) -> Self {
        Self {
            net,
            varmap,
            device,
        }
    }

    /// Save the trained weights to a safetensors file (reuse across runs without
    /// retraining).
    pub fn save(&self, path: &str) -> Result<()> {
        self.varmap.save(path)
    }

    /// Load a [`NeuralPrior`] from a safetensors file written by [`save`](Self::save).
    /// Rebuilds the net (so the var shapes exist) then overwrites them with the
    /// saved tensors.
    pub fn load(path: &str, device: Device) -> Result<Self> {
        let mut varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, candle_core::DType::F32, &device);
        let net = PolicyNet::new(vb)?;
        varmap.load(path)?;
        Ok(Self {
            net,
            varmap,
            device,
        })
    }

    /// Logits for a position's candidate moves (raw net outputs).
    pub fn logits(&self, features: &[Vec<f32>]) -> Result<Vec<f32>> {
        if features.is_empty() {
            return Ok(Vec::new());
        }
        let x = self.stack(features)?;
        self.net.forward(&x)?.to_vec1::<f32>()
    }
}

impl MovePrior for NeuralPrior {
    fn biases(&self, features: &[Vec<f32>]) -> Vec<f64> {
        // A prior must never break a playout; on the (unexpected) inference error
        // fall back to a neutral zero bias, which reduces NRPA to its base policy.
        match self.logits(features) {
            Ok(v) => v.into_iter().map(|x| x as f64).collect(),
            Err(e) => {
                log::error!("neural prior inference failed: {e}");
                vec![0.0; features.len()]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{Device, Tensor};

    /// The net forwards to one logit per input move, with the right output length.
    #[test]
    fn forward_shapes() {
        let dev = Device::Cpu;
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, candle_core::DType::F32, &dev);
        let net = PolicyNet::new(vb).unwrap();
        let n = 5usize;
        let x = Tensor::arange(0f32, (n * FEATURE_LEN) as f32, &dev)
            .unwrap()
            .reshape((n, FEATURE_LEN))
            .unwrap();
        let out = net.forward(&x).unwrap().to_vec1::<f32>().unwrap();
        assert_eq!(out.len(), n);
    }
}
