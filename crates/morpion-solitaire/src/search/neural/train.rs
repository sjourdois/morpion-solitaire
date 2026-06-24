//! Supervised training of the policy net by imitation of strong games.
//!
//! Each [`StateSample`] is a single classification example with a variable number
//! of classes — the position's legal moves — and one correct class, the move the
//! expert played. The loss is the softmax cross-entropy of the net's per-move
//! logits against that chosen index; minimising it raises the probability the
//! playout policy assigns to expert moves. This is the offline, supervised seed of
//! the neural prior; tabula-rasa Expert Iteration grows the set from the search's
//! own games (a later commit).
//!
//! CPU-only here: the supervised corpus seed trains in ~40 s on CPU, and the search
//! infers on CPU anyway. GPU training (the `neural-cuda`/`neural-metal` features) is
//! a later device-selection option — it would only speed tabula-rasa's many rounds.

use candle_core::{Device, Result, Tensor};
use candle_nn::{loss::cross_entropy, AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};

use super::dataset::StateSample;
use super::features::FEATURE_LEN;
use super::net::{NeuralPrior, PolicyNet};

/// Training hyper-parameters.
#[derive(Debug, Clone)]
pub struct TrainConfig {
    pub epochs: usize,
    pub lr: f64,
}

impl Default for TrainConfig {
    fn default() -> Self {
        Self { epochs: 40, lr: 1e-3 }
    }
}

/// Mean cross-entropy loss and top-1 accuracy of `prior` over `samples` (no
/// gradient). Top-1 = the fraction of positions whose highest-logit move is the
/// one the expert played — the headline imitation metric.
pub fn evaluate(prior: &NeuralPrior, samples: &[StateSample]) -> (f64, f64) {
    let mut loss_sum = 0.0;
    let mut correct = 0usize;
    let mut counted = 0usize;
    for s in samples {
        if s.moves.len() < 2 {
            continue; // a forced move teaches nothing (its softmax prob is 1)
        }
        let Ok(logits) = prior.logits(&s.moves) else {
            continue;
        };
        let argmax = logits
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);
        if argmax == s.chosen {
            correct += 1;
        }
        let max = logits.iter().cloned().fold(f32::MIN, f32::max);
        let denom: f32 = logits.iter().map(|l| (l - max).exp()).sum();
        let p = (logits[s.chosen] - max).exp() / denom;
        loss_sum += -(p.max(1e-12) as f64).ln();
        counted += 1;
    }
    if counted == 0 {
        return (0.0, 0.0);
    }
    (loss_sum / counted as f64, correct as f64 / counted as f64)
}

/// Train a fresh [`NeuralPrior`] on `samples` by imitation, on `device`.
pub fn train(samples: &[StateSample], cfg: &TrainConfig, device: Device) -> Result<NeuralPrior> {
    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, candle_core::DType::F32, &device);
    let net = PolicyNet::new(vb)?;
    let mut opt = AdamW::new(
        varmap.all_vars(),
        ParamsAdamW {
            lr: cfg.lr,
            ..Default::default()
        },
    )?;

    for _epoch in 0..cfg.epochs {
        for s in samples {
            if s.moves.len() < 2 {
                continue; // forced move: zero gradient signal
            }
            // [M, FEATURE_LEN] → [M] logits → [1, M] (one example, M classes).
            let n = s.moves.len();
            let flat: Vec<f32> = s.moves.iter().flat_map(|f| f.iter().copied()).collect();
            let x = Tensor::from_vec(flat, (n, FEATURE_LEN), &device)?;
            let logits = net.forward(&x)?.reshape((1, n))?;
            let target = Tensor::from_vec(vec![s.chosen as u32], 1, &device)?;
            let loss = cross_entropy(&logits, &target)?;
            opt.backward_step(&loss)?;
        }
    }

    Ok(NeuralPrior::new(net, varmap, device))
}

/// Train the **value** net by regression (mini-batched MSE) on `samples`. Value
/// features are fixed-width, so unlike the policy net this batches. Returns the net
/// wrapped with its `VarMap` (kept alive).
pub fn train_value(
    samples: &[super::dataset::ValueSample],
    cfg: &TrainConfig,
    batch: usize,
    device: Device,
) -> Result<(VarMap, super::net::ValueNet)> {
    use super::position::VALUE_LEN;
    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, candle_core::DType::F32, &device);
    let net = super::net::ValueNet::new(vb)?;
    let mut opt = AdamW::new(
        varmap.all_vars(),
        ParamsAdamW {
            lr: cfg.lr,
            ..Default::default()
        },
    )?;
    for _epoch in 0..cfg.epochs {
        for chunk in samples.chunks(batch.max(1)) {
            let b = chunk.len();
            let flat: Vec<f32> = chunk.iter().flat_map(|s| s.features.iter().copied()).collect();
            let x = Tensor::from_vec(flat, (b, VALUE_LEN), &device)?;
            let tgt = Tensor::from_vec(chunk.iter().map(|s| s.target).collect::<Vec<_>>(), b, &device)?;
            let loss = net.forward(&x)?.sub(&tgt)?.sqr()?.mean_all()?;
            opt.backward_step(&loss)?;
        }
    }
    Ok((varmap, net))
}

/// Mean-squared error of the value net over `samples` (normalised units).
pub fn value_mse(
    net: &super::net::ValueNet,
    samples: &[super::dataset::ValueSample],
    device: &Device,
) -> f64 {
    use super::position::VALUE_LEN;
    let mut se = 0.0;
    let mut n = 0usize;
    for chunk in samples.chunks(512) {
        let b = chunk.len();
        let flat: Vec<f32> = chunk.iter().flat_map(|s| s.features.iter().copied()).collect();
        let Ok(x) = Tensor::from_vec(flat, (b, VALUE_LEN), device) else {
            continue;
        };
        let Ok(pred) = net.forward(&x).and_then(|t| t.to_vec1::<f32>()) else {
            continue;
        };
        for (p, s) in pred.iter().zip(chunk) {
            se += (p - s.target).powi(2) as f64;
            n += 1;
        }
    }
    if n == 0 {
        0.0
    } else {
        se / n as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::rules::Variant;
    use crate::search::neural::dataset::samples_from_corpus;

    /// Imitation learning works: training on the 5T record corpus lowers the loss
    /// and lifts top-1 accuracy above an untrained net. A net that can't imitate
    /// records won't guide NRPA. Kept short (8 epochs) so it stays a fast check.
    #[test]
    #[ignore = "trains a net (~seconds); run with --features neural -- --ignored"]
    fn training_improves_imitation_on_corpus() {
        let samples = samples_from_corpus(Variant::T5);
        assert!(!samples.is_empty());
        let before = train(&samples, &TrainConfig { epochs: 0, lr: 1e-3 }, Device::Cpu).unwrap();
        let (loss0, acc0) = evaluate(&before, &samples);
        let after = train(&samples, &TrainConfig { epochs: 8, lr: 1e-3 }, Device::Cpu).unwrap();
        let (loss1, acc1) = evaluate(&after, &samples);
        println!("imitation: loss {loss0:.3} -> {loss1:.3}, top1 {acc0:.3} -> {acc1:.3}");
        assert!(loss1 < loss0, "training must reduce loss ({loss0} -> {loss1})");
        assert!(acc1 > acc0 + 0.05, "training must lift top-1 ({acc0} -> {acc1})");
    }
}
