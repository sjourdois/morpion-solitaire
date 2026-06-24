//! Self-play game generation — varied-length games for value-net training, and a
//! building block for the PUCT search. Native + `neural` (uses the policy net).

use rand::RngExt;

use super::features::encode;
use super::net::MovePrior;
use crate::game::moves::{legal_moves_into, Move};
use crate::game::rules::Variant;
use crate::game::state::GameState;

/// Play one full game from the cross to a terminal position. With `prior = None`,
/// moves are uniform random (short games). With a prior, each move is sampled by
/// `softmax(inv_temp · bias)` over the legal moves — `inv_temp = 0` is uniform,
/// large `inv_temp` is greedy (long games). Sweeping `inv_temp` spans game lengths,
/// the spread the value net needs.
pub fn rollout_game(variant: Variant, prior: Option<&dyn MovePrior>, inv_temp: f64) -> Vec<Move> {
    let mut st = GameState::new(variant);
    rollout_into(&mut st, prior, inv_temp);
    st.history
}

/// Play `st` from its current position to a terminal one (in place), sampling moves
/// like [`rollout_game`]. Used as the PUCT leaf evaluation (a grounded estimate +
/// it surfaces a real candidate game). Returns the final length.
pub fn rollout_into(st: &mut GameState, prior: Option<&dyn MovePrior>, inv_temp: f64) -> usize {
    let mut rng = rand::rng();
    let mut moves: Vec<Move> = Vec::new();
    loop {
        legal_moves_into(st, &mut moves);
        if moves.is_empty() {
            break;
        }
        let idx = match prior {
            None => rng.random_range(0..moves.len()),
            Some(p) => {
                let feats: Vec<Vec<f32>> = moves.iter().map(|m| encode(st, m)).collect();
                let biases = p.biases(&feats);
                let mut total = 0.0;
                let weights: Vec<f64> = biases
                    .iter()
                    .map(|b| {
                        let w = (b * inv_temp).exp();
                        total += w;
                        w
                    })
                    .collect();
                let mut r = rng.random::<f64>() * total;
                let mut c = moves.len() - 1;
                for (i, &w) in weights.iter().enumerate() {
                    r -= w;
                    if r <= 0.0 {
                        c = i;
                        break;
                    }
                }
                c
            }
        };
        if !st.apply(moves[idx]) {
            break;
        }
    }
    st.history.len()
}

/// A length-varied set of games: `n` uniform-random (short), plus — if a `prior` is
/// given — `n` at each `inv_temp` in `temps` (mid → long). The spread the value net
/// needs to learn position quality.
pub fn varied_games(
    variant: Variant,
    n: usize,
    prior: Option<&dyn MovePrior>,
    temps: &[f64],
) -> Vec<Vec<Move>> {
    let mut games = Vec::with_capacity(n * (1 + temps.len()));
    for _ in 0..n {
        games.push(rollout_game(variant, None, 0.0));
    }
    if let Some(p) = prior {
        for &t in temps {
            for _ in 0..n {
                games.push(rollout_game(variant, Some(p), t));
            }
        }
    }
    games
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Random rollouts terminate and give short-ish games; the generator runs.
    #[test]
    fn random_rollouts_produce_games() {
        let g = rollout_game(Variant::T5, None, 0.0);
        assert!(!g.is_empty(), "a rollout should place at least one move");
        // Random 5T games are far below the record.
        assert!(g.len() < 130, "random game unexpectedly long: {}", g.len());
    }
}
