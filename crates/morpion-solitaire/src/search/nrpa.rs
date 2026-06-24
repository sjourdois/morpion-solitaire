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

use rand::RngExt; // rand 0.10 moved random()/random_range() onto RngExt (was Rng)

use super::{
    symmetry::{local_code, MoveCoder, SymmetryHashes},
    SearchState,
};
use crate::game::{
    board::Pos,
    moves::{child_legal_moves_into, legal_moves_into, Move},
    state::GameState,
};

// ---- CLI run-config overrides ---------------------------------------------
// The CLI exposes the core tuning levers as proper options, never env vars. Each
// lever is a process-global atomic set once *before* a search launches, so every
// island thread observes it; when unset, the reader returns the baked-in default.
// One search runs at a time, so a single global slot is unambiguous.
const F64_UNSET: u64 = u64::MAX; // f64-bits sentinel ⇒ "not overridden"
static SYM_OVR: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0); // 0 unset · 1 off · 2 on
static CLAMP_OVR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(F64_UNSET); // f64 bits; ≤0 ⇒ off
static ALPHA_OVR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(F64_UNSET);
#[cfg(not(target_arch = "wasm32"))]
static KMIN_OVR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0); // 0 ⇒ unset
#[cfg(not(target_arch = "wasm32"))]
static KMAX_OVR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(not(target_arch = "wasm32"))]
static WINDOW_OVR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Override symmetry-invariant move coding (`--symmetry` / `--no-symmetry`).
#[allow(dead_code)] // used by the CLI
pub fn set_sym_override(on: bool) {
    SYM_OVR.store(if on { 2 } else { 1 }, Ordering::Relaxed);
}
/// Override the Stabilized-NRPA logit clamp C (`--clamp`; `0` ⇒ no clamping).
#[allow(dead_code)]
pub fn set_clamp_override(c: f64) {
    CLAMP_OVR.store(c.to_bits(), Ordering::Relaxed);
}
/// Override the policy adaptation step size α (`--alpha`).
#[allow(dead_code)]
pub fn set_alpha_override(alpha: f64) {
    ALPHA_OVR.store(alpha.to_bits(), Ordering::Relaxed);
}
/// Override the perturbation destroy-size lower bound K_min (`--kmin`).
#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn set_perturb_k_min_override(k: usize) {
    KMIN_OVR.store(k, Ordering::Relaxed);
}
/// Override the perturbation destroy-size upper bound K_max (`--kmax`).
#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn set_perturb_k_max_override(k: usize) {
    KMAX_OVR.store(k, Ordering::Relaxed);
}
/// Override the perturbation tabu/preservation window (`--window`).
#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn set_perturb_window_override(w: usize) {
    WINDOW_OVR.store(w, Ordering::Relaxed);
}

/// Policy adaptation step size α. Default 1.0 (the unclamped sweet spot; 0.5 and
/// 2.0 both regressed to ~92 *without* clamping). Clamping changes the dynamics,
/// so `--alpha` re-opens it for sweeping under clamp.
fn nrpa_alpha() -> f64 {
    let o = ALPHA_OVR.load(Ordering::Relaxed);
    if o != F64_UNSET {
        let a = f64::from_bits(o);
        if a > 0.0 {
            return a;
        }
    }
    1.0
}

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

/// Corpus-learned local prior strength (`NRPA_CORPUS`, default 0 = off). Scales
/// the imitation bias built by [`corpus_prior`] before it enters the softmax.
fn nrpa_corpus() -> f64 {
    use std::sync::OnceLock;
    static C: OnceLock<f64> = OnceLock::new();
    *C.get_or_init(|| {
        std::env::var("NRPA_CORPUS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0)
    })
}

/// Is any GNRPA prior active? When false (the default), the hot loops take the
/// fast path: an unseen policy code has softmax weight exp(0)=1 with no scan.
#[inline]
fn prior_active() -> bool {
    gnrpa_beta() != 0.0 || nrpa_corpus() != 0.0
}

/// Imitation prior learned from the record corpus (GNRPA "done right"): a log-odds
/// bias per **symmetry-invariant local neighbourhood** ([`local_code`]). For every
/// move played in a record game we tally its local pattern as *chosen*, and every
/// legal alternative's pattern as *available*; the bias of a pattern is
/// `ln((chosen+1)/(available+1))`, so patterns over-represented among record moves
/// get a positive softmax nudge. A constant shift across patterns is irrelevant
/// (softmax is shift-invariant), so no normaliser is needed. Built once, lazily.
///
/// This is the corpus-learned variant of the local feature; the hand-set
/// neighbour-count β and the raw local code (as move *identity*) both failed, but
/// a *learned* prior injected as β is the untried, higher-upside form.
fn corpus_prior() -> &'static Policy {
    use std::sync::OnceLock;
    static P: OnceLock<Policy> = OnceLock::new();
    P.get_or_init(build_corpus_prior)
}

fn build_corpus_prior() -> Policy {
    use crate::game::moves::legal_moves;
    let mut chosen: Policy = Policy::default();
    let mut avail: Policy = Policy::default();
    for rec in morpion_solitaire_records::RECORDS.iter() {
        let Ok(g) = crate::game::io::import_save(rec.2) else {
            continue;
        };
        if g.variant != crate::game::rules::Variant::T5 {
            continue; // keep the prior 5T-specific (the campaign's target variant)
        }
        let mut st = GameState::new(g.variant);
        for &mv in &g.history {
            let lm = legal_moves(&st);
            if !lm.contains(&mv) {
                break; // record diverges from our rules (shouldn't happen) — stop
            }
            for m in &lm {
                *avail.entry(local_code(&st.board, m.pos)).or_insert(0.0) += 1.0;
            }
            *chosen.entry(local_code(&st.board, mv.pos)).or_insert(0.0) += 1.0;
            st.apply(mv);
        }
    }
    let mut prior = Policy::default();
    for (&f, &a) in &avail {
        let c = chosen.get(&f).copied().unwrap_or(0.0);
        prior.insert(f, ((c + 1.0) / (a + 1.0)).ln());
    }
    prior
}

/// GNRPA prior bias β(move): a fixed log-space preference added to a move's
/// learned weight in the softmax (Cazenave's Generalized NRPA). β=0 reproduces
/// plain NRPA exactly. Two optional features combine here: the hand-set
/// neighbour-count term (`GNRPA_BETA`, historically unhelpful) and the
/// corpus-learned local prior (`NRPA_CORPUS`, see [`corpus_prior`]). Both keys are
/// symmetry-invariant, so the bias stays consistent with the policy coding.
#[inline]
fn beta(state: &GameState, pos: Pos) -> f64 {
    let mut bias = 0.0;
    let gb = gnrpa_beta();
    if gb != 0.0 {
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
        bias += gb * occ as f64;
    }
    let cb = nrpa_corpus();
    if cb != 0.0 {
        bias += cb
            * corpus_prior()
                .get(&local_code(&state.board, pos))
                .copied()
                .unwrap_or(0.0);
    }
    bias
}

// NRPA tuning knobs (env). NRPA_TEMP, NRPA_LOCAL and NRPA_PORTFOLIO are
// experimental and within NRPA's run-to-run variance at short budgets. NRPA_CLAMP
// (Stabilized-NRPA logit clamping) was a decisive win and is now ON BY DEFAULT at
// C=3 (see `nrpa_clamp`): clamping lifts the mean and cuts variance, and the gap
// over unclamped grows with budget/level (5T L4/120s ~112 vs ~95), so it scales.

/// Softmax temperature τ (`NRPA_TEMP`, default 1.0): weight ∝ exp(logit/τ). >1
/// flattens (more exploration), <1 sharpens. Read once.
fn nrpa_temp() -> f64 {
    use std::sync::OnceLock;
    static T: OnceLock<f64> = OnceLock::new();
    *T.get_or_init(|| {
        std::env::var("NRPA_TEMP")
            .ok()
            .and_then(|s| s.parse().ok())
            .filter(|&t| t > 0.0)
            .unwrap_or(1.0)
    })
}

/// Stabilization: clamp every adapted policy logit to ±C — a Stabilized-NRPA
/// flavour that curbs the premature convergence runaway weights cause. **On by
/// default at C = 3** (`--clamp <C>` overrides; `--clamp 0` disables). Set
/// from a sweep where clamping lifts the mean *and* cuts variance, and the gain
/// **grows** with budget/level rather than capping. The C sweet spot is tight:
/// 5T L4/120 s mean best — off 95, C=2 99, **C=3 112**, C=4 105, C=6 94 (C=3 beat
/// C=4 in every round of a 5-round head-to-head). Read once.
fn nrpa_clamp() -> Option<f64> {
    let o = CLAMP_OVR.load(Ordering::Relaxed);
    if o != F64_UNSET {
        let c = f64::from_bits(o);
        return if c > 0.0 { Some(c) } else { None };
    }
    Some(3.0) // unset ⇒ default on at C=3
}

/// Mix a symmetry-invariant local-neighbourhood feature into the move code
/// (`NRPA_LOCAL=1`, default off) so the policy generalizes across positions
/// sharing a local pattern — between position-specific and move-only coding.
fn nrpa_local() -> bool {
    use std::sync::OnceLock;
    static L: OnceLock<bool> = OnceLock::new();
    *L.get_or_init(|| {
        std::env::var("NRPA_LOCAL")
            .map(|v| v == "1")
            .unwrap_or(false)
    })
}

/// Symmetry-invariant move coding (default on). On: moves are coded in the board's
/// canonical D4 frame and all 8 Zobrist hashes are maintained. Off (`--no-symmetry`):
/// the identity frame only (one hash), skipping the 8-hash maintenance — ~+16%
/// throughput at neutral score, for cold record runs.
fn nrpa_sym() -> bool {
    // 1 ⇒ off, 2 ⇒ on, 0 (unset) ⇒ default on.
    SYM_OVR.load(Ordering::Relaxed) != 1
}

/// Island portfolio (`NRPA_PORTFOLIO=1`, default off): spread each island's
/// temperature over a range instead of all islands sharing one, for diversity.
fn nrpa_portfolio() -> bool {
    use std::sync::OnceLock;
    static P: OnceLock<bool> = OnceLock::new();
    *P.get_or_init(|| {
        std::env::var("NRPA_PORTFOLIO")
            .map(|v| v == "1")
            .unwrap_or(false)
    })
}

thread_local! {
    /// 1/τ for the current island's playouts and adapts (set in `island`).
    static TEMP_INV: std::cell::Cell<f64> = const { std::cell::Cell::new(1.0) };
    /// Per-island clamp override (set in `island` when the clamp portfolio is on);
    /// `None` ⇒ use the global [`nrpa_clamp`]. Lets diverse islands stabilize at
    /// different C, so the *global best* (what record hunting wants) benefits from
    /// whichever C suits a given run, not a single compromise C.
    static CLAMP_OVERRIDE: std::cell::Cell<Option<f64>> = const { std::cell::Cell::new(None) };
}

/// Clamp portfolio (`NRPA_CLAMP_PORTFOLIO=1`, default off): spread each island's
/// logit clamp C linearly over [2, 5] by island index instead of all sharing the
/// global C. Diversity aimed at the global max (the record-relevant metric).
fn nrpa_clamp_portfolio() -> bool {
    use std::sync::OnceLock;
    static P: OnceLock<bool> = OnceLock::new();
    *P.get_or_init(|| {
        std::env::var("NRPA_CLAMP_PORTFOLIO")
            .map(|v| v == "1")
            .unwrap_or(false)
    })
}

/// Self-improving warm restarts (`NRPA_SELFWARM=k`, default 0 = off): half the
/// islands restart with a policy pre-trained `k` adapt passes toward the *current
/// global best game* instead of a blank policy, so progress compounds (the best
/// basin is re-explored from a biased start) while the other half stay blank for
/// diversity. Only active on the plain cold run (no external seed). Read once.
fn nrpa_selfwarm() -> usize {
    use std::sync::OnceLock;
    static V: OnceLock<usize> = OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("NRPA_SELFWARM")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    })
}

/// This island's clamp C under the portfolio: linear over [2, 5] across islands.
fn island_clamp(i: usize, runs: usize) -> Option<f64> {
    if !nrpa_clamp_portfolio() || runs <= 1 {
        return None; // fall back to the global clamp
    }
    let frac = i as f64 / (runs - 1) as f64;
    Some(2.0 + 3.0 * frac)
}

/// Temperature inverse for island `i` of `runs`: the global τ, or a spread over
/// [0.6, 1.6]·τ across islands when the portfolio is on.
fn island_inv_temp(i: usize, runs: usize) -> f64 {
    let tau = nrpa_temp();
    if !nrpa_portfolio() || runs <= 1 {
        return 1.0 / tau;
    }
    let frac = i as f64 / (runs - 1) as f64;
    1.0 / (tau * (0.6 + frac))
}

/// A move's policy code: the symmetry code, optionally XORed with a local-context
/// term (see [`nrpa_local`]).
#[inline]
fn move_code(coder: &MoveCoder, scratch: &GameState, mv: &Move, local: bool) -> u64 {
    let c = coder.code(mv);
    if local {
        c ^ local_code(&scratch.board, mv.pos)
    } else {
        c
    }
}

/// Iterations per nesting level — NRPA uses the same N at every level, so we set
/// it once from the top level. Measured sweet spot for level 3 on a ~minute
/// budget is ~500 (N=100 converges low ~91, N=1000 stalls the level-3 outer loop
/// ~93). For deeper levels we shrink N to hold each *outer* iteration's cost
/// roughly constant (N^(level-1) ≈ 500² ≈ 250k playouts), so the outer loop keeps
/// iterating instead of stalling inside one sub-search. Deeper levels learn more
/// cumulatively but only pay off over very long (multi-hour) runs.
fn iterations_for_level(level: usize) -> usize {
    // NRPA uses one N at every level, set once from the top level; `NRPA_ITERS`
    // overrides it directly so N can be swept per level under clamp (the defaults
    // were tuned UNCLAMPED, so the sweet spots likely shifted). Read once.
    use std::sync::OnceLock;
    static OVERRIDE: OnceLock<Option<usize>> = OnceLock::new();
    if let Some(n) = *OVERRIDE.get_or_init(|| {
        std::env::var("NRPA_ITERS")
            .ok()
            .and_then(|s| s.parse().ok())
            .filter(|&n| n > 0)
    }) {
        return n;
    }
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

/// Perturbation destroy size K is drawn uniformly from `[K_MIN, K_MAX]` each
/// round — a mix of small local refinements and large restructures, capped to the
/// current game length. The destroy-size distribution is the biggest perturbation
/// lever, so both bounds are tunable via `--kmin` / `--kmax`.
#[cfg(not(target_arch = "wasm32"))]
fn perturb_k_min() -> usize {
    let o = KMIN_OVR.load(Ordering::Relaxed);
    if o >= 1 {
        o
    } else {
        8
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn perturb_k_max() -> usize {
    let o = KMAX_OVR.load(Ordering::Relaxed);
    if o >= 1 {
        o
    } else {
        70
    }
}

/// Effort (round duration) scales with K: a tiny completion needs only a couple
/// of seconds, a 70-move reconstruction much more. `secs = K · per_k`, clamped.
#[cfg(not(target_arch = "wasm32"))]
const PERTURB_SECS_PER_K: f64 = 0.5;
#[cfg(not(target_arch = "wasm32"))]
const PERTURB_MIN_SECS: f64 = 2.0;
#[cfg(not(target_arch = "wasm32"))]
const PERTURB_MAX_SECS: f64 = 30.0;

#[cfg(not(target_arch = "wasm32"))]
thread_local! {
    /// The crossover rate for this thread's perturbation loop (`--crossover`).
    /// `-1` ⇒ unset (default off); `≥0` is the configured rate.
    static XOVER_OVERRIDE: std::cell::Cell<f64> = const { std::cell::Cell::new(-1.0) };
}

/// Set the crossover rate (`-1` to clear ⇒ default off). The perturbation loop runs
/// on the calling thread, so call this at the top of that thread (the CLI does, to
/// apply the configured `--crossover` rate per-run; tests use it too).
#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)] // used by the CLI and the crossover test
pub fn set_crossover_override(rate: f64) {
    XOVER_OVERRIDE.with(|c| c.set(rate));
}

/// Probability that a perturbation round does a **crossover** of two archived games
/// instead of destroy/repair of one (`--crossover`, default 0 = off). The new
/// recombination lever: it can reach combinations a single-game destroy/repair can't.
#[cfg(not(target_arch = "wasm32"))]
fn perturb_crossover() -> f64 {
    let o = XOVER_OVERRIDE.with(|c| c.get());
    if o >= 0.0 {
        o
    } else {
        0.0
    }
}

/// Is `m` playable in `st` right now: its placed point empty, the line's four other
/// cells occupied, and the line not conflicting with a drawn one.
#[cfg(not(target_arch = "wasm32"))]
fn move_playable(st: &GameState, m: &Move, line_len: u8) -> bool {
    use crate::game::rules::TouchMode;
    if st.board.contains(m.pos) {
        return false;
    }
    for (k, cell) in m.line.positions(line_len).enumerate() {
        if k as u8 != m.line_pos && !st.board.contains(cell) {
            return false;
        }
    }
    let max_overlap = match st.variant.touch_mode {
        TouchMode::Touching => 1,
        TouchMode::Disjoint => 0,
    };
    !st.line_index.conflicts(&m.line, line_len - 1 - max_overlap)
}

/// Genetic-style crossover of two valid games: pool their move sets and salvage a
/// valid game by replaying the pool in a random valid order, dropping any pooled
/// move not playable (line incomplete or conflicting) at its turn. The result
/// reuses compatible substructure from both parents — exploration a single-game
/// destroy/repair can't reach — and is a valid game by construction. Perturbation
/// then repairs/extends it.
#[cfg(not(target_arch = "wasm32"))]
fn crossover_games<R: rand::Rng>(
    g1: &[Move],
    g2: &[Move],
    variant: crate::game::rules::Variant,
    rng: &mut R,
) -> Vec<Move> {
    use rand::seq::SliceRandom;
    use std::collections::HashSet;
    let line_len = variant.len();
    let mut seen = HashSet::new();
    let mut pool: Vec<Move> = Vec::new();
    for &m in g1.iter().chain(g2) {
        if seen.insert((m.pos, m.line.origin, m.line.dir as u8, m.line_pos)) {
            pool.push(m);
        }
    }
    pool.shuffle(rng);
    let mut st = GameState::new(variant);
    let mut progress = true;
    while progress {
        progress = false;
        let mut i = 0;
        while i < pool.len() {
            if move_playable(&st, &pool[i], line_len) {
                let m = pool.swap_remove(i);
                if !st.apply(m) {
                    return st.history; // grid overflow — keep what we have
                }
                progress = true;
            } else {
                i += 1;
            }
        }
    }
    st.history
}

/// Archive (population) memory: instead of a single annealing point, keep a
/// diverse pool of high games and perturb a random one each round. The archive
/// keeps every distinct game whose score is within `WINDOW` of the best seen
/// (so it holds a band of near-best games — diversity + escape room), capped at
/// `ARCHIVE_MAX`. A tabu set of game keys avoids re-processing duplicates.
/// Warm-start strength for a perturbation **repair** (`PERTURB_WARM`, default
/// `WARM_ITERS`=10): adapt passes pre-training the repair policy toward the
/// destroyed suffix. High values bias the repair to *replay* the old ending
/// (good for exploiting a known-strong seed, bad for genuinely improving it); 0
/// makes the repair a cold re-search of new completions. The key knob for the
/// "perturbation just rebuilds the seed" trivial-replay trap.
#[cfg(not(target_arch = "wasm32"))]
fn perturb_warm() -> usize {
    use std::sync::OnceLock;
    static V: OnceLock<usize> = OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("PERTURB_WARM")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(WARM_ITERS)
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn perturb_window() -> usize {
    let o = WINDOW_OVR.load(Ordering::Relaxed);
    if o >= 1 {
        o
    } else {
        10
    }
}
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
        let m = remaining.swap_remove(playable[rng.random_range(0..playable.len())]);
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

    let mut rng = rand::rng();
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
        } else if archive.len() >= 2 && rng.random::<f64>() < perturb_crossover() {
            // Crossover: recombine two archived games into a valid game, then reorder
            // for a varied destroy region. A long recombination is a candidate in its
            // own right (the repair may not reproduce it if much is destroyed), so
            // record it before perturbing.
            let a = &archive[rng.random_range(0..archive.len())];
            let b = &archive[rng.random_range(0..archive.len())];
            let merged = crossover_games(a, b, variant, &mut rng);
            if merged.len() > max_score {
                max_score = merged.len();
                search.record_best(merged.len() as u32, merged.clone());
            }
            random_extension(&merged, &cross, line_len, &mut rng)
        } else {
            let g = archive[rng.random_range(0..archive.len())].clone();
            random_extension(&g, &cross, line_len, &mut rng)
        };
        let k_max = perturb_k_max()
            .min(parent.len().saturating_sub(1))
            .max(perturb_k_min());
        let k = rng.random_range(perturb_k_min()..=k_max);
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
        let warm = perturb_warm();
        let h = std::thread::spawn(move || run_warm(&p, inner2, level, &suffix, warm));
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
            if score + perturb_window() >= max_score {
                archive.push(cand);
                let cutoff = max_score.saturating_sub(perturb_window());
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
    let base_sym = &base_sym; // capture a (Copy) reference in the move closures
    let runs = rayon::current_num_threads().max(1);
    rayon::scope(|s| {
        for i in 0..runs {
            s.spawn(move |_| island(i, runs, level, n, initial_state, base_sym, search, seed));
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
#[allow(clippy::too_many_arguments)]
fn island(
    idx: usize,
    runs: usize,
    level: usize,
    n: usize,
    initial_state: &GameState,
    base_sym: &SymmetryHashes,
    search: &Arc<SearchState>,
    seed: Option<&Policy>,
) {
    // This island's softmax temperature and clamp (portfolios spread them by idx).
    TEMP_INV.with(|c| c.set(island_inv_temp(idx, runs)));
    CLAMP_OVERRIDE.with(|c| c.set(island_clamp(idx, runs)));
    // One clone for the island's lifetime. `playout`/`adapt` apply moves onto
    // this scratch and undo them, always restoring it to `initial_state` — so
    // there is no per-playout clone of the ~14 KB game state.
    let mut scratch = initial_state.clone();
    let selfwarm = nrpa_selfwarm();
    while search.running.load(Ordering::Relaxed) {
        // Fresh policy each restart (diversity), or a clone of the warm-start
        // seed when one is supplied. With self-warm on, half the islands instead
        // pre-train toward the current global best (compounding) while the rest
        // stay blank for diversity.
        let mut policy = if seed.is_none() && selfwarm > 0 && idx % 2 == 1 {
            let best = search.best_sequence.read().unwrap().clone();
            if best.len() >= 60 {
                build_warm_policy(initial_state, &best, selfwarm)
            } else {
                Policy::default()
            }
        } else {
            seed.cloned().unwrap_or_default()
        };
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
    let mut rng = rand::rng();
    let prior = prior_active();
    let inv_temp = TEMP_INV.with(|c| c.get());
    let local = nrpa_local();
    let sym_on = nrpa_sym();
    // Buffers reused across every step of this playout (cleared per step) so the
    // hottest loop in the search does no per-node heap allocation.
    let mut moves: Vec<Move> = Vec::new();
    let mut moves_next: Vec<Move> = Vec::new();
    let mut weights: Vec<f64> = Vec::new();

    // Incremental legal-move maintenance: one full scan at entry, then each step
    // derives the next set from the played move instead of rescanning the board.
    legal_moves_into(scratch, &mut moves);
    loop {
        search.nodes_explored.fetch_add(1, Ordering::Relaxed);
        if moves.is_empty() {
            break;
        }

        // Softmax sampling (symmetry-invariant policy code per move). Build the
        // position coder once so canonical() isn't recomputed for every move.
        // Unseen codes have weight exp(0)=1 exactly (when β is off), so we skip
        // the transcendental for them — the common case while the policy is sparse.
        let coder = sym.move_coder();
        weights.clear();
        if !prior {
            // Cold fast path (no GNRPA/corpus bias): weight is exp(w/τ) for a seen
            // code, exp(0)=1 for an unseen one — no `beta` per move.
            weights.extend(moves.iter().map(|mv| {
                match policy.get(&move_code(&coder, scratch, mv, local)) {
                    Some(&w) => (w * inv_temp).exp(),
                    None => 1.0,
                }
            }));
        } else {
            weights.extend(moves.iter().map(|mv| {
                match policy.get(&move_code(&coder, scratch, mv, local)) {
                    Some(&w) => ((w + beta(scratch, mv.pos)) * inv_temp).exp(),
                    None => (beta(scratch, mv.pos) * inv_temp).exp(),
                }
            }));
        }
        let total: f64 = weights.iter().sum();
        let mut r = rng.random::<f64>() * total;
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
        let mv = moves[chosen];
        if !scratch.apply(mv) {
            break;
        }
        if sym_on {
            sym.toggle(mv.pos);
        } else {
            sym.toggle_identity(mv.pos);
        }
        child_legal_moves_into(&mut moves_next, &moves, scratch, mv);
        std::mem::swap(&mut moves, &mut moves_next);
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
    let prior = prior_active();
    let inv_temp = TEMP_INV.with(|c| c.get());
    let local = nrpa_local();
    let sym_on = nrpa_sym();
    // Per-island portfolio clamp if set, else the global default.
    let clamp = match CLAMP_OVERRIDE.with(|c| c.get()) {
        Some(c) => Some(c),
        None => nrpa_clamp(),
    };
    let alpha = nrpa_alpha();
    // Reused per step (cleared each iteration). `codes` lets us hash each move's
    // symmetry code once and reuse it for both the softmax and the update.
    let mut moves: Vec<Move> = Vec::new();
    let mut moves_next: Vec<Move> = Vec::new();
    let mut codes: Vec<u64> = Vec::new();
    let mut exps: Vec<f64> = Vec::new();

    // Incremental legal set (one scan, then derived per step) — see `playout`.
    legal_moves_into(scratch, &mut moves);
    for &mv in best_seq {
        if moves.is_empty() {
            break;
        }
        // Softmax over the legal moves, read BEFORE this step's update (each step
        // touches distinct codes, so the running policy is clean). One coder per
        // position; code each move once. Unseen codes contribute exp(0)=1 (β off).
        let coder = sym.move_coder();
        codes.clear();
        codes.extend(moves.iter().map(|m| move_code(&coder, scratch, m, local)));
        exps.clear();
        exps.extend(
            codes
                .iter()
                .zip(&moves)
                .map(|(code, m)| match policy.get(code) {
                    Some(&w) => ((w + beta(scratch, m.pos)) * inv_temp).exp(),
                    None if !prior => 1.0,
                    None => (beta(scratch, m.pos) * inv_temp).exp(),
                }),
        );
        let z: f64 = exps.iter().sum();

        // chosen += α ; each legal -= α · P(move). Optionally clamp the touched
        // logits to ±C (stabilization). The chosen code is among `codes`, so the
        // loop also clamps it after its own decrement.
        *policy
            .entry(move_code(&coder, scratch, &mv, local))
            .or_insert(0.0) += alpha;
        for (&code, &e_m) in codes.iter().zip(&exps) {
            let e = policy.entry(code).or_insert(0.0);
            *e -= alpha * (e_m / z);
            if let Some(c) = clamp {
                *e = e.clamp(-c, c);
            }
        }

        if !scratch.apply(mv) {
            break;
        }
        if sym_on {
            sym.toggle(mv.pos);
        } else {
            sym.toggle_identity(mv.pos);
        }
        child_legal_moves_into(&mut moves_next, &moves, scratch, mv);
        std::mem::swap(&mut moves, &mut moves_next);
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
    use crate::game::moves::legal_moves;
    use crate::game::{rules::Variant, state::GameState};
    use std::time::Duration;

    /// Crossover of two valid games is itself a valid game: every move replays
    /// legally from the cross. (Validity by construction; this guards it.)
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn crossover_yields_a_valid_game() {
        use crate::game::io::import_save;
        let recs: Vec<Vec<Move>> = morpion_solitaire_records::RECORDS
            .iter()
            .filter_map(|r| import_save(r.2).ok())
            .filter(|g| g.variant == Variant::T5)
            .map(|g| g.history)
            .take(2)
            .collect();
        assert_eq!(recs.len(), 2, "need two 5T records");
        let mut rng = rand::rng();
        let g = crossover_games(&recs[0], &recs[1], Variant::T5, &mut rng);
        assert!(!g.is_empty());
        let mut st = GameState::new(Variant::T5);
        for &mv in &g {
            assert!(legal_moves(&st).contains(&mv), "crossover move illegal at replay");
            assert!(st.apply(mv));
        }
        assert_eq!(st.history.len(), g.len());
    }

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

    /// Averaged perturbation-from-seed benchmark (the real large-neighbourhood
    /// path). Seeds the archive from rosin178 truncated to `SEED_LEN` (so there is
    /// real room to climb, below the 178 ceiling), runs the actual archive-based
    /// `run_perturbation` for `NRPA_SECS`, and reports the best-score distribution
    /// over `NRPA_RUNS`. Sweep the destroy-size distribution with `--kmin` /
    /// `--kmax` / `--window`. Env: NRPA_LEVEL (repair level),
    /// NRPA_SECS, NRPA_RUNS, SEED_LEN.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "measurement, run with --ignored --nocapture"]
    fn measure_perturbation_run() {
        use crate::game::io::import_save;
        let env = |k: &str, d: u64| {
            std::env::var(k)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(d)
        };
        let level = env("NRPA_LEVEL", 2) as usize;
        let secs = env("NRPA_SECS", 60);
        let runs = env("NRPA_RUNS", 3) as usize;
        let seed_len = env("SEED_LEN", 0) as usize; // 0 = full game (no truncation)
        let seed_name = std::env::var("SEED_RECORD").unwrap_or_else(|_| "akiyama145".to_string());

        let rec = import_save(
            morpion_solitaire_records::RECORDS
                .iter()
                .find(|(n, id, _)| *n == seed_name.as_str() || *id == seed_name.as_str())
                .unwrap_or_else(|| panic!("record {seed_name:?} not found"))
                .2,
        )
        .unwrap();
        let variant = rec.variant;
        let mut seed = rec.history.clone();
        if seed_len > 0 && seed_len < seed.len() {
            seed.truncate(seed_len);
        }
        println!(
            "seed={seed_name} len={} (used {})",
            rec.history.len(),
            seed.len()
        );

        let mut bests: Vec<u32> = Vec::with_capacity(runs);
        for r in 0..runs {
            let search = SearchState::new();
            let s2 = search.clone();
            let seed2 = seed.clone();
            let h = std::thread::spawn(move || run_perturbation(s2, level, seed2, variant));
            std::thread::sleep(Duration::from_secs(secs));
            search.running.store(false, Ordering::Relaxed);
            h.join().unwrap();
            let best = search.best_score.load(Ordering::Relaxed);
            bests.push(best);
            println!("  run {r}: best={best}");
        }
        let n = bests.len() as f64;
        let mean = bests.iter().map(|&b| b as f64).sum::<f64>() / n;
        println!(
            "PERTURB L{level} seed={seed_len} kmin={} kmax={} window={} {secs}s ×{runs} : mean={mean:.1} min={} max={}",
            perturb_k_min(),
            perturb_k_max(),
            perturb_window(),
            bests.iter().min().unwrap(),
            bests.iter().max().unwrap(),
        );
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
