/// Systematic backtracking search — exhaustive and **stateless** (no
/// transposition table, so memory is bounded by the DFS stack and a search can
/// run indefinitely). Each position is reached exactly once thanks to two exact,
/// memory-free layers:
///
/// 1. **Trace normal form** (`canonical_ok`): a move is explored only if it
///    could not have been played earlier — eliminates *all* move-order
///    transpositions (not just adjacent ones).
/// 2. **Structural symmetry**: the initial position is fixed by some subgroup of
///    D4; at each node we explore only one representative per orbit of the
///    *current* stabiliser, which shrinks to identity after the first generic
///    move. So symmetric positions are never both explored.
///
/// (These suffice because the move-set is a function of the position: two move
/// orders reaching the same position are the same trace, and a position can't be
/// reached by two different move-sets — the line→point placement can't cycle.)
///
/// Top-level first moves are explored in parallel via Rayon.
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

#[cfg(not(target_arch = "wasm32"))]
use super::checkpoint;
use super::symmetry::{apply_transform, is_orbit_min, stab_after, transform_line};
use super::SearchState;
#[cfg(not(target_arch = "wasm32"))]
use crate::game::io;
use crate::game::{
    board::{Pos, OFFSET},
    line::{Dir, Line},
    line_index::lines_conflict,
    moves::{legal_moves, legal_moves_into, Move},
    rules::TouchMode,
    state::GameState,
};

/// Flush the thread-local node counter into the shared atomic every this many
/// nodes. Batching avoids a `fetch_add` (and its cache-line contention across
/// worker threads) on every single node, while staying responsive enough for
/// the live nodes/s display.
const NODE_FLUSH_INTERVAL: u64 = 4096;

/// Identity-only stabiliser bitmask (no symmetry left).
const NO_SYMMETRY: u8 = 0b0000_0001;

/// Nodes a worker explores from one frontier item before stopping and pushing
/// its remaining frontier back to the shared stack. Bounds the frontier size
/// (keeps it DFS-like) and gives load-balancing / checkpoint granularity.
///
/// **Why 500 000 and not smaller?**  At the top of the 5T canonical tree the
/// branching factor is very high (≈ 20 at depth 1, ≈ 7 at depth 4), so a small
/// budget runs out before the DFS can descend past the high-branching region.
/// The resulting frontier items are all shallow; the LIFO stack then keeps
/// workers busy processing exponentially many shallow items and the search never
/// consistently reaches terminal depth (≈ 72–90 moves).  A budget of 500 k is
/// large enough to push past that region in a single chunk, placing items at
/// depth ≥ 30 where b ≈ 2 and further chunks converge on terminals quickly.
/// This matters especially on macOS / Apple Silicon where the scheduler does not
/// produce the "lucky" exploration order that was observed on x86 Linux with the
/// old 20 000 budget.
const CHUNK_BUDGET: u64 = 500_000;

/// A unit of work: the move sequence (from the initial position) to an
/// unexplored subtree root. Self-contained and serialisable — this is what the
/// frontier holds and what a checkpoint will save.
type WorkItem = Vec<Move>;

/// A legal move at a node together with its **trace-canonical** flag (whether it
/// is in trace normal form here — see [`canonical_ok`]). The flag is derived
/// incrementally from the parent's, so the O(depth) history scan of
/// `canonical_ok` runs only at chunk roots, never in the hot loop.
#[derive(Clone, Copy)]
struct Cand {
    mv: Move,
    canon: bool,
}

/// One level of an explicit (non-recursive) DFS: the canonical children of a
/// node and how many we've already descended into, plus that node's stabiliser.
struct Frame {
    /// Canonical children (descended into); a filtered subset of `legal`.
    children: Vec<Move>,
    idx: usize,
    stab: u8,
    /// Raw legal-move set at this node (each with its trace-canonical flag),
    /// carried so each child's set is derived incrementally (parent set ± a local
    /// delta) instead of rescanning the whole board — `legal_moves` is ~⅔ of a
    /// node's cost — and so each move's canonicity is an O(1) update of the
    /// parent's flag instead of an O(depth) history scan.
    legal: Vec<Cand>,
}

/// `forbid` parameter of the touch rule for `variant` (see `LineIndex::conflicts`).
fn forbid_of(variant: crate::game::rules::Variant) -> u8 {
    let max_overlap = match variant.touch_mode {
        TouchMode::Touching => 1,
        TouchMode::Disjoint => 0,
    };
    variant.len() - 1 - max_overlap
}

/// Canonical children to descend into, from a node's legal set: keep the
/// trace-canonical moves (precomputed `canon` flag) and, while symmetry remains,
/// only the orbit representatives.
fn canonical_children_into(out: &mut Vec<Move>, legal: &[Cand], stab: u8, k: i16, n: i16) {
    out.clear();
    let symmetric = stab != NO_SYMMETRY;
    out.extend(
        legal
            .iter()
            .filter(|c| c.canon && (!symmetric || is_orbit_min(&c.mv, stab, k, n)))
            .map(|c| c.mv),
    );
}

/// Trace-canonical flags for a freshly-scanned legal set (chunk-root only): runs
/// the O(depth) `canonical_ok` history scan once per move, then the search keeps
/// the flags up to date incrementally (see [`child_legal_into`]).
fn root_candidates(out: &mut Vec<Cand>, legal: &[Move], state: &GameState) {
    out.clear();
    out.extend(legal.iter().map(|&mv| Cand {
        mv,
        canon: canonical_ok(&mv, state),
    }));
}

/// Take a cleared buffer from the pool, or a fresh one if the pool is empty.
#[inline]
fn take_buf<T>(pool: &mut Vec<Vec<T>>) -> Vec<T> {
    pool.pop().unwrap_or_default()
}

/// Raw legal set of the child reached by playing `mv`, derived from the parent's
/// set (`state` already has `mv` applied). Playing `mv` adds exactly one point
/// (`mv.pos`) and one line (`mv.line`), so the set changes only locally:
///   • a parent move dies if its empty cell is `mv.pos`, or its line now
///     conflicts with `mv.line` (it conflicted with nothing before, so a plain
///     `conflicts` test against the post-move index isolates `mv.line`);
///   • new moves can only appear in windows through `mv.pos` (the sole cell whose
///     occupancy changed).
/// Legal set (with canonical flags) of the child reached by playing `mv`,
/// derived from the parent's set (cleared into `out`; `state` already has `mv`).
///
/// Each carried move's `canon` flag is updated in O(1): a carried move `m` cannot
/// have `mv.pos` on its line (else `m` would have had two empty cells at the
/// parent and not been legal), so prepending `mv` to the history only flips `m`
/// to non-canonical when `mv.pos > m.pos` (a larger move now commutes ahead of
/// it); otherwise the flag is unchanged. Newly-appearing moves all pass through
/// `mv.pos`, so `mv` is their most-recent dependency — they are always canonical.
fn child_legal_into(
    out: &mut Vec<Cand>,
    parent_legal: &[Cand],
    state: &GameState,
    mv: Move,
    forbid: u8,
    nu: usize,
) {
    out.clear();
    for &c in parent_legal {
        // A carried move was legal at the parent, so it conflicts with nothing
        // drawn before `mv`; it survives unless it collides with `mv.pos` or with
        // `mv.line` itself — a direct two-line test, no index lookup (and an
        // instant reject when directions differ, the common case).
        if c.mv.pos != mv.pos && !lines_conflict(&c.mv.line, &mv.line, forbid) {
            let canon = c.canon && mv.pos <= c.mv.pos;
            out.push(Cand { mv: c.mv, canon });
        }
    }
    add_windows_through(out, state, mv.pos, forbid, nu);
}

/// Append every newly-legal move whose line passes through `p`: the `nu` windows
/// per direction that contain `p`, keeping those with exactly one empty cell and
/// no line conflict. Each such window has `p` occupied, so it is emitted once.
///
/// For each direction we read the `2·nu−1`-cell occupancy *strip* centred on `p`
/// once (bit `nu−1` = `p` itself), then each window is a contiguous `nu`-bit
/// field of that strip: "exactly one empty" is a single-zero-bit test, sharing
/// the strip across all `nu` overlapping windows instead of re-reading cells.
/// `p` is an interior cell (the board margin = `nu−1`), so the strip never leaves
/// the grid. `n ≤ 5` ⇒ the strip fits a `u16`.
fn add_windows_through(out: &mut Vec<Cand>, state: &GameState, p: Pos, forbid: u8, nu: usize) {
    let span = nu as i16 - 1; // p is at strip bit `span`
    let full: u16 = (1 << nu) - 1; // a window's nu-bit mask at offset 0
    let strip_mask: u16 = (1 << (2 * nu - 1)) - 1;
    for dir in Dir::ALL {
        let (dx, dy) = dir.delta();
        let mut strip: u16 = 0;
        if dy == 0 {
            // Horizontal: the whole strip lies in one board row — read it in a
            // single shift+mask instead of `2n−1` per-cell `contains` calls.
            let gy = (p.1 + OFFSET) as usize;
            let gx0 = (p.0 + OFFSET - span) as u32; // leftmost strip column
            strip = (state.board.row(gy) >> gx0) as u16 & strip_mask;
        } else {
            for b in 0..(2 * nu - 1) {
                let t = b as i16 - span;
                if state.board.contains((p.0 + t * dx, p.1 + t * dy)) {
                    strip |= 1 << b;
                }
            }
        }
        // Window `j` (p at position j within it) occupies strip bits
        // [nu−1−j .. nu−1−j+nu−1]; its origin is `p − j·dir`.
        for j in 0..nu {
            let lo = (nu - 1 - j) as u32;
            let zeros = !strip & (full << lo); // empty cells of this window
            if zeros == 0 || zeros & (zeros - 1) != 0 {
                continue; // not exactly one empty cell
            }
            let empty_idx = (zeros.trailing_zeros() - lo) as u8; // position in window
            let origin = (p.0 - j as i16 * dx, p.1 - j as i16 * dy);
            let line = Line::new(origin, dir);
            if state.line_index.conflicts(&line, forbid) {
                continue;
            }
            let new_pos = (
                origin.0 + empty_idx as i16 * dx,
                origin.1 + empty_idx as i16 * dy,
            );
            // `p` (= mv.pos) is an occupied cell of this window, so the move just
            // played is this new move's most-recent dependency ⇒ trace-canonical.
            out.push(Cand {
                mv: Move::new(new_pos, line, empty_idx),
                canon: true,
            });
        }
    }
}

/// Launch the systematic search.  Call from a background `std::thread`.
///
/// Frontier model: a shared LIFO stack of `WorkItem`s (partial games) is
/// drained by a pool of workers. Each worker reconstructs a subtree root,
/// explores up to `CHUNK_BUDGET` nodes depth-first, then pushes whatever
/// frontier it didn't finish back onto the stack. The stack is the entire,
/// serialisable state of the search — the basis for exact checkpoint/resume.
pub fn run(initial_state: &GameState, search: Arc<SearchState>) {
    search.reset();
    let n = initial_state.variant.len() as i16;
    let k = 2 * n - 1;
    let init_stab = initial_stabilizer(initial_state, k, n);

    // Seed the frontier with the canonical first moves (one orbit rep each).
    let firsts: Vec<Move> = {
        let moves = legal_moves(initial_state);
        if init_stab == NO_SYMMETRY {
            moves
        } else {
            moves
                .into_iter()
                .filter(|m| is_orbit_min(m, init_stab, k, n))
                .collect()
        }
    };
    let frontier: Vec<WorkItem> = firsts.into_iter().map(|m| vec![m]).collect();
    drive_workers(initial_state, init_stab, frontier, &search, k, n);
}

/// Resume a systematic search from a checkpoint: restore best/records/nodes and
/// seed the frontier from the saved unexplored work, then continue exactly.
#[cfg(not(target_arch = "wasm32"))]
pub fn resume(search: Arc<SearchState>, checkpoint: io::Checkpoint) {
    search.reset();
    // Restore progress.
    search
        .best_score
        .store(checkpoint.best.len() as u32, Ordering::Relaxed);
    *search.best_sequence.write().unwrap() = checkpoint.best;
    *search.records.write().unwrap() = checkpoint.records;
    search
        .nodes_explored
        .store(checkpoint.nodes_explored, Ordering::Relaxed);

    let initial_state = GameState::new(checkpoint.variant);
    let n = initial_state.variant.len() as i16;
    let k = 2 * n - 1;
    let init_stab = initial_stabilizer(&initial_state, k, n);
    drive_workers(
        &initial_state,
        init_stab,
        checkpoint.frontier,
        &search,
        k,
        n,
    );
}

/// Upper bound on systematic workers: the number of **physical** cores. On Intel
/// hybrid CPUs (P + E cores) and any SMT/HyperThreading host, the per-node work
/// is compute-bound, so HT siblings add almost nothing (measured ≈ +12% for
/// doubling the P-core threads) and oversubscribing them is neutral-to-slightly
/// negative. Capping to physical cores matches the measured throughput peak with
/// fewer threads (less heat, logical threads free for the UI). The pool is only
/// resized *down* — an explicit smaller `--threads` is still honoured.
#[cfg(not(target_arch = "wasm32"))]
fn worker_cap() -> usize {
    num_cpus::get_physical().max(1)
}

/// On wasm the worker count is governed by `wasm-bindgen-rayon`; no cap.
#[cfg(target_arch = "wasm32")]
fn worker_cap() -> usize {
    usize::MAX
}

/// Drive the worker pool over a (possibly resumed) frontier until it drains or
/// the search stops. If the ambient Rayon pool is larger than the physical-core
/// count (a hybrid / SMT host running the default one-worker-per-logical-CPU
/// pool), run the workers in a dedicated pool capped to physical cores; otherwise
/// use the ambient pool as-is (honours an explicit, smaller `--threads`).
fn drive_workers(
    initial_state: &GameState,
    init_stab: u8,
    seed: Vec<WorkItem>,
    search: &Arc<SearchState>,
    k: i16,
    n: i16,
) {
    let cap = worker_cap();
    if cap < rayon::current_num_threads().max(1) {
        if let Ok(pool) = rayon::ThreadPoolBuilder::new().num_threads(cap).build() {
            pool.install(|| run_workers(initial_state, init_stab, seed, search, k, n));
            return;
        }
    }
    run_workers(initial_state, init_stab, seed, search, k, n);
}

/// The worker-driving loop proper; runs in whatever Rayon pool it is called in
/// (so `current_num_threads` reflects the capped pool when one is installed).
fn run_workers(
    initial_state: &GameState,
    init_stab: u8,
    seed: Vec<WorkItem>,
    search: &Arc<SearchState>,
    k: i16,
    n: i16,
) {
    search.running.store(true, Ordering::Relaxed);
    let frontier: Mutex<Vec<WorkItem>> = Mutex::new(seed);
    let active = AtomicUsize::new(0);
    let workers = rayon::current_num_threads().max(1);

    let mut exhausted = false;
    while search.running.load(Ordering::Relaxed) {
        if frontier.lock().unwrap().is_empty() {
            exhausted = true; // whole tree drained on its own — best is optimal
            break;
        }
        search.checkpoint_requested.store(false, Ordering::Relaxed);
        rayon::scope(|s| {
            for _ in 0..workers {
                s.spawn(|_| worker(&frontier, &active, initial_state, init_stab, search, k, n));
            }
        });
        if search.checkpoint_requested.load(Ordering::Relaxed) {
            let snapshot = frontier.lock().unwrap().clone();
            save_checkpoint(initial_state, search, &snapshot);
        }
    }

    search.exhausted.store(exhausted, Ordering::Relaxed);
    search.running.store(false, Ordering::Relaxed);
    search.checkpoint_requested.store(false, Ordering::Relaxed);
}

/// Serialise a checkpoint (frontier + best + records) to disk, logging the save
/// time. Workers are stopped when this runs, so the frontier is stable.
#[cfg(not(target_arch = "wasm32"))]
fn save_checkpoint(initial: &GameState, search: &SearchState, frontier: &[WorkItem]) {
    use std::time::Instant;
    let t0 = Instant::now();
    let best = search.best_sequence.read().unwrap().clone();
    let records = search.records.read().unwrap().clone();
    let nodes = search.nodes_explored.load(Ordering::Relaxed);
    let serialized = match io::export_checkpoint(
        initial.variant,
        nodes,
        &best,
        &records,
        frontier,
        "systematic",
        io::unix_now(),
    ) {
        Ok(s) => s,
        Err(e) => {
            log::error!("checkpoint serialise failed: {e}");
            return;
        }
    };
    if let Err(e) = checkpoint::write("systematic", &serialized) {
        log::error!("checkpoint write failed: {e}");
        return;
    }
    log::info!(
        "checkpoint saved: {} frontier items, {} bytes, {:.0} ms",
        frontier.len(),
        serialized.len(),
        t0.elapsed().as_secs_f64() * 1e3,
    );
}

/// On the web there is no filesystem; checkpointing is a no-op for now.
#[cfg(target_arch = "wasm32")]
fn save_checkpoint(_initial: &GameState, _search: &SearchState, _frontier: &[WorkItem]) {}

/// Pull items from the frontier and explore them until the search stops or the
/// frontier is drained (and no worker is still producing).
fn worker(
    frontier: &Mutex<Vec<WorkItem>>,
    active: &AtomicUsize,
    initial: &GameState,
    init_stab: u8,
    search: &Arc<SearchState>,
    k: i16,
    n: i16,
) {
    let mut local = 0u64;
    loop {
        search.wait_if_paused(); // idle here between chunks while paused
                                 // Stop on shutdown or a checkpoint request (we finish the current chunk
                                 // first — `explore_item` pushes its leftover frontier on budget exit —
                                 // so the frontier is consistent when all workers have returned).
        if !search.running.load(Ordering::Relaxed)
            || search.checkpoint_requested.load(Ordering::Relaxed)
        {
            break;
        }
        // Pop an item and mark ourselves active under the same lock.
        let item = {
            let mut f = frontier.lock().unwrap();
            let it = f.pop();
            if it.is_some() {
                active.fetch_add(1, Ordering::Relaxed);
            }
            it
        };
        match item {
            Some(path) => {
                explore_item(path, initial, init_stab, search, frontier, &mut local, k, n);
                active.fetch_sub(1, Ordering::Release);
            }
            None => {
                // Frontier empty. If nobody is producing, the search is done —
                // re-check the frontier under the lock to rule out a race with a
                // worker that just pushed children before going idle.
                if active.load(Ordering::Acquire) == 0 && frontier.lock().unwrap().is_empty() {
                    break;
                }
                std::thread::yield_now();
            }
        }
    }
    flush_nodes(search, local);
}

/// Explore the subtree rooted at `item` for up to `CHUNK_BUDGET` nodes with an
/// explicit-stack DFS. On budget exhaustion (or stop) the untried frontier is
/// pushed back onto the shared stack as new work items, so nothing is lost.
#[allow(clippy::too_many_arguments)]
fn explore_item(
    item: WorkItem,
    initial: &GameState,
    init_stab: u8,
    search: &Arc<SearchState>,
    frontier: &Mutex<Vec<WorkItem>>,
    local: &mut u64,
    k: i16,
    n: i16,
) {
    // Reconstruct the state and stabiliser at the item's node by replaying it.
    let mut state = initial.clone();
    let mut stab = init_stab;
    for &mv in &item {
        stab = stab_after(stab, &mv, k, n);
        // If this work item leads off the fixed grid, abandon it: the flag is now
        // set and the app will save the best and alert (see board::GRID_OVERFLOW).
        if !state.apply(mv) {
            return;
        }
    }
    let base_len = state.history.len();
    let forbid = forbid_of(initial.variant);
    let nu = n as usize;

    // Recycled buffers: popped frames return their `children`/`legal` to the
    // matching pool so child-set construction reuses allocations instead of
    // malloc/free per node (one pool per element type).
    let mut mpool: Vec<Vec<Move>> = Vec::new(); // `children` buffers
    let mut cpool: Vec<Vec<Cand>> = Vec::new(); // `legal` buffers

    *local += 1; // the item's own node
    let mut root_legal = take_buf(&mut mpool);
    legal_moves_into(&state, &mut root_legal); // full scan once at the chunk root
    let mut root_cand = take_buf(&mut cpool);
    root_candidates(&mut root_cand, &root_legal, &state); // O(depth) canon scan, once
    let mut root_children = take_buf(&mut mpool);
    canonical_children_into(&mut root_children, &root_cand, stab, k, n);
    if root_children.is_empty() {
        // Only a *truly terminal* position (no legal moves at all) is a game
        // result. With no canonical children but legal moves still available,
        // the continuations are reached via another move order elsewhere —
        // recording here would surface a non-terminal position (available > 0)
        // as a "best", which then can't be saved as a record.
        if root_legal.is_empty() {
            search.record_best(state.score() as u32, state.history.clone());
        }
        return;
    }
    mpool.push(root_legal); // the raw root scan is consumed; recycle it

    let mut stack = vec![Frame {
        children: root_children,
        idx: 0,
        stab,
        legal: root_cand,
    }];
    let mut budget = CHUNK_BUDGET;

    while budget > 0 && search.running.load(Ordering::Relaxed) {
        let top = stack.last_mut().unwrap();
        if top.idx >= top.children.len() {
            let done = stack.pop().unwrap();
            mpool.push(done.children); // recycle this frame's buffers
            cpool.push(done.legal);
            if stack.is_empty() {
                return; // subtree fully explored — nothing to push back
            }
            state.undo();
            continue;
        }
        let mv = top.children[top.idx];
        top.idx += 1;
        let child_stab = stab_after(top.stab, &mv, k, n);
        // A canonical child whose point falls in the grid margin can't be played;
        // skip it (state unchanged). The flag now signals the app to save & alert.
        if !state.apply(mv) {
            continue;
        }
        budget -= 1;
        *local += 1;
        if *local >= NODE_FLUSH_INTERVAL {
            flush_nodes(search, *local);
            *local = 0;
        }
        // Derive the child's legal set incrementally from the parent's (`top`
        // still borrows it; the borrow ends before the push below). Both buffers
        // come from a pool so the hot loop does no per-node allocation.
        let mut child_lgl = take_buf(&mut cpool);
        child_legal_into(&mut child_lgl, &top.legal, &state, mv, forbid, nu);
        #[cfg(debug_assertions)]
        {
            use std::collections::HashSet;
            let got: HashSet<Move> = child_lgl.iter().map(|c| c.mv).collect();
            let want: HashSet<Move> = legal_moves(&state).into_iter().collect();
            debug_assert_eq!(
                got, want,
                "incremental legal set diverged from full generation"
            );
            debug_assert_eq!(
                got.len(),
                child_lgl.len(),
                "duplicate in incremental legal set"
            );
            for c in &child_lgl {
                debug_assert_eq!(
                    c.canon,
                    canonical_ok(&c.mv, &state),
                    "incremental canon flag diverged from canonical_ok for {:?}",
                    c.mv
                );
            }
        }
        let mut children = take_buf(&mut mpool);
        canonical_children_into(&mut children, &child_lgl, child_stab, k, n);
        if children.is_empty() {
            // Record only true terminals (see the root case above): a leaf with
            // no canonical children but a non-empty legal set is not terminal.
            if child_lgl.is_empty() {
                search.record_best(state.score() as u32, state.history.clone());
            }
            state.undo();
            cpool.push(child_lgl); // recycle: this leaf spawned no frame
            mpool.push(children);
        } else {
            stack.push(Frame {
                children,
                idx: 0,
                stab: child_stab,
                legal: child_lgl,
            });
        }
    }

    // Budget spent: push the untried frontier back. The node of frame `i` is
    // reached by `item` followed by the first `i` moves applied this chunk.
    let descent: Vec<Move> = state.history[base_len..].to_vec();
    let mut f = frontier.lock().unwrap();
    for (i, frame) in stack.iter().enumerate() {
        for &c in &frame.children[frame.idx..] {
            let mut wi = Vec::with_capacity(item.len() + i + 1);
            wi.extend_from_slice(&item);
            wi.extend_from_slice(&descent[..i]);
            wi.push(c);
            f.push(wi);
        }
    }
}

/// Stabiliser of `state`: the bitmask of D4 transforms that map its occupied
/// points to occupied points and its drawn lines to drawn lines. Computed once
/// per search start (cheap); identity (bit 0) is always included.
fn initial_stabilizer(state: &GameState, k: i16, n: i16) -> u8 {
    let mut stab = 0u8;
    for t in 0..8 {
        let points_ok = state
            .board
            .cells
            .iter()
            .all(|&c| state.board.contains(apply_transform(t, c, k)));
        let lines_ok = state.history.iter().all(|mv| {
            state
                .line_index
                .contains(&transform_line(t, &mv.line, k, n))
        });
        if points_ok && lines_ok {
            stab |= 1 << t;
        }
    }
    stab
}

/// Add `n` to the shared explored-node counter (no-op for 0).
#[inline]
fn flush_nodes(search: &Arc<SearchState>, n: u64) {
    if n > 0 {
        search.nodes_explored.fetch_add(n, Ordering::Relaxed);
    }
}

/// Lexicographic normal form for traces: `mv` is canonical iff it could not
/// have been played earlier in the sequence.
///
/// Two moves *commute* (are independent) unless one uses the point the other
/// placed; since earlier moves can't use a later move's point, `mv` depends on
/// an earlier move `p` exactly when `mv`'s line passes through `p.pos`. Scanning
/// the path back from the most recent move: every commuting move must have a
/// smaller new point than `mv` (otherwise `mv` should have been played before
/// it); the scan stops at the first move `mv` depends on — the barrier it cannot
/// slide past. This generalises the old adjacent-only check to the whole
/// commuting suffix, eliminating *all* move-order transpositions with no memory.
fn canonical_ok(mv: &Move, state: &GameState) -> bool {
    let n = state.variant.len();
    // The n−1 already-occupied cells of mv's line (mv.pos is the new one).
    let mut deps = [(0i16, 0i16); 8];
    let mut ndeps = 0usize;
    for c in mv.line.positions(n) {
        if c != mv.pos {
            deps[ndeps] = c;
            ndeps += 1;
        }
    }
    for p in state.history.iter().rev() {
        if deps[..ndeps].contains(&p.pos) {
            return true; // mv depends on p — the barrier; mv legitimately follows
        }
        if p.pos > mv.pos {
            return false; // mv commutes with p but is smaller → it should precede p
        }
    }
    true
}

#[cfg(test)]
mod bench {
    use super::*;
    use crate::game::{moves::Move, rules::Variant, state::GameState};
    use std::hint::black_box;
    use std::time::Instant;

    /// Build the root DFS frame for `state` (full legal scan + canon flags +
    /// canonical children), mirroring `explore_item`'s chunk-root setup.
    fn root_frame(state: &GameState, stab: u8, k: i16, n: i16) -> Frame {
        let raw = legal_moves(state);
        let mut legal = Vec::new();
        root_candidates(&mut legal, &raw, state);
        let mut children = Vec::new();
        canonical_children_into(&mut children, &legal, stab, k, n);
        Frame {
            children,
            idx: 0,
            stab,
            legal,
        }
    }

    /// Release-safe divergence check: walk a bounded DFS and compare
    /// `child_legal` to `legal_moves` at every node. The debug_assertions
    /// block in `explore_item` only runs in debug builds; this test runs in
    /// both, so an aarch64 release miscompile of `child_legal` or
    /// `add_windows_through` shows up here as a FAIL rather than a silent
    /// never-terminal search.
    #[test]
    fn child_legal_matches_full_regen_release() {
        use std::collections::HashSet;
        let mut state = GameState::new(Variant::T5);
        let n = state.variant.len() as i16;
        let k = 2 * n - 1;
        let init_stab = initial_stabilizer(&state, k, n);
        let forbid = forbid_of(state.variant);
        let nu = n as usize;

        let mut stack: Vec<Frame> = vec![root_frame(&state, init_stab, k, n)];
        let mut nodes = 0u64;
        // 50 K nodes: fast in both debug and release; covers depths up to ~20 which
        // is enough to exercise the incremental set across many branch points.
        const NODE_LIMIT: u64 = 50_000;

        let mut incremental = Vec::new();
        while nodes < NODE_LIMIT {
            let top = stack.last_mut().unwrap();
            if top.idx >= top.children.len() {
                stack.pop();
                if stack.is_empty() {
                    break;
                }
                state.undo();
                continue;
            }
            let mv = top.children[top.idx];
            top.idx += 1;
            let cstab = stab_after(top.stab, &mv, k, n);
            state.apply(mv);
            nodes += 1;

            child_legal_into(
                &mut incremental,
                &stack.last().unwrap().legal,
                &state,
                mv,
                forbid,
                nu,
            );
            let full: Vec<Move> = legal_moves(&state);

            let inc_set: HashSet<Move> = incremental.iter().map(|c| c.mv).collect();
            let full_set: HashSet<Move> = full.iter().copied().collect();
            assert_eq!(
                inc_set,
                full_set,
                "child_legal diverged from legal_moves at depth {} after {} nodes\n\
                 extra in incremental: {:?}\n\
                 missing from incremental: {:?}",
                state.history.len(),
                nodes,
                inc_set.difference(&full_set).collect::<Vec<_>>(),
                full_set.difference(&inc_set).collect::<Vec<_>>(),
            );
            assert_eq!(
                incremental.len(),
                inc_set.len(),
                "duplicate in incremental legal set at depth {}",
                state.history.len()
            );
            // The incrementally-derived canon flag must match a fresh scan.
            for c in &incremental {
                assert_eq!(
                    c.canon,
                    canonical_ok(&c.mv, &state),
                    "incremental canon flag diverged at depth {} for {:?}",
                    state.history.len(),
                    c.mv,
                );
            }

            let mut children = Vec::new();
            canonical_children_into(&mut children, &incremental, cstab, k, n);
            if children.is_empty() {
                state.undo();
            } else {
                stack.push(Frame {
                    children,
                    idx: 0,
                    stab: cstab,
                    legal: std::mem::take(&mut incremental),
                });
            }
        }
    }

    /// Effective branching factor `b` (average canonical children per node) by
    /// depth, sampled with a single-threaded DFS from the cross. With game depth
    /// `d ≈ 178`, the canonical tree has ~`b^d` distinct positions — this turns
    /// the "10^70" estimate into a number from our actual tree.
    #[test]
    #[ignore = "measurement, run with --ignored --nocapture"]
    fn measure_branching() {
        let st = GameState::new(Variant::T5);
        let n = st.variant.len() as i16;
        let k = 2 * n - 1;
        let nu = n as usize;
        let forbid = forbid_of(st.variant);
        let init_stab = initial_stabilizer(&st, k, n);
        let cap: usize = std::env::var("CAP")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(12);

        // Exhaustive count of canonical nodes per depth up to `cap`. The exact
        // ratio nodes[d+1]/nodes[d] is the real growth rate of the tree.
        let mut count = vec![0u64; cap + 1];
        count[0] = 1; // the cross
        let mut state = st.clone();
        let mut stack = vec![root_frame(&state, init_stab, k, n)];
        while let Some(top) = stack.last_mut() {
            if top.idx >= top.children.len() {
                stack.pop();
                if !stack.is_empty() {
                    state.undo();
                }
                continue;
            }
            let mv = top.children[top.idx];
            top.idx += 1;
            let cstab = stab_after(top.stab, &mv, k, n);
            state.apply(mv);
            let d = state.history.len();
            count[d] += 1;
            if d < cap {
                let mut child_lgl = Vec::new();
                child_legal_into(&mut child_lgl, &top.legal, &state, mv, forbid, nu);
                let mut children = Vec::new();
                canonical_children_into(&mut children, &child_lgl, cstab, k, n);
                if children.is_empty() {
                    state.undo();
                } else {
                    stack.push(Frame {
                        children,
                        idx: 0,
                        stab: cstab,
                        legal: child_lgl,
                    });
                }
            } else {
                state.undo(); // at the cap: counted, don't recurse
            }
        }

        println!("BRANCHING: canonical nodes per depth (exhaustive to {cap})");
        let mut ratios = Vec::new();
        for d in 0..=cap {
            let r = if d > 0 && count[d - 1] > 0 {
                count[d] as f64 / count[d - 1] as f64
            } else {
                0.0
            };
            if d > 0 {
                ratios.push(r);
            }
            println!("  depth {d:>2}: {:>12} nodes   ratio x{r:.2}", count[d]);
        }
        // Geometric-mean growth over the last few levels (steady regime).
        let tail = &ratios[ratios.len().saturating_sub(5)..];
        let gmean = tail.iter().map(|r| r.ln()).sum::<f64>() / tail.len() as f64;
        let b = gmean.exp();
        println!("BRANCHING: steady growth b ≈ {b:.2}");
        for d in [80usize, 120, 150, 178] {
            println!("  b^{d} ≈ 10^{:.0}", d as f64 * b.log10());
        }
    }

    /// Best *terminal* score vs nodes explored — the empirical climb. Each extra
    /// move toward 178 costs exponentially more nodes (the curve flattens fast).
    #[test]
    #[ignore = "measurement, run with --ignored --nocapture"]
    fn measure_best_curve() {
        use std::time::Duration;
        let search = SearchState::new();
        let s2 = search.clone();
        let st = GameState::new(Variant::T5);
        let h = std::thread::spawn(move || run(&st, s2));
        for i in 1..=15 {
            std::thread::sleep(Duration::from_secs(2));
            println!(
                "{:>3}s: nodes={:>13} best_terminal={}",
                i * 2,
                search.nodes_explored.load(Ordering::Relaxed),
                search.best_score.load(Ordering::Relaxed),
            );
        }
        search.running.store(false, Ordering::Relaxed);
        h.join().unwrap();
    }

    /// Correctness of the trace normal form: among ALL valid reorderings of a
    /// fixed set of moves (one trace), exactly ONE must be canonical. More than
    /// one ⇒ redundant exploration; zero ⇒ the trace (position) would be lost.
    #[test]
    fn trace_nf_keeps_exactly_one_order_per_trace() {
        for seed_len in [3usize, 4, 5, 6] {
            // A short deterministic game gives the move set of one trace.
            let mut start = GameState::new(Variant::T5);
            let mut set = Vec::new();
            for _ in 0..seed_len {
                let ms = legal_moves(&start);
                let Some(mv) = ms.into_iter().min_by_key(|m| {
                    (
                        m.pos.0,
                        m.pos.1,
                        m.line.origin.0,
                        m.line.origin.1,
                        m.line_pos,
                    )
                }) else {
                    break;
                };
                set.push(mv);
                start.apply(mv);
            }

            let mut valid = 0usize;
            let mut canonical = 0usize;
            permute(&mut set.clone(), 0, &mut |perm: &[Move]| {
                let mut s = GameState::new(Variant::T5);
                let mut is_valid = true;
                let mut is_canon = true;
                for &mv in perm {
                    if !legal_moves(&s).contains(&mv) {
                        is_valid = false;
                        break;
                    }
                    if !canonical_ok(&mv, &s) {
                        is_canon = false;
                    }
                    s.apply(mv);
                }
                if is_valid {
                    valid += 1;
                    if is_canon {
                        canonical += 1;
                    }
                }
            });
            assert_eq!(
                canonical, 1,
                "len {seed_len}: {valid} valid orders, {canonical} canonical (want 1)"
            );
        }
    }

    /// Visit every permutation of `xs` (Heap's algorithm), calling `f` on each.
    fn permute(xs: &mut [Move], i: usize, f: &mut impl FnMut(&[Move])) {
        if i + 1 >= xs.len() {
            f(xs);
            return;
        }
        for j in i..xs.len() {
            xs.swap(i, j);
            permute(xs, i + 1, f);
            xs.swap(i, j);
        }
    }

    /// Build a deterministic mid-game position by always playing the first
    /// legal move, so the benchmark exercises a realistically large board.
    fn mid_game(moves: usize) -> GameState {
        let mut state = GameState::new(Variant::T5);
        for _ in 0..moves {
            let ms = legal_moves(&state);
            // Deterministic pick (HashSet iteration order is randomized).
            let Some(mv) = ms.into_iter().min_by_key(|m| {
                (
                    m.pos.0,
                    m.pos.1,
                    m.line.origin.0,
                    m.line.origin.1,
                    m.line_pos,
                )
            }) else {
                break;
            };
            state.apply(mv);
        }
        state
    }

    /// Decompose the per-node cost into its parts on a representative mid-game
    /// node, so hot-loop effort targets the real cost (measure, don't assume).
    #[test]
    #[ignore = "timing benchmark, run with --ignored --nocapture"]
    fn bench_node_breakdown() {
        let parent = mid_game(25);
        let n = parent.variant.len() as i16;
        let k = 2 * n - 1;
        let nu = n as usize;
        let forbid = forbid_of(parent.variant);

        let parent_legal = legal_moves(&parent);
        assert!(!parent_legal.is_empty());
        let mut parent_cand = Vec::new();
        root_candidates(&mut parent_cand, &parent_legal, &parent);
        let mv = parent_legal[0];
        let mut state = parent.clone();
        state.apply(mv);
        let stab = NO_SYMMETRY; // realistic: symmetry is gone after move 1
        let mut child_lgl = Vec::new();
        child_legal_into(&mut child_lgl, &parent_cand, &state, mv, forbid, nu);

        let iters = 5_000_000u64;
        let bench = |name: &str, mut f: Box<dyn FnMut() -> usize>| {
            for _ in 0..10_000 {
                black_box(f());
            }
            let t = Instant::now();
            let mut acc = 0usize;
            for _ in 0..iters {
                acc += black_box(f());
            }
            println!(
                "  {name:<22} {:.1} ns  (acc={acc})",
                t.elapsed().as_secs_f64() * 1e9 / iters as f64
            );
        };

        println!(
            "BREAKDOWN at depth {} ({} cells, parent_legal={}, child_legal={}):",
            state.history.len(),
            state.board.len(),
            parent_legal.len(),
            child_lgl.len(),
        );
        {
            let (pl, st) = (parent_cand.clone(), state.clone());
            let mut out = Vec::new();
            bench(
                "child_legal_into",
                Box::new(move || {
                    child_legal_into(&mut out, &pl, &st, mv, forbid, nu);
                    out.len()
                }),
            );
        }
        {
            let st = state.clone();
            let mut out = Vec::new();
            bench(
                "add_windows_through",
                Box::new(move || {
                    out.clear();
                    add_windows_through(&mut out, &st, mv.pos, forbid, nu);
                    out.len()
                }),
            );
        }
        {
            let cl = child_lgl.clone();
            let mut out = Vec::new();
            bench(
                "canonical_children_into",
                Box::new(move || {
                    canonical_children_into(&mut out, &cl, stab, k, n);
                    out.len()
                }),
            );
        }
        {
            let (cl, st) = (child_lgl.clone(), state.clone());
            bench(
                "canonical_ok (per set)",
                Box::new(move || cl.iter().filter(|c| canonical_ok(&c.mv, &st)).count()),
            );
        }
        {
            let mut s = parent.clone();
            bench(
                "apply+undo",
                Box::new(move || {
                    s.apply(mv);
                    s.undo();
                    s.board.len()
                }),
            );
        }
    }

    #[test]
    #[ignore = "timing benchmark, run with --ignored --nocapture"]
    fn bench_single_thread_node() {
        const BUDGET: u64 = 60_000_000;
        let st = GameState::new(Variant::T5);
        let n = st.variant.len() as i16;
        let k = 2 * n - 1;
        let nu = n as usize;
        let forbid = forbid_of(st.variant);
        let init_stab = initial_stabilizer(&st, k, n);

        let mut state = st.clone();
        let mut mpool: Vec<Vec<Move>> = Vec::new();
        let mut cpool: Vec<Vec<Cand>> = Vec::new();
        let mut root_legal = take_buf(&mut mpool);
        legal_moves_into(&state, &mut root_legal);
        let mut root_cand = take_buf(&mut cpool);
        root_candidates(&mut root_cand, &root_legal, &state);
        let mut root = take_buf(&mut mpool);
        canonical_children_into(&mut root, &root_cand, init_stab, k, n);
        mpool.push(root_legal);
        let mut stack = vec![Frame {
            children: root,
            idx: 0,
            stab: init_stab,
            legal: root_cand,
        }];
        let mut nodes = 0u64;
        let t0 = Instant::now();
        while nodes < BUDGET {
            let top = stack.last_mut().unwrap();
            if top.idx >= top.children.len() {
                let done = stack.pop().unwrap();
                mpool.push(done.children);
                cpool.push(done.legal);
                if stack.is_empty() {
                    break;
                }
                state.undo();
                continue;
            }
            let mv = top.children[top.idx];
            top.idx += 1;
            let cstab = stab_after(top.stab, &mv, k, n);
            state.apply(mv);
            nodes += 1;
            let mut child_lgl = take_buf(&mut cpool);
            child_legal_into(&mut child_lgl, &top.legal, &state, mv, forbid, nu);
            let mut children = take_buf(&mut mpool);
            canonical_children_into(&mut children, &child_lgl, cstab, k, n);
            if children.is_empty() {
                state.undo();
                cpool.push(child_lgl);
                mpool.push(children);
            } else {
                stack.push(Frame {
                    children,
                    idx: 0,
                    stab: cstab,
                    legal: child_lgl,
                });
            }
        }
        let dt = t0.elapsed().as_secs_f64();
        println!(
            "BENCH single-thread: nodes={nodes} rate={:.0} nodes/s ({:.1} ns/node)",
            nodes as f64 / dt,
            dt * 1e9 / nodes as f64,
        );
    }

    /// Parallel scaling curve: run the real search inside a Rayon pool of N
    /// threads (drive_workers reads `current_num_threads`/`scope`, so this sizes
    /// the worker count), measure nodes/s per N, and report speedup + efficiency
    /// vs the 1-thread rate. Shows where the frontier/`active` contention or
    /// starvation bends the curve away from linear.
    #[test]
    #[ignore = "measurement, run with --ignored --nocapture"]
    fn bench_scaling() {
        use std::time::Duration;
        let secs = 3.0;
        let max = rayon::current_num_threads();
        let mut threads: Vec<usize> = [1, 2, 4, 8, 16, 32, 48, 64]
            .into_iter()
            .filter(|&t| t <= max)
            .collect();
        if !threads.contains(&max) {
            threads.push(max);
        }
        println!("SCALING ({secs}s per point, host max {max} threads):");
        let mut base = 0.0;
        for nt in threads {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(nt)
                .build()
                .unwrap();
            let search = SearchState::new();
            let st = GameState::new(Variant::T5);
            let stopper = search.clone();
            let h = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs_f64(secs));
                stopper.running.store(false, Ordering::Relaxed);
            });
            let run_search = search.clone();
            pool.install(move || run(&st, run_search));
            h.join().unwrap();
            let n = search.nodes_explored.load(Ordering::Relaxed);
            let rate = n as f64 / secs;
            if nt == 1 {
                base = rate;
            }
            println!(
                "  {nt:>3} thr: {rate:>14.0} nodes/s   speedup {:>5.1}x   eff {:>4.0}%",
                rate / base,
                100.0 * rate / base / nt as f64,
            );
        }
    }

    #[test]
    #[ignore = "timing benchmark, run with --ignored --nocapture"]
    fn bench_nodes_per_sec() {
        let state = GameState::new(Variant::T5);
        let search = SearchState::new();
        let s2 = search.clone();
        let st = state.clone();
        let handle = std::thread::spawn(move || run(&st, s2));
        let secs = 3.0;
        std::thread::sleep(std::time::Duration::from_secs_f64(secs));
        search.running.store(false, Ordering::Relaxed);
        handle.join().unwrap();
        let n = search.nodes_explored.load(Ordering::Relaxed);
        println!(
            "BENCH end-to-end: nodes={n} rate={:.0} nodes/s",
            n as f64 / secs
        );
    }

    /// Regression: the systematic search must only record TERMINAL positions
    /// (0 legal moves). A canonical leaf that still has legal moves (case-b) is
    /// not a finished game and must not surface as a best/record. In release
    /// this reliably reaches terminals within ~1-2 s; in (much slower) debug it
    /// may reach none in time, in which case the invariant holds vacuously.
    #[test]
    fn recorded_best_is_terminal() {
        use std::time::Duration;
        let search = SearchState::new();
        let s2 = search.clone();
        let st = GameState::new(Variant::T5);
        let h = std::thread::spawn(move || run(&st, s2));
        // 12 s is enough for release on both x86 and aarch64 (first terminal
        // appears within ~10 s on Apple Silicon with the current CHUNK_BUDGET
        // via the std::thread launch path; rayon::spawn is somewhat faster).
        // Debug builds are much slower and may not reach a terminal in time;
        // the early-return below handles that case without a false failure.
        std::thread::sleep(Duration::from_secs(12));
        search.running.store(false, Ordering::Relaxed);
        h.join().unwrap();

        let best = search.best_sequence.read().unwrap().clone();
        if best.is_empty() {
            return; // no terminal reached in time (debug build) — nothing to check
        }
        let mut state = GameState::new(Variant::T5);
        for mv in &best {
            state.apply(*mv);
        }
        assert!(
            legal_moves(&state).is_empty(),
            "recorded best must be terminal, but {} legal move(s) remain",
            legal_moves(&state).len(),
        );
    }

    /// Diagnostic for the macOS "nodes climb but best stays 0" report: run the
    /// search via the *exact GUI launch path* (`rayon::spawn`, not a raw OS
    /// thread) and print best / nodes / overflow every 0.5 s.  Root cause was
    /// `CHUNK_BUDGET` being too small (20 k): the high branching factor at shallow
    /// depths (b ≈ 20 at depth 1) meant each chunk stayed shallow, never reaching
    /// terminal depth on macOS/aarch64.  With CHUNK_BUDGET = 500 000 a terminal
    /// appears within ~6 s on Apple Silicon.
    #[test]
    #[ignore = "measurement, run with --ignored --nocapture"]
    fn rayon_spawn_reaches_terminal() {
        use crate::game::board::GRID_OVERFLOW;
        use std::time::{Duration, Instant};
        GRID_OVERFLOW.store(false, Ordering::Relaxed);
        let search = SearchState::new();
        let s2 = search.clone();
        let st = GameState::new(Variant::T5);
        rayon::spawn(move || run(&st, s2));
        let start = Instant::now();
        for _ in 0..16 {
            std::thread::sleep(Duration::from_millis(500));
            println!(
                "{:>4.1}s : best={} nodes={} overflow={} threads={}",
                start.elapsed().as_secs_f64(),
                search.best_score.load(Ordering::Relaxed),
                search.nodes_explored.load(Ordering::Relaxed),
                GRID_OVERFLOW.load(Ordering::Relaxed),
                rayon::current_num_threads(),
            );
        }
        search.running.store(false, Ordering::Relaxed);
    }

    /// Diagnostic: how soon does the systematic search reach an actual TERMINAL
    /// (0 legal moves) vs only canonical-leaf (case-b) positions?
    #[test]
    #[ignore = "measurement, run with --ignored --nocapture"]
    fn first_terminal_time() {
        use std::time::{Duration, Instant};
        let search = SearchState::new();
        let s2 = search.clone();
        let st = GameState::new(Variant::T5);
        let h = std::thread::spawn(move || run(&st, s2));
        let start = Instant::now();
        for _ in 0..20 {
            std::thread::sleep(Duration::from_secs(1));
            let best = search.best_sequence.read().unwrap().clone();
            let terminal = {
                let mut s = GameState::new(Variant::T5);
                for mv in &best {
                    s.apply(*mv);
                }
                legal_moves(&s).is_empty()
            };
            println!(
                "{:>2.0}s : best={} terminal={terminal} nodes={}",
                start.elapsed().as_secs_f64(),
                best.len(),
                search.nodes_explored.load(Ordering::Relaxed),
            );
        }
        search.running.store(false, Ordering::Relaxed);
        h.join().unwrap();
    }

    /// Throughput and best score reached in a fixed window, per variant.
    /// (4T/4D are excluded: their long games currently overflow the fixed grid.)
    #[test]
    #[ignore = "measurement, run with --ignored --nocapture"]
    fn measure_pruning() {
        use std::time::Duration;
        for (name, variant) in [("5T", Variant::T5), ("5D", Variant::D5)] {
            let search = SearchState::new();
            let s2 = search.clone();
            let st = GameState::new(variant);
            let h = std::thread::spawn(move || run(&st, s2));
            std::thread::sleep(Duration::from_secs(3));
            search.running.store(false, Ordering::Relaxed);
            h.join().unwrap();
            println!(
                "{name}/3s : nodes={:>11} best={}",
                search.nodes_explored.load(Ordering::Relaxed),
                search.best_score.load(Ordering::Relaxed),
            );
        }
    }

    /// Run a search, trigger a checkpoint, then read it back and report the save
    /// size and (de)serialise times — so we can pick the auto-checkpoint period.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "measurement, run with --ignored --nocapture"]
    fn checkpoint_live() {
        use std::time::{Duration, Instant};
        let search = SearchState::new();
        let s2 = search.clone();
        let st = GameState::new(Variant::T5);
        let h = std::thread::spawn(move || run(&st, s2));

        std::thread::sleep(Duration::from_secs(4));
        search.checkpoint_requested.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(800)); // let it serialise + resume
        search.running.store(false, Ordering::Relaxed);
        h.join().unwrap();

        let path = checkpoint::path("systematic");
        let content = std::fs::read_to_string(&path).expect("checkpoint file");
        let t0 = Instant::now();
        let cp = io::import_checkpoint(&content).expect("valid checkpoint");
        let import_ms = t0.elapsed().as_secs_f64() * 1e3;

        // Re-serialise to time the save itself.
        let t1 = Instant::now();
        let _ = io::export_checkpoint(
            cp.variant,
            cp.nodes_explored,
            &cp.best,
            &cp.records,
            &cp.frontier,
            "systematic",
            0,
        )
        .unwrap();
        let export_ms = t1.elapsed().as_secs_f64() * 1e3;

        println!(
            "CHECKPOINT: {} bytes, {} frontier items, best {}, export {export_ms:.0} ms, import {import_ms:.0} ms",
            content.len(),
            cp.frontier.len(),
            cp.best.len(),
        );
        assert!(!cp.frontier.is_empty());
        assert_eq!(cp.variant, Variant::T5);
    }

    /// Run → checkpoint → stop, then resume from the file and confirm progress
    /// is restored and continues (best kept, node count carried over and grows).
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "measurement, run with --ignored --nocapture"]
    fn resume_continues() {
        use std::time::Duration;

        // First search: run, checkpoint, stop.
        let search = SearchState::new();
        let s2 = search.clone();
        let st = GameState::new(Variant::T5);
        let h = std::thread::spawn(move || run(&st, s2));
        std::thread::sleep(Duration::from_secs(3));
        search.checkpoint_requested.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(800));
        search.running.store(false, Ordering::Relaxed);
        h.join().unwrap();
        let best_before = search.best_score.load(Ordering::Relaxed);
        let nodes_before = search.nodes_explored.load(Ordering::Relaxed);

        // Resume from disk.
        let cp = checkpoint::load("systematic").expect("checkpoint on disk");
        let frontier_len = cp.frontier.len();
        let search2 = SearchState::new();
        let r2 = search2.clone();
        let h2 = std::thread::spawn(move || super::resume(r2, cp));
        std::thread::sleep(Duration::from_secs(2));
        search2.running.store(false, Ordering::Relaxed);
        h2.join().unwrap();

        let best_after = search2.best_score.load(Ordering::Relaxed);
        let nodes_after = search2.nodes_explored.load(Ordering::Relaxed);
        println!(
            "RESUME: frontier {frontier_len} | best {best_before}->{best_after} | nodes {nodes_before}->{nodes_after}"
        );
        assert!(best_after >= best_before, "best must be restored, not lost");
        assert!(
            nodes_after > nodes_before,
            "node count restored and search continued"
        );
    }

    #[test]
    #[ignore = "timing benchmark, run with --ignored --nocapture"]
    fn bench_legal_moves() {
        let state = mid_game(25);
        assert!(!legal_moves(&state).is_empty(), "want a non-terminal node");
        let iters = 300_000u64;
        // Warm up.
        for _ in 0..1000 {
            black_box(legal_moves(black_box(&state)));
        }
        let t0 = Instant::now();
        let mut acc = 0usize;
        for _ in 0..iters {
            acc += black_box(legal_moves(black_box(&state))).len();
        }
        let dt = t0.elapsed().as_secs_f64();
        println!(
            "BENCH legal_moves: cells={} iters={iters} {:.0} calls/s ({:.1} ns/call) acc={acc}",
            state.board.len(),
            iters as f64 / dt,
            dt * 1e9 / iters as f64,
        );

        // Reused-buffer variant (what the search hot path uses).
        let mut buf = Vec::new();
        for _ in 0..1000 {
            crate::game::moves::legal_moves_into(&state, &mut buf);
            black_box(&buf);
        }
        let t1 = Instant::now();
        let mut acc2 = 0usize;
        for _ in 0..iters {
            crate::game::moves::legal_moves_into(black_box(&state), &mut buf);
            acc2 += black_box(buf.len());
        }
        let dt2 = t1.elapsed().as_secs_f64();
        println!(
            "BENCH legal_moves_into: {:.0} calls/s ({:.1} ns/call) acc={acc2}",
            iters as f64 / dt2,
            dt2 * 1e9 / iters as f64,
        );
    }
}

// ── registry plugin (co-located with the systematic engine) ───────────────────

use crate::search::plugin::{Method, Plugin, Registry, StartCtx};

struct Systematic;
impl Method for Systematic {
    fn id(&self) -> &'static str {
        "systematic"
    }
    fn label_key(&self) -> &'static str {
        "algo-systematic"
    }
    fn spawn(&self, ctx: StartCtx, search: Arc<SearchState>) {
        let initial = ctx.initial;
        std::thread::spawn(move || run(&initial, search));
    }
    fn method_desc(&self, _ctx: &StartCtx) -> String {
        "systematic".to_owned()
    }
    fn checkpoint_kind(&self) -> Option<&'static str> {
        Some("systematic")
    }
}
static SYSTEMATIC: Systematic = Systematic;

pub struct SystematicPlugin;
impl Plugin for SystematicPlugin {
    fn id(&self) -> &'static str {
        "systematic"
    }
    fn register(&self, reg: &mut Registry) {
        reg.add_method(&SYSTEMATIC);
    }
}
pub static SYSTEMATIC_PLUGIN: SystematicPlugin = SystematicPlugin;
