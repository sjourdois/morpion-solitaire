//! PUCT (policy + value) tree search for single-player Morpion — the policy+value
//! line: it guides whole *lines* through a position value, not just per-move
//! imitation of the policy prior.
//!
//! One tree, grown by repeated simulations: descend by PUCT
//! (`Q + c·P·√ΣN/(1+n)`), expand the leaf with the policy net's move priors, and
//! evaluate it. Each expansion also runs a policy-guided rollout to a terminal —
//! grounding the value AND surfacing a real candidate game (recorded as the best so
//! far). The backup value is either that rollout's length (`LeafEval::Rollout`,
//! ≈ NMCS) or the value net's estimate (`LeafEval::Value`) — the A/B that isolates
//! whether value guidance helps. Single-player: we keep the best terminal found,
//! not an optimal policy. CPU inference (per-node net calls); native + `neural`.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use super::features::encode;
use super::net::{MovePrior, ValuePredictor};
use super::selfplay::rollout_into;
use crate::game::moves::{legal_moves, Move};
use crate::game::rules::Variant;
use crate::game::state::GameState;
use crate::search::SearchState;

/// How a freshly-expanded leaf is valued for backup.
#[derive(Clone, Copy, Debug)]
pub enum LeafEval {
    /// The length of a policy-guided rollout from the leaf (grounded; ≈ NMCS).
    Rollout,
    /// The value net's position estimate (tests value-net guidance).
    Value,
}

pub struct PuctConfig {
    pub c_puct: f64,
    pub leaf: LeafEval,
    pub rollout_inv_temp: f64,
}

struct Child {
    mv: Move,
    prior: f32,
    n: u32,
    w: f64,
    node: Option<usize>,
}

struct Node {
    expanded: bool,
    children: Vec<Child>,
}

/// Softmax move priors from the policy net over `moves` (numerically stable).
fn policy_priors(prior: &dyn MovePrior, state: &GameState, moves: &[Move]) -> Vec<f32> {
    let feats: Vec<Vec<f32>> = moves.iter().map(|m| encode(state, m)).collect();
    let biases = prior.biases(&feats);
    let maxb = biases.iter().cloned().fold(f64::MIN, f64::max);
    let exps: Vec<f64> = biases.iter().map(|b| (b - maxb).exp()).collect();
    let z: f64 = exps.iter().sum::<f64>().max(1e-12);
    exps.iter().map(|e| (e / z) as f32).collect()
}

/// PUCT child selection: argmax of `Q + c·P·√(ΣN)/(1+n)`.
fn select(node: &Node, c_puct: f64) -> usize {
    let total_n: u32 = node.children.iter().map(|c| c.n).sum();
    let sqrt_n = (total_n as f64).sqrt();
    let mut best = 0usize;
    let mut best_s = f64::MIN;
    for (i, c) in node.children.iter().enumerate() {
        let q = if c.n == 0 { 0.0 } else { c.w / c.n as f64 };
        let u = c_puct * c.prior as f64 * sqrt_n / (1.0 + c.n as f64);
        let s = q + u;
        if s > best_s {
            best_s = s;
            best = i;
        }
    }
    best
}

#[inline]
fn record(search: &SearchState, game: &[Move]) {
    let score = game.len() as u32;
    if score > search.best_score.load(Ordering::Relaxed) {
        search.record_best(score, game.to_vec());
    }
}

/// Run PUCT from the cross until `search.running` clears, recording the best
/// terminal game found. One tree, single thread.
pub fn run_puct(
    search: Arc<SearchState>,
    variant: Variant,
    policy: &dyn MovePrior,
    value: Option<&ValuePredictor>,
    cfg: &PuctConfig,
) {
    search.reset();
    search.running.store(true, Ordering::Relaxed);
    let root = GameState::new(variant);
    let mut arena: Vec<Node> = vec![Node {
        expanded: false,
        children: Vec::new(),
    }];
    while search.running.load(Ordering::Relaxed) {
        simulate(&mut arena, &root, policy, value, cfg, &search);
    }
    search.running.store(false, Ordering::Relaxed);
}

fn simulate(
    arena: &mut Vec<Node>,
    root: &GameState,
    policy: &dyn MovePrior,
    value: Option<&ValuePredictor>,
    cfg: &PuctConfig,
    search: &SearchState,
) {
    let mut scratch = root.clone();
    let mut path: Vec<(usize, usize)> = Vec::new();
    let mut idx = 0usize;
    let v: f64 = loop {
        if !arena[idx].expanded {
            break expand(arena, idx, &scratch, policy, value, cfg, search);
        }
        if arena[idx].children.is_empty() {
            break scratch.history.len() as f64 / 200.0; // terminal node revisited
        }
        let ci = select(&arena[idx], cfg.c_puct);
        // A move that would overflow the fixed grid is not applied (apply returns
        // false and sets GRID_OVERFLOW); stop the descent here and back up the current
        // depth rather than descending into a child whose move never landed (which
        // would desync `scratch` from the tree path and corrupt the Q backups).
        if !scratch.apply(arena[idx].children[ci].mv) {
            break scratch.history.len() as f64 / 200.0;
        }
        path.push((idx, ci));
        idx = match arena[idx].children[ci].node {
            Some(n) => n,
            None => {
                let n = arena.len();
                arena.push(Node {
                    expanded: false,
                    children: Vec::new(),
                });
                arena[idx].children[ci].node = Some(n);
                n
            }
        };
    };
    for (ni, ci) in path {
        let c = &mut arena[ni].children[ci];
        c.n += 1;
        c.w += v;
    }
}

fn expand(
    arena: &mut [Node],
    idx: usize,
    scratch: &GameState,
    policy: &dyn MovePrior,
    value: Option<&ValuePredictor>,
    cfg: &PuctConfig,
    search: &SearchState,
) -> f64 {
    search.nodes_explored.fetch_add(1, Ordering::Relaxed);
    let moves = legal_moves(scratch);
    if moves.is_empty() {
        arena[idx].expanded = true;
        record(search, &scratch.history);
        return scratch.history.len() as f64 / 200.0;
    }
    let priors = policy_priors(policy, scratch, &moves);
    arena[idx].children = moves
        .iter()
        .zip(priors)
        .map(|(m, p)| Child {
            mv: *m,
            prior: p,
            n: 0,
            w: 0.0,
            node: None,
        })
        .collect();
    arena[idx].expanded = true;

    // Always roll out to a terminal: it surfaces a real candidate game (recorded)
    // and grounds the Rollout value.
    let mut sc = scratch.clone();
    let rollout_len = rollout_into(&mut sc, Some(policy), cfg.rollout_inv_temp);
    record(search, &sc.history);

    match cfg.leaf {
        LeafEval::Value => value
            .map(|vp| vp.value(scratch) as f64)
            .unwrap_or(rollout_len as f64 / 200.0),
        LeafEval::Rollout => rollout_len as f64 / 200.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PUCT with a uniform policy grows its tree, records terminal games, and never
    /// hangs: spin it on a thread, wait until it has expanded some nodes, then stop.
    #[test]
    fn puct_uniform_records_a_game() {
        struct Zero;
        impl MovePrior for Zero {
            fn biases(&self, f: &[Vec<f32>]) -> Vec<f64> {
                vec![0.0; f.len()]
            }
        }
        let search = SearchState::new();
        let s2 = search.clone();
        let handle = std::thread::spawn(move || {
            let cfg = PuctConfig {
                c_puct: 1.5,
                leaf: LeafEval::Rollout,
                rollout_inv_temp: 1.0,
            };
            run_puct(s2, Variant::T5, &Zero, None, &cfg);
        });
        // Wait (bounded) until it has done real work, then ask it to stop.
        for _ in 0..200 {
            if search.nodes_explored.load(Ordering::Relaxed) >= 5 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        search.running.store(false, Ordering::Relaxed);
        handle.join().unwrap();
        assert!(
            search.best_score.load(Ordering::Relaxed) > 0,
            "a rollout should record at least one game"
        );
    }
}
