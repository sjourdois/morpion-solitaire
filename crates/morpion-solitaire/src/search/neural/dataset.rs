//! Training-example extraction for the neural move prior.
//!
//! A strong game is a sequence of expert choices: at each played position, the
//! move that was actually played is the *positive* example and every other legal
//! move is a *negative*. Replaying a game (records, or games NRPA/perturbation
//! produce) yields, per position, the feature vectors of all legal moves plus the
//! index of the chosen one — exactly the supervision a softmax policy needs
//! (cross-entropy toward the chosen move). We also carry the game's final score so
//! a value head can be trained later (the value/PUCT path lands in a later commit).

use super::features::{encode, encode_orientation};
use crate::game::moves::{legal_moves, Move};
use crate::game::rules::Variant;
use crate::game::state::GameState;

/// One decision point in a game: the encoded legal moves, which one was played,
/// and the game's final length (the value target for that position).
#[derive(Debug, Clone)]
pub struct StateSample {
    /// Feature vector per legal move (each [`super::features::FEATURE_LEN`] long).
    pub moves: Vec<Vec<f32>>,
    /// Index into `moves` of the move actually played (the policy target).
    pub chosen: usize,
    /// Final score (length) of the game this position belongs to.
    #[allow(dead_code)] // read by the value/PUCT path (later commit)
    pub final_score: u32,
}

/// Extract one [`StateSample`] per played move by replaying `history` from the
/// initial cross of `variant`. A move in `history` that is not legal at its
/// position (a corrupt or rule-mismatched game) ends extraction early. The final
/// score is `history.len()` — the terminal length of the supplied game.
pub fn samples_from_game(variant: Variant, history: &[Move]) -> Vec<StateSample> {
    let final_score = history.len() as u32;
    let mut st = GameState::new(variant);
    let mut out = Vec::with_capacity(history.len());
    for &mv in history {
        let legal = legal_moves(&st);
        let Some(chosen) = legal.iter().position(|m| *m == mv) else {
            break; // game diverges from our rules — stop cleanly (shouldn't happen)
        };
        let moves = legal.iter().map(|m| encode(&st, m)).collect();
        out.push(StateSample {
            moves,
            chosen,
            final_score,
        });
        if !st.apply(mv) {
            break; // grid overflow — keep what we have
        }
    }
    out
}

/// Extract training samples from the bundled record corpus, restricted to a variant
/// (5T by default — the campaign's target). Games that fail to import or diverge
/// from our rules are skipped.
pub fn samples_from_corpus(variant: Variant) -> Vec<StateSample> {
    let mut out = Vec::new();
    for rec in morpion_solitaire_records::RECORDS.iter() {
        let Ok(g) = crate::game::io::import_save(rec.2) else {
            continue;
        };
        if g.variant != variant {
            continue;
        }
        out.extend(samples_from_game(g.variant, &g.history));
    }
    out
}

/// One [`StateSample`] per played move, encoded under D4 transform `t`. The same
/// `t` is applied to every legal move at a position, so the softmax-over-moves
/// structure (and the chosen index) is preserved — it is just the position viewed
/// in a rotated/reflected frame. Used to build the augmented training set.
fn samples_from_game_oriented(variant: Variant, history: &[Move], t: usize) -> Vec<StateSample> {
    let final_score = history.len() as u32;
    let mut st = GameState::new(variant);
    let mut out = Vec::with_capacity(history.len());
    for &mv in history {
        let legal = legal_moves(&st);
        let Some(chosen) = legal.iter().position(|m| *m == mv) else {
            break;
        };
        let moves = legal.iter().map(|m| encode_orientation(&st, m, t)).collect();
        out.push(StateSample {
            moves,
            chosen,
            final_score,
        });
        if !st.apply(mv) {
            break;
        }
    }
    out
}

/// The **D4-augmented** corpus training set: every position encoded in all eight
/// orientations. This is how the net learns symmetry invariance when the runtime
/// encoder is *not* canonicalising (the default cheap mode). 8× the samples of
/// [`samples_from_corpus`].
pub fn augmented_samples_from_corpus(variant: Variant) -> Vec<StateSample> {
    let mut out = Vec::new();
    for rec in morpion_solitaire_records::RECORDS.iter() {
        let Ok(g) = crate::game::io::import_save(rec.2) else {
            continue;
        };
        if g.variant != variant {
            continue;
        }
        for t in 0..8 {
            out.extend(samples_from_game_oriented(g.variant, &g.history, t));
        }
    }
    out
}

/// The D4-augmented sample set for arbitrary games (e.g. the bundled from-scratch
/// corpus, or the best games a search finds during Expert Iteration). Each game is
/// encoded in all eight orientations.
pub fn augmented_samples_from_games(variant: Variant, games: &[Vec<Move>]) -> Vec<StateSample> {
    let mut out = Vec::new();
    for g in games {
        for t in 0..8 {
            out.extend(samples_from_game_oriented(variant, g, t));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::moves::legal_moves;
    use crate::search::neural::features::FEATURE_LEN;

    /// A self-play-style game: every sample's `chosen` indexes a valid legal move,
    /// the feature vectors have the right width, and the count matches the game.
    #[test]
    fn samples_match_a_replayed_game() {
        // Build a short legal game greedily (first legal move each step).
        let mut st = GameState::new(Variant::T5);
        let mut history = Vec::new();
        for _ in 0..15 {
            let ms = legal_moves(&st);
            if ms.is_empty() {
                break;
            }
            history.push(ms[0]);
            st.apply(ms[0]);
        }
        let samples = samples_from_game(Variant::T5, &history);
        assert_eq!(samples.len(), history.len());
        for s in &samples {
            assert!(s.chosen < s.moves.len());
            assert_eq!(s.final_score, history.len() as u32);
            for f in &s.moves {
                assert_eq!(f.len(), FEATURE_LEN);
            }
        }
    }

    /// The record corpus yields a substantial 5T training set: many positions, and
    /// the best record contributes ~178 of them.
    #[test]
    fn corpus_yields_5t_samples() {
        let samples = samples_from_corpus(Variant::T5);
        assert!(
            samples.len() > 178,
            "expected hundreds of 5T decision points, got {}",
            samples.len()
        );
        assert!(samples.iter().all(|s| s.chosen < s.moves.len()));
    }
}
