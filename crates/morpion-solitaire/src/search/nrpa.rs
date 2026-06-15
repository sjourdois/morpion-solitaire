//! Nested Rollout Policy Adaptation (NRPA), and a perturbation
//! (large-neighbourhood) search built on it.
//!
//! NRPA ([Rosin2011]) refines Nested Monte-Carlo Search ([Cazenave2009]). The
//! policy is a map `(state_hash XOR move_hash) → logit weight`: a level-0 playout
//! samples moves by `softmax(policy)`; a level-N (N>0) run does N playouts of
//! level N−1, adapts the policy toward the best, and returns the best sequence.
//! The optional density feature follows Generalized NRPA ([Edelkamp2016]).
//!
//! The perturbation search keeps a quality-diversity archive ([Mouret2015]) of
//! diverse high games and repeatedly destroys/repairs a suffix — a
//! large-neighbourhood search ([Shaw1998]).
//!
//! References are collected in `docs/BIBLIOGRAPHY.md`.
//!
//! [Rosin2011]: https://github.com/sjourdois/morpion-solitaire/blob/main/docs/BIBLIOGRAPHY.md
//! [Cazenave2009]: https://github.com/sjourdois/morpion-solitaire/blob/main/docs/BIBLIOGRAPHY.md
//! [Edelkamp2016]: https://github.com/sjourdois/morpion-solitaire/blob/main/docs/BIBLIOGRAPHY.md
//! [Mouret2015]: https://github.com/sjourdois/morpion-solitaire/blob/main/docs/BIBLIOGRAPHY.md
//! [Shaw1998]: https://github.com/sjourdois/morpion-solitaire/blob/main/docs/BIBLIOGRAPHY.md
use rustc_hash::FxHashMap;
use std::sync::{atomic::Ordering, Arc};

use rand::Rng;

use super::{symmetry::SymmetryHashes, SearchState};
use crate::game::{
    board::Pos,
    moves::{legal_moves, Move},
    state::GameState,
};

const NRPA_ALPHA: f64 = 1.0; // policy adaptation step size (sweet spot; 0.5 and 2.0 both regress to ~92)

/// GNRPA prior-bias strength (per occupied neighbour). 0 ⇒ plain NRPA.
/// Overridable via the GNRPA_BETA env var for sweeps (read once).
fn gnrpa_beta() -> f64 {
    use std::sync::OnceLock;
    static B: OnceLock<f64> = OnceLock::new();
    *B.get_or_init(|| {
        std::env::var("GNRPA_BETA")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0)
    })
}

/// GNRPA prior bias β(move): a fixed log-space preference added to a move's
/// learned weight in the softmax (Cazenave's Generalized NRPA). The framework is
/// correct (β=0 reproduces plain NRPA exactly), but this particular feature —
/// local connectedness (occupied 8-neighbours) — did NOT help in testing:
/// strong β (0.2) caused premature convergence (flat ~94 vs plain's ~99), and
/// weak β was lost in NRPA's large run-to-run variance. So it ships OFF by
/// default (`GNRPA_BETA=0`); the env knob is kept for future experiments with
/// better-motivated, averaged-over-many-runs features. The neighbour count is
/// symmetry-invariant (preserved by every D4 transform), so a non-zero bias
/// would stay consistent with the symmetry-invariant policy coding.
#[inline]
fn beta(state: &GameState, pos: Pos) -> f64 {
    let b = gnrpa_beta();
    if b == 0.0 {
        return 0.0; // GNRPA off (default): skip the neighbour scan entirely.
    }
    const NEIGHBOURS: [(i16, i16); 8] = [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, -1),
        (-1, 1),
        (1, 1),
        (-1, -1),
    ];
    let occ = NEIGHBOURS
        .iter()
        .filter(|&&(dx, dy)| state.board.contains((pos.0 + dx, pos.1 + dy)))
        .count();
    b * occ as f64
}

/// Iterations per nesting level — NRPA uses the same N at every level, so we set
/// it once from the top level. Measured sweet spot for level 3 on a ~minute
/// budget is ~500 (N=100 converges low ~91, N=1000 stalls the level-3 outer loop
/// ~93). For deeper levels we shrink N to hold each *outer* iteration's cost
/// roughly constant (N^(level-1) ≈ 500² ≈ 250k playouts), so the outer loop keeps
/// iterating instead of stalling inside one sub-search. Deeper levels learn more
/// cumulatively but only pay off over very long (multi-hour) runs.
fn iterations_for_level(level: usize) -> usize {
    match level {
        0..=3 => 500,
        4 => 64, // 64³ ≈ 262k
        _ => 24, // 24⁴ ≈ 332k  (level 5+)
    }
}

// Policy table keyed by symmetry-invariant move code. FxHash (not the std
// default SipHash) because get/entry are called ~20-30× per move in the softmax
// and adapt hot paths — billions of lookups per second.
type Policy = FxHashMap<u64, f64>;

/// Launch NRPA from `initial_state`.  Call from a background `std::thread`.
///
/// Island model: one island per core, each running independent NRPA(level)
/// restarts with a fresh policy (see `island`). The global best accumulates
/// across all islands via `search.record_best`; the symmetry-invariant policy
/// coding lets each island learn a sequence whatever its orientation.
pub fn run(initial_state: &GameState, search: Arc<SearchState>, level: usize) {
    run_seeded(initial_state, search, level, None);
}

/// Like [`run`] but each island restart starts from a clone of `seed` instead of
/// a blank policy — used to warm-start NRPA from a known strong game's policy.
fn run_seeded(
    initial_state: &GameState,
    search: Arc<SearchState>,
    level: usize,
    seed: Option<Policy>,
) {
    search.reset();
    spawn_islands(level, initial_state, &search, seed.as_ref());
}

/// Default warm-start strength: number of `adapt` passes over the seed game.
/// ~10 measurably lifts the best (e.g. 5T reached 137 vs a ~95 cold ceiling)
/// without merely replaying the seed.
pub const WARM_ITERS: usize = 10;

/// Launch NRPA warm-started from `seed_seq` (a known game's moves on the same
/// initial position): the policy is pre-trained toward that sequence so playouts
/// start biased toward its structure, then NRPA searches normally. The seed is a
/// soft prior — NRPA explores variations and the global best can exceed the seed.
pub fn run_warm(
    initial_state: &GameState,
    search: Arc<SearchState>,
    level: usize,
    seed_seq: &[Move],
    iters: usize,
) {
    let policy = build_warm_policy(initial_state, seed_seq, iters);
    search.reset();
    spawn_islands(level, initial_state, &search, Some(&policy));
}

/// Build a warm-start policy by running `iters` NRPA gradient steps toward `seq`.
fn build_warm_policy(initial: &GameState, seq: &[Move], iters: usize) -> Policy {
    let base_sym = build_base_sym(initial);
    let mut scratch = initial.clone();
    let mut policy = Policy::default();
    for _ in 0..iters {
        adapt(&mut policy, &mut scratch, &base_sym, seq);
    }
    policy
}

/// Perturbation destroy size K is drawn uniformly from this range each round —
/// a mix of small local refinements and large restructures. Capped to the
/// current game length.
#[cfg(not(target_arch = "wasm32"))]
const PERTURB_K_MIN: usize = 8;
#[cfg(not(target_arch = "wasm32"))]
const PERTURB_K_MAX: usize = 70;

/// Effort (round duration) scales with K: a tiny completion needs only a couple
/// of seconds, a 70-move reconstruction much more. `secs = K · per_k`, clamped.
#[cfg(not(target_arch = "wasm32"))]
const PERTURB_SECS_PER_K: f64 = 0.5;
#[cfg(not(target_arch = "wasm32"))]
const PERTURB_MIN_SECS: f64 = 2.0;
#[cfg(not(target_arch = "wasm32"))]
const PERTURB_MAX_SECS: f64 = 30.0;

/// Archive (population) memory: instead of a single annealing point, keep a
/// diverse pool of high games and perturb a random one each round. The archive
/// keeps every distinct game whose score is within `WINDOW` of the best seen
/// (so it holds a band of near-best games — diversity + escape room), capped at
/// `ARCHIVE_MAX`. A tabu set of game keys avoids re-processing duplicates.
#[cfg(not(target_arch = "wasm32"))]
const PERTURB_WINDOW: usize = 10;
#[cfg(not(target_arch = "wasm32"))]
const PERTURB_ARCHIVE_MAX: usize = 20_000;

/// Order- and (cheaply) duplicate-invariant key of a game: hash of its move set
/// sorted, so the same game found in a different play order maps to one key.
#[cfg(not(target_arch = "wasm32"))]
fn game_key(game: &[Move]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut keys: Vec<(i16, i16, i16, i16, u8, u8)> = game
        .iter()
        .map(|m| {
            (
                m.pos.0,
                m.pos.1,
                m.line.origin.0,
                m.line.origin.1,
                m.line.dir as u8,
                m.line_pos,
            )
        })
        .collect();
    keys.sort_unstable();
    let mut h = rustc_hash::FxHasher::default();
    keys.hash(&mut h);
    h.finish()
}

/// Perturbation (large-neighbourhood) search, native only. Keeps an archive of
/// diverse high games; each round it perturbs a random one (destroy K moves,
/// re-search the suffix warm-started toward the old ending) and inserts any new
/// game within `WINDOW` of the best. Improvements promote to the shared best
/// (display + auto-saved records). `seed` is the starting game (a loaded record);
/// an empty seed bootstraps from the cross.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_perturbation(
    search: Arc<SearchState>,
    level: usize,
    seed: Vec<Move>,
    variant: crate::game::rules::Variant,
) {
    let archive = if seed.is_empty() {
        Vec::new()
    } else {
        vec![seed]
    };
    perturbation_search(search, level, variant, archive);
}

/// Resume perturbation from a restored archive (the checkpoint's frontier).
#[cfg(not(target_arch = "wasm32"))]
pub fn resume_perturbation(
    search: Arc<SearchState>,
    level: usize,
    variant: crate::game::rules::Variant,
    archive: Vec<Vec<Move>>,
) {
    perturbation_search(search, level, variant, archive);
}

#[cfg(not(target_arch = "wasm32"))]
/// Reorder a game into a *random valid play order* (a random linear extension of
/// its dependency order). A move can be placed only once its line's other cells
/// are placed; conflicts impose no ordering (in a valid game all lines coexist).
/// Picking the smallest eligible move instead of a random one would give the
/// canonical (lexicographic-normal-form) order — same algorithm, different choice.
/// Used so perturbation destroys a *different region* each round, not always the
/// seed's recorded tail.
#[cfg(not(target_arch = "wasm32"))]
fn random_extension<R: rand::Rng>(
    game: &[Move],
    cross: &std::collections::HashSet<Pos>,
    line_len: u8,
    rng: &mut R,
) -> Vec<Move> {
    let mut placed: std::collections::HashSet<Pos> = cross.clone();
    let mut remaining = game.to_vec();
    let mut order = Vec::with_capacity(game.len());
    while !remaining.is_empty() {
        let playable: Vec<usize> = remaining
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                m.line
                    .positions(line_len)
                    .all(|c| c == m.pos || placed.contains(&c))
            })
            .map(|(i, _)| i)
            .collect();
        if playable.is_empty() {
            order.extend_from_slice(&remaining); // safety net (valid games never hit this)
            break;
        }
        let m = remaining.swap_remove(playable[rng.gen_range(0..playable.len())]);
        placed.insert(m.pos);
        order.push(m);
    }
    order
}

#[cfg(not(target_arch = "wasm32"))]
fn perturbation_search(
    search: Arc<SearchState>,
    level: usize,
    variant: crate::game::rules::Variant,
    mut archive: Vec<Vec<Move>>,
) {
    use std::collections::HashSet;
    use std::time::{Duration, Instant};
    search.reset();
    search.running.store(true, Ordering::Relaxed);

    // Tabu: keys of every game already processed (dedup + avoid re-exploring).
    let mut tabu: HashSet<u64> = HashSet::new();
    let mut max_score = 0usize;
    for g in &archive {
        tabu.insert(game_key(g));
        max_score = max_score.max(g.len());
    }
    if let Some(best) = archive.iter().max_by_key(|g| g.len()) {
        search.record_best(best.len() as u32, best.clone());
    }

    // Cross cells (no placing move) — the base of every dependency order.
    let cross: HashSet<Pos> = GameState::new(variant)
        .board
        .cells
        .iter()
        .copied()
        .collect();
    let line_len = variant.len();

    let mut rng = rand::thread_rng();
    let mut total_nodes = 0u64;
    while search.running.load(Ordering::Relaxed) {
        search.wait_if_paused(); // idle here between perturbation rounds
                                 // Checkpoint the archive when the app asks.
        if search.checkpoint_requested.swap(false, Ordering::Relaxed) {
            save_perturbation_checkpoint(variant, &archive, &search);
        }

        // Perturb a random archived game, reordered into a random valid play
        // order so the destroyed suffix covers a different region each round.
        let parent: Vec<Move> = if archive.is_empty() {
            Vec::new()
        } else {
            let g = archive[rng.gen_range(0..archive.len())].clone();
            random_extension(&g, &cross, line_len, &mut rng)
        };
        let k_max = PERTURB_K_MAX
            .min(parent.len().saturating_sub(1))
            .max(PERTURB_K_MIN);
        let k = rng.gen_range(PERTURB_K_MIN..=k_max);
        let round_secs = (k as f64 * PERTURB_SECS_PER_K).clamp(PERTURB_MIN_SECS, PERTURB_MAX_SECS);
        let prefix_len = parent.len().saturating_sub(k);
        let mut p = GameState::new(variant);
        for &mv in &parent[..prefix_len] {
            p.apply(mv);
        }
        let suffix: Vec<Move> = parent[prefix_len..].to_vec();

        // Inner warm NRPA search over completions of P, time-bounded by the loop.
        let inner = SearchState::new();
        let inner2 = inner.clone();
        let h = std::thread::spawn(move || run_warm(&p, inner2, level, &suffix, WARM_ITERS));
        let t0 = Instant::now();
        while t0.elapsed().as_secs_f64() < round_secs && search.running.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(100));
            let live = total_nodes + inner.nodes_explored.load(Ordering::Relaxed);
            search.nodes_explored.store(live, Ordering::Relaxed);
        }
        inner.running.store(false, Ordering::Relaxed);
        let _ = h.join();
        total_nodes += inner.nodes_explored.load(Ordering::Relaxed);
        search.nodes_explored.store(total_nodes, Ordering::Relaxed);

        // Insert a genuinely new game (tabu.insert returns true if not seen) when
        // it is within WINDOW of the best; promote real improvements.
        let cand = inner.best_sequence.read().unwrap().clone();
        if !cand.is_empty() && tabu.insert(game_key(&cand)) {
            let score = cand.len();
            if score > max_score {
                max_score = score;
                search.record_best(score as u32, cand.clone());
            }
            if score + PERTURB_WINDOW >= max_score {
                archive.push(cand);
                let cutoff = max_score.saturating_sub(PERTURB_WINDOW);
                archive.retain(|g| g.len() >= cutoff); // drop games now below the band
                if archive.len() > PERTURB_ARCHIVE_MAX {
                    if let Some(i) = (0..archive.len()).min_by_key(|&i| archive[i].len()) {
                        archive.swap_remove(i);
                    }
                }
            }
        }
    }
    search.running.store(false, Ordering::Relaxed);
}

/// Serialise the perturbation archive (as the checkpoint frontier) + the best.
#[cfg(not(target_arch = "wasm32"))]
fn save_perturbation_checkpoint(
    variant: crate::game::rules::Variant,
    archive: &[Vec<Move>],
    search: &SearchState,
) {
    let best = search.best_sequence.read().unwrap().clone();
    let records = search.records.read().unwrap().clone();
    let nodes = search.nodes_explored.load(Ordering::Relaxed);
    let serialized = match crate::game::io::export_checkpoint(
        variant,
        nodes,
        &best,
        &records,
        archive,
        "perturbation",
        crate::game::io::unix_now(),
    ) {
        Ok(s) => s,
        Err(e) => {
            log::error!("perturbation checkpoint serialise failed: {e}");
            return;
        }
    };
    if let Err(e) = crate::search::checkpoint::write("perturbation", &serialized) {
        log::error!("perturbation checkpoint write failed: {e}");
    }
}

/// Spawn one island per core and block until the search stops. Shared by
/// [`run`] (fresh start) and [`resume`] (after restoring a saved best). When
/// `seed` is `Some`, each restart clones it instead of starting blank.
fn spawn_islands(
    level: usize,
    initial_state: &GameState,
    search: &Arc<SearchState>,
    seed: Option<&Policy>,
) {
    search.running.store(true, Ordering::Relaxed);
    let n = iterations_for_level(level);
    let base_sym = build_base_sym(initial_state);
    let runs = rayon::current_num_threads().max(1);
    rayon::scope(|s| {
        for _ in 0..runs {
            s.spawn(|_| island(level, n, initial_state, &base_sym, search, seed));
        }
    });
    search.running.store(false, Ordering::Relaxed);
}

/// Snapshot the accumulated progress (best sequence, records, node count) to the
/// shared checkpoint file. NRPA is stochastic and restart-based — there is no
/// deterministic frontier to resume exactly, so the checkpoint preserves only
/// the best-found-so-far; the frontier is left empty and [`resume`] simply keeps
/// searching from that baseline. Safe to call from the UI thread while islands
/// run: it just reads the shared atomics/locks.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_checkpoint(variant: crate::game::rules::Variant, search: &SearchState) {
    let best = search.best_sequence.read().unwrap().clone();
    let records = search.records.read().unwrap().clone();
    let nodes = search.nodes_explored.load(Ordering::Relaxed);
    let serialized = match crate::game::io::export_checkpoint(
        variant,
        nodes,
        &best,
        &records,
        &[],
        "nrpa",
        crate::game::io::unix_now(),
    ) {
        Ok(s) => s,
        Err(e) => {
            log::error!("nrpa checkpoint serialise failed: {e}");
            return;
        }
    };
    if let Err(e) = crate::search::checkpoint::write("nrpa", &serialized) {
        log::error!("nrpa checkpoint write failed: {e}");
    }
}

/// Resume NRPA from a checkpoint: restore the best/records/nodes baseline, then
/// keep searching. The islands preserve the loaded best (via `record_best`'s
/// `fetch_max`) and try to beat it.
#[cfg(not(target_arch = "wasm32"))]
pub fn resume(search: Arc<SearchState>, checkpoint: crate::game::io::Checkpoint, level: usize) {
    search.reset();
    search
        .best_score
        .store(checkpoint.best.len() as u32, Ordering::Relaxed);
    *search.best_sequence.write().unwrap() = checkpoint.best;
    *search.records.write().unwrap() = checkpoint.records;
    search
        .nodes_explored
        .store(checkpoint.nodes_explored, Ordering::Relaxed);

    let initial_state = GameState::new(checkpoint.variant);
    spawn_islands(level, &initial_state, &search, None);
}

/// One island: repeatedly run a full NRPA(level) with a **fresh policy**. Each
/// run starts from a blank policy and explores a different basin, so restarts
/// supply the diversity a single converged policy lacks (NRPA converges to a
/// local optimum and then stops improving). The global best accumulates across
/// all islands and restarts via `record_best`.
fn island(
    level: usize,
    n: usize,
    initial_state: &GameState,
    base_sym: &SymmetryHashes,
    search: &Arc<SearchState>,
    seed: Option<&Policy>,
) {
    // One clone for the island's lifetime. `playout`/`adapt` apply moves onto
    // this scratch and undo them, always restoring it to `initial_state` — so
    // there is no per-playout clone of the ~14 KB game state.
    let mut scratch = initial_state.clone();
    while search.running.load(Ordering::Relaxed) {
        // Fresh policy each restart (diversity), or a clone of the warm-start
        // seed when one is supplied.
        let mut policy = seed.cloned().unwrap_or_default();
        nrpa(level, n, &mut policy, &mut scratch, base_sym, search);
    }
}

fn nrpa(
    level: usize,
    n: usize,
    policy: &mut Policy,
    scratch: &mut GameState,
    base_sym: &SymmetryHashes,
    search: &Arc<SearchState>,
) -> Vec<Move> {
    if !search.running.load(Ordering::Relaxed) {
        return Vec::new();
    }

    if level == 0 {
        return playout(policy, scratch, base_sym, search);
    }

    let mut best: Vec<Move> = Vec::new();
    for _ in 0..n {
        search.wait_if_paused(); // cooperative pause between sub-searches
        if !search.running.load(Ordering::Relaxed) {
            break;
        }
        let seq = nrpa(level - 1, n, policy, scratch, base_sym, search);
        if seq.len() > best.len() {
            best = seq;
        }
        adapt(policy, scratch, base_sym, &best);
    }
    best
}

fn playout(
    policy: &Policy,
    scratch: &mut GameState,
    base_sym: &SymmetryHashes,
    search: &Arc<SearchState>,
) -> Vec<Move> {
    let mut sym = base_sym.clone();
    let start = scratch.history.len();
    let mut rng = rand::thread_rng();

    loop {
        search.nodes_explored.fetch_add(1, Ordering::Relaxed);
        let moves = legal_moves(scratch);
        if moves.is_empty() {
            break;
        }

        // Softmax sampling (symmetry-invariant policy code per move). Build the
        // position coder once so canonical() isn't recomputed for every move.
        let coder = sym.move_coder();
        let weights: Vec<f64> = moves
            .iter()
            .map(|mv| {
                (policy.get(&coder.code(mv)).copied().unwrap_or(0.0) + beta(scratch, mv.pos)).exp()
            })
            .collect();
        let total: f64 = weights.iter().sum();
        let mut r = rng.gen::<f64>() * total;
        let mut chosen = moves.len() - 1;
        for (i, &w) in weights.iter().enumerate() {
            r -= w;
            if r <= 0.0 {
                chosen = i;
                break;
            }
        }

        // Stop cleanly if the move would overflow the fixed grid: the playout so
        // far is a valid game and gets recorded below; the flag tells the app to
        // save and alert (see board::GRID_OVERFLOW).
        if !scratch.apply(moves[chosen]) {
            break;
        }
        sym.toggle(moves[chosen].pos);
    }

    // The global best is the FULL game from the cross (initial prefix included);
    // `record_best` stores that. But what we return for adapt is only the
    // SUFFIX played from `initial_state` — the prefix is already on `scratch`, so
    // replaying the full history would double-apply it (and corrupt the state)
    // when searching from a mid-game position.
    let full = scratch.history.clone();
    let score = full.len() as u32;
    if score > search.best_score.load(Ordering::Relaxed) {
        search.record_best(score, full.clone());
    }
    let suffix = full[start..].to_vec();

    // Restore the scratch to its entry state (== initial) for the next playout.
    while scratch.history.len() > start {
        scratch.undo();
    }
    suffix
}

/// Adapt the policy toward `best_seq` (the standard NRPA gradient step): at each
/// state, raise the chosen move's weight by `α` and lower every legal move's
/// weight by `α · P(move)`, where `P` is its current softmax probability. The
/// net change sums to zero, nudging the policy toward the best sequence.
fn adapt(
    policy: &mut Policy,
    scratch: &mut GameState,
    base_sym: &SymmetryHashes,
    best_seq: &[Move],
) {
    let mut sym = base_sym.clone();
    let start = scratch.history.len();

    for &mv in best_seq {
        let moves = legal_moves(scratch);
        if moves.is_empty() {
            break;
        }
        // Softmax over the legal moves, read BEFORE this step's update (each
        // step touches distinct codes, so the running policy is clean). One
        // coder per position avoids recomputing canonical() per move.
        let coder = sym.move_coder();
        let exps: Vec<f64> = moves
            .iter()
            .map(|m| {
                (policy.get(&coder.code(m)).copied().unwrap_or(0.0) + beta(scratch, m.pos)).exp()
            })
            .collect();
        let z: f64 = exps.iter().sum();

        // chosen += α ; each legal -= α · P(move).
        *policy.entry(coder.code(&mv)).or_insert(0.0) += NRPA_ALPHA;
        for (m, &e_m) in moves.iter().zip(&exps) {
            *policy.entry(coder.code(m)).or_insert(0.0) -= NRPA_ALPHA * (e_m / z);
        }

        if !scratch.apply(mv) {
            break;
        }
        sym.toggle(mv.pos);
    }

    // Restore the scratch to its entry state (== initial) for the caller.
    while scratch.history.len() > start {
        scratch.undo();
    }
}

fn build_base_sym(state: &GameState) -> SymmetryHashes {
    let k = 2 * state.variant.len() as i16 - 1;
    let mut s = SymmetryHashes::new(k);
    for &cell in &state.board.cells {
        s.toggle(cell);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{rules::Variant, state::GameState};
    use std::time::Duration;

    /// Warm-start experiment: pre-train the NRPA policy on the Rosin-178 record,
    /// then compare WARM-started vs COLD (blank) NRPA over `NRPA_RUNS` runs each.
    /// Reports the best-score distribution AND how many runs merely *replayed*
    /// the seeded game (best sequence == Rosin) — the trivial-replay trap to
    /// avoid. Env: NRPA_LEVEL, NRPA_SECS, NRPA_RUNS, WARM_ITERS.
    #[test]
    #[ignore = "measurement, run with --ignored --nocapture"]
    fn measure_warm_start() {
        use crate::game::io::import_save;
        let envn = |k: &str, d: u64| {
            std::env::var(k)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(d)
        };
        let level = envn("NRPA_LEVEL", 3) as usize;
        let secs = envn("NRPA_SECS", 20);
        let runs = envn("NRPA_RUNS", 4) as usize;
        let warm_iters = envn("WARM_ITERS", 10) as usize;

        let saved = morpion_solitaire_records::RECORDS
            .iter()
            .find(|(n, _, _)| *n == "Rosin 178")
            .unwrap()
            .2
            .to_owned();
        let rosin = import_save(&saved).expect("import rosin178");
        assert_eq!(rosin.score(), 178);
        let seq = rosin.history.clone();
        let initial = GameState::new(Variant::T5);

        let policy = build_warm_policy(&initial, &seq, warm_iters);
        println!(
            "warm policy: {} weights (WARM_ITERS={warm_iters})",
            policy.len()
        );

        let run_batch = |seed: Option<Policy>| -> Vec<(u32, bool)> {
            (0..runs)
                .map(|_| {
                    let search = SearchState::new();
                    let s2 = search.clone();
                    let st = GameState::new(Variant::T5);
                    let seed2 = seed.clone();
                    let h = std::thread::spawn(move || run_seeded(&st, s2, level, seed2));
                    std::thread::sleep(Duration::from_secs(secs));
                    search.running.store(false, Ordering::Relaxed);
                    h.join().unwrap();
                    let best = search.best_score.load(Ordering::Relaxed);
                    let replay = *search.best_sequence.read().unwrap() == seq;
                    (best, replay)
                })
                .collect()
        };

        let warm = run_batch(Some(policy));
        let cold = run_batch(None);

        let report = |label: &str, r: &[(u32, bool)]| {
            let bests: Vec<u32> = r.iter().map(|x| x.0).collect();
            let mean = bests.iter().map(|&b| b as f64).sum::<f64>() / bests.len() as f64;
            let replays = r.iter().filter(|x| x.1).count();
            println!(
                "{label}: bests={bests:?} mean={mean:.1} max={} replays(==rosin)={replays}/{}",
                bests.iter().max().unwrap(),
                r.len(),
            );
        };
        report("WARM", &warm);
        report("COLD", &cold);
    }

    /// Perturbation (large-neighbourhood) search: start from a seed game, then
    /// each round destroy the last K moves and re-search the suffix from that
    /// fixed prefix (warm-started toward the original ending so it explores
    /// nearby), keeping the result if it is longer. Iterated, this climbs.
    /// Env: NRPA_LEVEL, NRPA_SECS (per round), ROUNDS, WARM_ITERS, SEED_LEN
    /// (truncate the Rosin seed to this length to demo climbing; 0 = full 178).
    #[test]
    #[ignore = "measurement, run with --ignored --nocapture"]
    fn measure_perturbation() {
        use crate::game::io::import_save;
        let envn = |k: &str, d: u64| {
            std::env::var(k)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(d)
        };
        let level = envn("NRPA_LEVEL", 3) as usize;
        let secs = envn("NRPA_SECS", 8);
        let rounds = envn("ROUNDS", 12) as usize;
        let warm_iters = envn("WARM_ITERS", 10) as usize;
        let seed_len = envn("SEED_LEN", 0) as usize;

        let rosin178 = morpion_solitaire_records::RECORDS
            .iter()
            .find(|(n, _, _)| *n == "Rosin 178")
            .unwrap()
            .2;
        let rosin = import_save(rosin178).unwrap();
        let variant = rosin.variant;
        let mut best: Vec<Move> = rosin.history.clone();
        if seed_len > 0 && seed_len < best.len() {
            best.truncate(seed_len);
        }
        println!(
            "seed={} moves level={level} secs={secs} warm_iters={warm_iters}",
            best.len()
        );

        for round in 0..rounds {
            let k = 5 + (round % 8) * 5; // cycle 5,10,...,40
            let prefix_len = best.len().saturating_sub(k);
            let mut p = GameState::new(variant);
            for &mv in &best[..prefix_len] {
                p.apply(mv);
            }
            let suffix: Vec<Move> = best[prefix_len..].to_vec();

            let search = SearchState::new();
            let s2 = search.clone();
            let pc = p.clone();
            let h = std::thread::spawn(move || run_warm(&pc, s2, level, &suffix, warm_iters));
            std::thread::sleep(Duration::from_secs(secs));
            search.running.store(false, Ordering::Relaxed);
            h.join().unwrap();

            let cand = search.best_sequence.read().unwrap().clone();
            // Strict re-validation on a fresh cross: how many of cand's moves are
            // actually legal in order? If < cand.len(), the search produced an
            // illegal sequence (bug).
            let valid_len = {
                let mut st = GameState::new(variant);
                let mut i = 0;
                while i < cand.len() && legal_moves(&st).contains(&cand[i]) {
                    st.apply(cand[i]);
                    i += 1;
                }
                i
            };
            let improved = cand.len() > best.len();
            if improved {
                best = cand.clone();
            }
            println!(
                "round {round:>2} k={k:>2} prefix={prefix_len:>3}: cand={:>3} valid={:>3} best={:>3} nodes={} {}",
                cand.len(), valid_len, best.len(),
                search.nodes_explored.load(Ordering::Relaxed),
                if improved { "IMPROVED" } else { "" },
            );
        }
        println!("FINAL best={}", best.len());
    }

    /// Averaged benchmark: NRPA's best-at-time is high variance, so a single run
    /// can't tell two configs apart. This runs `NRPA_RUNS` (default 8) fresh,
    /// independent searches *sequentially* (each gets the whole machine) for
    /// `NRPA_SECS` (default 20) seconds each, and reports the distribution of
    /// best scores (mean / std / min / max). Compare configs by running it twice
    /// with different `GNRPA_BETA` and checking whether the means differ by more
    /// than ~1 std. Env: NRPA_LEVEL, NRPA_SECS, NRPA_RUNS, GNRPA_BETA.
    #[test]
    #[ignore = "measurement, run with --ignored --nocapture"]
    fn measure_nrpa_averaged() {
        let env = |k: &str, d: u64| {
            std::env::var(k)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(d)
        };
        let level = env("NRPA_LEVEL", 3) as usize;
        let secs = env("NRPA_SECS", 20);
        let runs = env("NRPA_RUNS", 8) as usize;
        let beta = gnrpa_beta();

        let mut bests: Vec<u32> = Vec::with_capacity(runs);
        for r in 0..runs {
            let search = SearchState::new();
            let s2 = search.clone();
            let st = GameState::new(Variant::T5);
            let h = std::thread::spawn(move || run(&st, s2, level));
            std::thread::sleep(Duration::from_secs(secs));
            search.running.store(false, Ordering::Relaxed);
            h.join().unwrap();
            let best = search.best_score.load(Ordering::Relaxed);
            bests.push(best);
            println!("  run {r}: best={best}");
        }

        let n = bests.len() as f64;
        let mean = bests.iter().map(|&b| b as f64).sum::<f64>() / n;
        let std = (bests
            .iter()
            .map(|&b| (b as f64 - mean).powi(2))
            .sum::<f64>()
            / n)
            .sqrt();
        println!(
            "NRPA L{level} β={beta} {secs}s ×{runs} : mean={mean:.1} std={std:.1} min={} max={}",
            bests.iter().min().unwrap(),
            bests.iter().max().unwrap(),
        );
    }

    /// Progression curve: best 5T score over a long run, sampled every 5 s — to
    /// see whether the search keeps climbing toward the record or plateaus.
    #[test]
    #[ignore = "measurement, run with --ignored --nocapture"]
    fn measure_nrpa_long() {
        use std::time::Instant;
        // Level (and window) overridable: NRPA_LEVEL=4 NRPA_SECS=120 cargo test ...
        let level: usize = std::env::var("NRPA_LEVEL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3);
        let secs: u64 = std::env::var("NRPA_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);
        // Optional warm-start: WARM_ITERS>0 seeds the policy from rosin178.msr.
        let warm_iters: usize = std::env::var("WARM_ITERS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let seed = (warm_iters > 0).then(|| {
            let saved = morpion_solitaire_records::RECORDS
                .iter()
                .find(|(n, _, _)| *n == "Rosin 178")
                .unwrap()
                .2
                .to_owned();
            let seq = crate::game::io::import_save(&saved)
                .expect("import")
                .history;
            build_warm_policy(&GameState::new(Variant::T5), &seq, warm_iters)
        });
        println!("warm_iters={warm_iters}");
        let search = SearchState::new();
        let s2 = search.clone();
        let st = GameState::new(Variant::T5);
        let h = std::thread::spawn(move || run_seeded(&st, s2, level, seed));
        let start = Instant::now();
        for _ in 0..(secs / 5) {
            std::thread::sleep(Duration::from_secs(5));
            println!(
                "NRPA L{level} @ {:>3.0}s : best={} nodes={}",
                start.elapsed().as_secs_f64(),
                search.best_score.load(Ordering::Relaxed),
                search.nodes_explored.load(Ordering::Relaxed),
            );
        }
        search.running.store(false, Ordering::Relaxed);
        h.join().unwrap();
    }

    /// Checkpoint → stop → resume round-trip: the saved best is restored and the
    /// resumed search continues from it (never regresses).
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "measurement, run with --ignored --nocapture"]
    fn nrpa_checkpoint_resume() {
        let search = SearchState::new();
        let s2 = search.clone();
        let st = GameState::new(Variant::T5);
        let h = std::thread::spawn(move || run(&st, s2, 3));
        std::thread::sleep(Duration::from_secs(6));
        save_checkpoint(Variant::T5, &search);
        search.running.store(false, Ordering::Relaxed);
        h.join().unwrap();
        let best_before = search.best_score.load(Ordering::Relaxed);

        let cp = crate::search::checkpoint::load("nrpa").expect("checkpoint on disk");
        assert_eq!(cp.algo, "nrpa");
        assert!(cp.frontier.is_empty(), "NRPA checkpoint has no frontier");
        assert_eq!(cp.best.len() as u32, best_before);

        let search2 = SearchState::new();
        let r2 = search2.clone();
        let h2 = std::thread::spawn(move || resume(r2, cp, 3));
        std::thread::sleep(Duration::from_secs(6));
        search2.running.store(false, Ordering::Relaxed);
        h2.join().unwrap();
        let best_after = search2.best_score.load(Ordering::Relaxed);

        println!("NRPA RESUME: best {best_before} -> {best_after}");
        assert!(best_after >= best_before, "resumed best must not regress");
    }

    /// Best 5T score NRPA reaches in a fixed window, per level. A working policy
    /// should clear a pure-random rollout (~40-50) by a wide margin.
    #[test]
    #[ignore = "measurement, run with --ignored --nocapture"]
    fn measure_nrpa() {
        for level in [1usize, 2, 3] {
            let search = SearchState::new();
            let s2 = search.clone();
            let st = GameState::new(Variant::T5);
            let h = std::thread::spawn(move || run(&st, s2, level));
            std::thread::sleep(Duration::from_secs(8));
            search.running.store(false, Ordering::Relaxed);
            h.join().unwrap();
            println!(
                "NRPA L{level}: best={} playouts-nodes={}",
                search.best_score.load(Ordering::Relaxed),
                search.nodes_explored.load(Ordering::Relaxed),
            );
        }
    }
}
