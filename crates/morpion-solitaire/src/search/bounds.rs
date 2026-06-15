//! Upper-bound heuristics for branch-and-bound pruning.
//!
//! These are admissible (never underestimate the achievable future score) but
//! intentionally loose for speed. The caller can swap in tighter estimators
//! without changing the correctness of the search. For the theoretical limits on
//! how long a Morpion Solitaire game can be — the backdrop to any bound — see
//! the upper/lower bounds of ([Demaine2006]).
//!
//! [Demaine2006]: https://github.com/sjourdois/morpion-solitaire/blob/main/docs/BIBLIOGRAPHY.md

use crate::game::state::GameState;

/// Returns an optimistic upper bound on the total score reachable from
/// `state` (current score + future moves).  Current implementation just
/// returns the count of currently-legal moves added to the current score,
/// which is trivially admissible.
pub fn upper_bound(state: &GameState, legal_count: usize) -> u32 {
    state.score() as u32 + legal_count as u32
}
