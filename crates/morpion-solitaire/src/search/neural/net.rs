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

use super::features::{PatchKey, FEATURE_LEN};
use crate::game::{moves::Move, state::GameState};

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

/// A pluggable source of per-move feature vectors `φ(s,m)` for feature-space NRPA:
/// the adapt rule `θ += α_θ(φ_chosen − Σ p φ)` is written against this trait, so
/// only *how φ is produced* varies. [`NeuralPrior`]'s
/// impl returns the frozen net's penultimate activation over the move's local patch.
pub trait FeatureSource: Send + Sync {
    /// Feature dimension `d` (= |θ|).
    fn feat_dim(&self) -> usize;
    /// Warm-start `θ₀` reproducing this source's own prior logit at step 0
    /// (`scale·head` for the net); zeros ≡ cold. Length must be [`feat_dim`](Self::feat_dim).
    fn warm_theta(&self, scale: f64) -> Vec<f64>;
    /// A move's cache key plus the raw model input run on a cache miss.
    fn key_and_input(&self, scratch: &GameState, mv: &Move) -> (PatchKey, Vec<f32>);
    /// Run the model on a batch of miss inputs → one `φ` each (length [`feat_dim`](Self::feat_dim)).
    fn compute_features(&self, inputs: &[Vec<f32>]) -> Vec<Vec<f32>>;
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

    /// Forward `[N, FEATURE_LEN]` → `[N, h]`: the **penultimate** activation (after
    /// `l2.relu`, before the head `l3`). This is the feature map `φ(s,m)` that
    /// feature-space NRPA adapts a linear head `θ` over; by construction
    /// `l3(forward_features(x))` equals [`forward`](Self::forward).
    pub fn forward_features(&self, x: &Tensor) -> Result<Tensor> {
        let x = self.l1.forward(x)?.relu()?;
        self.l2.forward(&x)?.relu() // [N, h]
    }

    /// Forward `[N, FEATURE_LEN]` → `[N]`: one logit per input move.
    pub fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let x = self.forward_features(x)?; // [N, h]
        let x = self.l3.forward(&x)?; // [N, 1]
        x.squeeze(1) // [N]
    }

    /// The output head `l3` (a `[1, h]` weight). Feature-space NRPA reads its weight to
    /// warm-start `θ₀ = scale·l3.weight`, so `θ₀·φ` reproduces the frozen prior's logit.
    pub fn head(&self) -> &Linear {
        &self.l3
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

    /// Penultimate feature maps `φ(s,m)` for a position's candidate moves: one `[h]`
    /// vector per move (the frozen `l1,l2` activation). Feature-space NRPA caches these
    /// per local pattern and adapts a head `θ` over them; the net stays frozen.
    fn penult(&self, features: &[Vec<f32>]) -> Result<Vec<Vec<f32>>> {
        if features.is_empty() {
            return Ok(Vec::new());
        }
        let x = self.stack(features)?;
        self.net.forward_features(&x)?.to_vec2::<f32>() // [N, h]
    }

    /// The head weight `l3.weight` flattened to `[h]` (f64). `θ₀ = scale·head_weight`
    /// makes `θ·φ` reproduce the frozen prior's contribution exactly.
    fn head_weight(&self) -> Vec<f64> {
        match self
            .net
            .head()
            .weight()
            .flatten_all()
            .and_then(|t| t.to_vec1::<f32>())
        {
            Ok(w) => w.into_iter().map(|x| x as f64).collect(),
            Err(e) => {
                log::error!("head_weight read failed: {e}");
                Vec::new()
            }
        }
    }

    /// Hidden width `h`, read from `l3.weight`'s `[1, h]` shape (a loaded model carries
    /// its own width).
    fn width(&self) -> usize {
        self.net.head().weight().dims().get(1).copied().unwrap_or(0)
    }
}

/// The default φ source: the frozen net's penultimate over the move's local patch.
impl FeatureSource for NeuralPrior {
    fn feat_dim(&self) -> usize {
        self.width()
    }
    fn warm_theta(&self, scale: f64) -> Vec<f64> {
        self.head_weight().into_iter().map(|w| w * scale).collect()
    }
    fn key_and_input(&self, scratch: &GameState, mv: &Move) -> (PatchKey, Vec<f32>) {
        super::features::encode_keyed(scratch, mv)
    }
    fn compute_features(&self, inputs: &[Vec<f32>]) -> Vec<Vec<f32>> {
        self.penult(inputs).unwrap_or_else(|e| {
            log::error!("feat penult inference failed: {e}");
            Vec::new()
        })
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

/// The value net: a whole-position scorer (policy+value / PUCT line). Input is a
/// [`super::position::VALUE_LEN`] vector (occupancy crop + scalars); output is a
/// scalar in (0, 1) = predicted final game length / 200. Same MLP shape as
/// [`PolicyNet`] but with a sigmoid head; value features are fixed-width so value
/// training mini-batches.
pub struct ValueNet {
    l1: Linear,
    l2: Linear,
    l3: Linear,
}

impl ValueNet {
    pub fn new(vb: VarBuilder) -> Result<Self> {
        Ok(Self {
            l1: linear(super::position::VALUE_LEN, HIDDEN, vb.pp("v1"))?,
            l2: linear(HIDDEN, HIDDEN, vb.pp("v2"))?,
            l3: linear(HIDDEN, 1, vb.pp("v3"))?,
        })
    }

    /// Forward `[N, VALUE_LEN]` → `[N]`: predicted normalised length in (0, 1).
    pub fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let x = self.l1.forward(x)?.relu()?;
        let x = self.l2.forward(&x)?.relu()?;
        let x = candle_nn::ops::sigmoid(&self.l3.forward(&x)?)?; // [N, 1]
        x.squeeze(1)
    }
}

/// A trained [`ValueNet`] ready to score positions (PUCT leaf eval). Inference runs
/// on CPU (one position at a time in the search), like the policy prior.
pub struct ValuePredictor {
    net: ValueNet,
    #[allow(dead_code)]
    varmap: VarMap,
    device: Device,
}

impl ValuePredictor {
    /// Wrap a freshly trained net + its varmap.
    pub fn new(net: ValueNet, varmap: VarMap, device: Device) -> Self {
        Self {
            net,
            varmap,
            device,
        }
    }

    /// Save / load the value weights (safetensors), like the policy net.
    pub fn save(&self, path: &str) -> Result<()> {
        self.varmap.save(path)
    }
    pub fn load(path: &str, device: Device) -> Result<Self> {
        let mut varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, candle_core::DType::F32, &device);
        let net = ValueNet::new(vb)?;
        varmap.load(path)?;
        Ok(Self {
            net,
            varmap,
            device,
        })
    }

    /// Estimated value (normalised final length, ~[0,1]) of a position. Returns a
    /// neutral 0.5 on the (unexpected) inference error so the search never breaks.
    pub fn value(&self, state: &crate::game::state::GameState) -> f32 {
        let f = super::position::encode_value_natural(state);
        let run = || -> Result<f32> {
            let x = Tensor::from_vec(f, (1, super::position::VALUE_LEN), &self.device)?;
            Ok(self.net.forward(&x)?.to_vec1::<f32>()?[0])
        };
        run().unwrap_or(0.5)
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
