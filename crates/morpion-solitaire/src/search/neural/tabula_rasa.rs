//! Tabula-rasa training of the neural move prior — **no human record corpus**.
//!
//! The product's "strong method" normally trains the prior on the bundled record
//! games. This module trains one **from scratch** by cold-start Expert Iteration
//! (the NeuralNRPA self-improvement loop):
//!
//! 1. **Round 0 (cold):** run plain NRPA (no prior) to generate the first games.
//! 2. **Each round:** keep the best games found so far (a diverse elite), train a
//!    fresh prior on *them only*, arm it, and search again — the apprentice chases
//!    the expert it just produced. The elite only improves, so (unlike a
//!    corpus-seeded loop, where weak self-play dilutes strong human data) there is
//!    no dilution: each prior learns from strictly-better-or-equal games.
//!
//! Everything here is **CPU-only** — game generation is CPU by design (the NRPA hot
//! loop is launch-latency bound, never GPU), and the small h64 prior trains on CPU
//! in seconds. So a from-scratch run needs no GPU and no committed weights; the same
//! `train` entry is exposed in the CLI (`tabula-rasa` subcommand).
//!
//! The prior is driven through the plugin [`BiasModifier`](crate::search::plugin::
//! BiasModifier) seam: [`super::prior::arm`] installs each round's prior, and
//! [`super::set_scale`] anneals its strength — the search picks them up with no
//! special coupling.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::net::NeuralPrior;
use crate::game::moves::Move;
use crate::game::rules::Variant;
use crate::game::state::GameState;
use crate::search::nrpa;
use crate::search::SearchState;

/// Tabula-rasa hyper-parameters. Defaults target a 5T fleet run; the CLI exposes the
/// knobs and a smoke config drops them to run in seconds.
#[derive(Debug, Clone)]
pub struct TabulaRasaConfig {
    pub variant: Variant,
    /// Expert-Iteration rounds (round 0 is the cold seed; 1.. are prior-guided).
    pub rounds: usize,
    /// Wall-clock search budget per round, split across `islands`.
    pub secs_per_round: f64,
    /// Independent searches per round — diversity in the elite (each fresh-cross
    /// NRPA run diverges by RNG), at no extra wall-clock cost (the budget is split).
    pub islands: usize,
    /// NRPA nesting level for generation (3 is the fast default).
    pub level: usize,
    /// Training epochs per round (the h64 net; ~CPU-seconds).
    pub epochs: usize,
    /// Adam learning rate.
    pub lr: f64,
    /// Max games retained in the elite (best-by-length, de-duplicated).
    pub elite: usize,
    /// Prior strength at the FIRST guided round (β scale; ~4 lifts fast).
    pub scale: f64,
    /// Prior strength at the LAST round — `scale` is **annealed** linearly down to
    /// this across the guided rounds. A strong fixed prior collapses perturbation's
    /// diversity (near-identical repairs → stagnation), so easing it off lets later
    /// rounds keep climbing. Set `scale_min == scale` for no annealing.
    pub scale_min: f64,
}

impl Default for TabulaRasaConfig {
    fn default() -> Self {
        Self {
            variant: Variant::T5,
            rounds: 12,
            secs_per_round: 60.0,
            islands: 4,
            level: 3,
            epochs: 30,
            lr: 1e-3,
            elite: 40,
            scale: 4.0,
            scale_min: 4.0,
        }
    }
}

impl TabulaRasaConfig {
    /// A tiny config that exercises the whole loop in a few seconds (for the smoke
    /// test / a "does it run" check) — not for score.
    pub fn smoke(variant: Variant) -> Self {
        Self {
            variant,
            rounds: 2,
            secs_per_round: 2.0,
            islands: 1,
            level: 1,
            epochs: 1,
            lr: 1e-3,
            elite: 4,
            scale: 4.0,
            scale_min: 4.0,
        }
    }
}

/// One Expert-Iteration round's outcome, reported to the caller as progress.
#[derive(Debug, Clone, Copy)]
pub struct Rung {
    /// 0-based round index (0 = cold seed).
    pub round: usize,
    /// Best game length generated this round.
    pub found: usize,
    /// Best game length over all rounds so far.
    pub best_ever: usize,
    /// The annealed β scale used for this round's prior-guided generation.
    pub scale: f64,
    /// Longest / shortest game currently in the elite.
    pub elite_max: usize,
    pub elite_min: usize,
    /// Number of games in the elite.
    pub elite_size: usize,
}

/// Run the cold-start Expert-Iteration loop and return the final trained prior
/// **and the elite it trained on** (the from-scratch corpus).
///
/// Round 0 seeds a diverse elite with plain NRPA from the empty cross. From round 1
/// on, each round runs prior-guided **perturbation** (large-neighbourhood search) on
/// the best elite games; improved games re-enter the elite, the prior retrains on the
/// better elite, and the next round works from there (NRPA → perturbation → NRPA → …).
///
/// `cancel` lets a caller stop early (a CLI signal): it is polled during each search
/// and between rounds; on cancel the loop returns the best prior trained so far.
/// `progress` is called once per completed round with its [`Rung`].
///
/// Arms each round's prior into the search hook for the duration (so the next round's
/// generation is guided), and disarms on return — the caller arms the returned prior
/// itself if it wants subsequent searches to use it.
pub fn train(
    cfg: &TabulaRasaConfig,
    cancel: &AtomicBool,
    mut progress: impl FnMut(Rung),
) -> candle_core::Result<(Arc<NeuralPrior>, Vec<Vec<Move>>)> {
    // Annealed β scale per round: `scale` at the first guided round, easing to
    // `scale_min` at the last — a strong fixed prior collapses perturbation's
    // diversity, so later rounds search with a lighter touch.
    let scale_at = |round: usize| -> f64 {
        if cfg.rounds <= 2 {
            return cfg.scale;
        }
        let t = ((round.saturating_sub(1)) as f64 / (cfg.rounds - 2) as f64).clamp(0.0, 1.0);
        cfg.scale + (cfg.scale_min - cfg.scale) * t
    };

    let islands = cfg.islands.max(1);
    let per = (cfg.secs_per_round / islands as f64).max(0.5);

    let mut elite: Vec<Vec<Move>> = Vec::new();
    let mut current: Option<Arc<NeuralPrior>> = None; // None on round 0 (cold)
    let mut last_prior: Option<Arc<NeuralPrior>> = None;
    let mut best_ever = 0usize;

    for round in 0..cfg.rounds {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        // 1. Generate this round's games with the current prior (cold on round 0),
        //    at the annealed β scale for this round.
        let scale = scale_at(round);
        super::set_scale(scale);
        super::prior::arm(current.clone());
        let mut round_max = 0usize;
        for i in 0..islands {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            // Round 0 (no elite yet) seeds with plain NRPA from the empty cross. From
            // round 1 on, the prior-guided PERTURBATION reworks the best elite games
            // (large-neighbourhood search). Each island perturbs a different top game
            // for diversity; the prior guides the inner-NRPA repairs.
            let g = if round == 0 || elite.is_empty() {
                generate_one(cfg.variant, cfg.level, per, cancel)
            } else {
                let seed = elite[i % elite.len()].clone();
                refine_one(cfg.variant, cfg.level, per, seed, cancel)
            };
            round_max = round_max.max(g.len());
            if !g.is_empty() {
                elite.push(g);
            }
        }
        super::prior::arm(None);
        best_ever = best_ever.max(round_max);

        // 2. Refresh the elite: drop exact duplicates, keep the best `elite` by
        //    length. Quadratic dedup is fine — the elite is tiny.
        let mut deduped: Vec<Vec<Move>> = Vec::with_capacity(elite.len());
        for g in elite.drain(..) {
            if !deduped.contains(&g) {
                deduped.push(g);
            }
        }
        deduped.sort_by_key(|g| std::cmp::Reverse(g.len()));
        deduped.truncate(cfg.elite);
        elite = deduped;
        if elite.is_empty() {
            continue; // no game generated yet (only possible if cancelled instantly)
        }

        // 3. Train a fresh prior on the elite only (no records) and make it current.
        let prior = Arc::new(super::prior::train_on_games(
            cfg.variant,
            &elite,
            cfg.epochs,
            cfg.lr,
        )?);
        current = Some(prior.clone());
        last_prior = Some(prior);

        progress(Rung {
            round,
            found: round_max,
            best_ever,
            scale,
            elite_max: elite.first().map(|g| g.len()).unwrap_or(0),
            elite_min: elite.last().map(|g| g.len()).unwrap_or(0),
            elite_size: elite.len(),
        });
    }

    super::prior::arm(None);
    super::reset_scale(); // restore the default scale for later searches
    let prior = last_prior.ok_or_else(|| {
        candle_core::Error::Msg("tabula-rasa produced no prior (cancelled before round 0)".into())
    })?;
    // `elite` is the from-scratch corpus the final prior was trained on.
    Ok((prior, elite))
}

/// Run one NRPA search (all cores) for `secs`, using whatever prior is armed, and
/// return its best full game. Stops early if `cancel` is set.
fn generate_one(variant: Variant, level: usize, secs: f64, cancel: &AtomicBool) -> Vec<Move> {
    let search = SearchState::new();
    let s2 = search.clone();
    let st = GameState::new(variant);
    search.running.store(true, Ordering::Relaxed);
    let handle = std::thread::spawn(move || nrpa::run(&st, s2, level));
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < secs && !cancel.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(50));
    }
    search.running.store(false, Ordering::Relaxed);
    let _ = handle.join();
    let g = search.best_sequence.read().unwrap().clone();
    g
}

/// Run one prior-guided **perturbation** seeded by `seed` (a known game) for `secs`,
/// returning its best full game. Perturbation destroys/repairs the seed and only
/// keeps improvements, so the result is ≥ the seed; the armed prior guides the
/// inner-NRPA repairs.
fn refine_one(
    variant: Variant,
    level: usize,
    secs: f64,
    seed: Vec<Move>,
    cancel: &AtomicBool,
) -> Vec<Move> {
    let search = SearchState::new();
    let s2 = search.clone();
    search.running.store(true, Ordering::Relaxed);
    let handle = std::thread::spawn(move || nrpa::run_perturbation(s2, level, seed, variant));
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < secs && !cancel.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(50));
    }
    search.running.store(false, Ordering::Relaxed);
    let _ = handle.join();
    let g = search.best_sequence.read().unwrap().clone();
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole cold-start loop runs end-to-end at a tiny budget and yields a prior
    /// trained on self-generated games (no records). Score is not asserted — this is
    /// a "does the pipeline work" smoke test.
    #[test]
    #[ignore = "tabula-rasa smoke (a few seconds), run with --features neural -- --ignored --nocapture"]
    fn tabula_rasa_smoke() {
        let cfg = TabulaRasaConfig::smoke(Variant::T5);
        let cancel = AtomicBool::new(false);
        let mut rungs = 0usize;
        let (prior, corpus) = train(&cfg, &cancel, |r| {
            rungs += 1;
            println!(
                "round {} found={} best_ever={} elite=[{}..{}]x{}",
                r.round, r.found, r.best_ever, r.elite_min, r.elite_max, r.elite_size
            );
        })
        .expect("tabula-rasa training should produce a prior");
        assert!(rungs >= 1, "at least one round should complete");
        assert!(!corpus.is_empty(), "the corpus (elite) should be non-empty");
        let st = GameState::new(Variant::T5);
        let moves = crate::game::moves::legal_moves(&st);
        let feats: Vec<Vec<f32>> = moves
            .iter()
            .map(|m| crate::search::neural::features::encode(&st, m))
            .collect();
        let logits = prior.logits(&feats).expect("prior forward pass");
        assert_eq!(logits.len(), moves.len());
    }
}
