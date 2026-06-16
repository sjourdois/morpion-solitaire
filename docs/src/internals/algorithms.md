# Search algorithms

The search space is astronomical — a 5T game can run past 170 moves with a wide
branching factor — so every solver but one is heuristic. The exception is the
systematic search, which is exact but only tractable for the small variants.
All of them work on the same `GameState` (a bitboard position plus the move
history) and stream their best game live to the GUI. Full citations are in the
[bibliography](../reference/bibliography.md); the GUI's algorithm selector lists
them in the order below, from the exact baseline to the most sophisticated
heuristic.

## Systematic search

Exhaustive backtracking that is **exact** and **stateless**: it visits every
reachable position and keeps the best, with memory bounded only by the DFS stack,
so a run can continue indefinitely without growing. There is deliberately **no
transposition table** — instead two exact, memory-free layers guarantee each
position is reached exactly once:

1. A **trace normal form.** Moves are kept in a canonical order and a move is
   explored only if it could not have been played earlier in the sequence. This
   removes *all* move-order transpositions with no stored visited-set — the prune
   is a local test on the candidate move against the trace so far.
2. **Structural D4-symmetry** pruning. Only one representative per orbit of the
   position's current symmetry stabiliser is expanded. The stabiliser starts as
   the full dihedral group of the cross and collapses to the identity after the
   first symmetry-breaking move, so the saving is largest near the root where the
   tree is widest.

A branch-and-bound layer then prunes any branch whose admissible upper bound
cannot beat the incumbent; the theoretical length limits \[Demaine2006] are the
backdrop for those bounds. For 4D and 4T the whole tree can be drained, which
**proves the optimum** (35 and 62 respectively): when the frontier empties on its
own, `SearchState::exhausted` is set and the app reports the elapsed time and that
the score is optimal. For 5T/5D the space is far too large to exhaust, so the
systematic search confirms small results rather than hunting records.

## Beam search

A bounded breadth-first search: at each depth keep only the best `width` states,
expand them all, and repeat. States are scored by their current score plus a
weight on the number of remaining legal moves — a crude potential that favours
positions which keep their options open. It is deterministic, memory-bounded by
the beam width, and never backtracks, which makes it a fast, predictable
**baseline** rather than a record-setter: too narrow a beam commits to a dead end
early, while a wide beam costs memory without the policy learning that NRPA
brings. The design follows the beam / parallel-NRPA line of work \[Cazenave2020].

## NRPA

**Nested Rollout Policy Adaptation** \[Rosin2011] — which refines Nested
Monte-Carlo Search \[Cazenave2009] and generalises as GNRPA \[Edelkamp2016] — is
the workhorse for the large variants. It learns a **policy**: a table mapping a
*symmetry-invariant move code* to a real weight. Sampling and learning are nested
by level:

- A **level-0 playout** plays a full game, at each step sampling a legal move with
  probability `softmax(policy)` over the candidates' codes.
- A **level-N run** performs a fixed number of level-(N−1) runs; after each it
  adapts the policy toward the moves of the best game seen so far (a gradient step
  that raises the chosen codes and lowers their alternatives), then recurses.

Because the code is symmetry-invariant, a lesson learned in one orientation
transfers to all eight, sharply shrinking what must be learned. Independent
**island restarts** — one per core — supply diversity and are merged by keeping
the global best. A **warm start** can pre-train the policy toward a known strong
game (load a bundled record from the dropdown, or pass `--warm game.msr` to the
CLI) so the search begins in a good basin instead of from scratch.

The **nesting level** trades breadth for depth: level 3 is the responsive default;
level 4 and above search far more deeply per run but only pay off over multi-hour
sessions.

The adapted policy logits are **clamped** to a small bound (a Stabilized-NRPA
variant, on by default; `NRPA_CLAMP=0` disables) so the policy cannot run away and
over-commit. This both raises the typical score and cuts its run-to-run variance,
and the gain *grows* with the search budget rather than capping (5T level-4
mean-best went from ~95 unclamped to ~112 clamped in tuning).

## Perturbation (large-neighbourhood) search

A destroy-and-repair **large-neighbourhood search** \[Shaw1998, Ropke2006] built
on top of NRPA — the solver's main record-climber. Instead of a single incumbent
it keeps a quality-diversity **archive** \[Mouret2015] of high-scoring but
*structurally diverse* games. Each round it:

1. picks a game from the archive,
2. **destroys** a suffix of it (drops the last moves), and
3. **repairs** the remainder with a short, warm NRPA search seeded by what was
   kept.

Improved or novel games re-enter the archive. Diversity is the whole point: a
plain hill-climb collapses onto one local optimum, whereas the archive preserves
promising-but-different lines so the search can escape and recombine them. It runs
its inner NRPA searches across OS threads, so it is **native-only** — the web
build omits it (browsers can't spawn the same thread pool).

## Record-hunting workflow

A practical loop: pick the variant; optionally warm-start from the best bundled
record; run **NRPA** across all cores, or **perturbation** for a sustained climb;
and let the app auto-save every position that beats the best already stored for
that algorithm category. Beating the 5T world record (178) raises an audible
alarm. Indicative reach per variant is listed in the project README.
