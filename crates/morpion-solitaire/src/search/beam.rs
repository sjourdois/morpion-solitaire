//! Beam Search: keep the top-`width` states at each depth level.
//!
//! States are scored by a weighted combination of current score and the
//! number of remaining legal moves (as a heuristic for future potential).
//! The beam is expanded breadth-first until all beams reach a terminal state.
//! Beam and parallel/island variants of nested search are surveyed in
//! ([Cazenave2020]).
//!
//! [Cazenave2020]: https://github.com/sjourdois/morpion-solitaire/blob/main/docs/BIBLIOGRAPHY.md
use std::sync::{atomic::Ordering, Arc};

use super::SearchState;
use crate::game::{moves::legal_moves, state::GameState};

const LEGAL_WEIGHT: f64 = 0.5;

/// Launch beam search from `initial_state`.  Call from a background thread.
pub fn run(initial_state: &GameState, search: Arc<SearchState>, width: usize) {
    search.reset();
    search.running.store(true, Ordering::Relaxed);

    let mut beam: Vec<GameState> = vec![initial_state.clone()];

    loop {
        if !search.running.load(Ordering::Relaxed) {
            break;
        }

        let mut next: Vec<GameState> = Vec::with_capacity(beam.len() * 4);
        let mut all_terminal = true;

        for state in &beam {
            search.nodes_explored.fetch_add(1, Ordering::Relaxed);
            let moves = legal_moves(state);

            if moves.is_empty() {
                // Terminal: record if best
                let score = state.score() as u32;
                search.record_best(score, state.history.clone());
                continue;
            }

            all_terminal = false;

            for mv in moves {
                let mut child = state.clone();
                child.apply(mv);
                next.push(child);
            }
        }

        if all_terminal || next.is_empty() {
            break;
        }

        // Score and trim to beam width.
        next.sort_by(|a, b| {
            let sa = beam_score(a);
            let sb = beam_score(b);
            sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
        });
        next.truncate(width);
        beam = next;
    }

    search.running.store(false, Ordering::Relaxed);
}

fn beam_score(state: &GameState) -> f64 {
    let legal = legal_moves(state);
    state.score() as f64 + LEGAL_WEIGHT * legal.len() as f64
}
