# Search algorithms

The space is astronomical, so the solvers are heuristic — except the systematic
search, which is exact but only tractable for the small variants. Full citations
are in the [bibliography](../reference/bibliography.md).

## NRPA

**Nested Rollout Policy Adaptation** \[Rosin2011], refining Nested Monte-Carlo
Search \[Cazenave2009]. A policy maps a symmetry-invariant move code to a logit
weight; a level-0 playout samples moves by `softmax(policy)`, and a level-N run
does N playouts of level N−1, adapts the policy toward the best, and recurses.
Independent island restarts (one per core) supply diversity. A **warm start** can
pre-train the policy toward a known strong game (load a record from the dropdown,
or pass `--warm game.msr`), so the search begins near a good basin instead of
from scratch.

Nesting level trades breadth for depth: level 3 is the fast default; level 4+
searches more deeply but only pays off over multi-hour runs.

## Perturbation (large-neighbourhood) search

Built on NRPA: keep a quality-diversity **archive** \[Mouret2015] of diverse high
games; each round, destroy a suffix of one and repair it with a short warm NRPA
search — a destroy/repair large-neighbourhood search \[Shaw1998]. This is how the
solver climbs toward records: the archive preserves promising-but-different
games so the search doesn't collapse onto a single local optimum.

## Beam search

Keep the top-`width` states at each depth, scored by current score plus a weight
on remaining legal moves (a crude potential heuristic), and expand
breadth-first. Simple and memory-bounded; a useful baseline rather than a
record-setter.

## Systematic search

Exhaustive backtracking that is **exact** and **stateless** — memory is bounded
by the DFS stack, so a search can run indefinitely. Each position is reached
exactly once via two exact, memory-free layers:

1. a **trace normal form**, which explores a move only if it could not have been
   played earlier, eliminating *all* move-order transpositions; and
2. **structural D4-symmetry** pruning — only one representative per orbit of the
   current stabiliser is explored (the stabiliser shrinks to the identity after
   the first generic move).

Branch-and-bound prunes with an admissible upper bound; the theoretical length
limits are the backdrop \[Demaine2006]. For the small variants the whole tree can
be drained, which **proves the optimum** (35 for 4D, 62 for 4T): when the
frontier empties on its own, `SearchState::exhausted` is set and the app
announces it. For 5T/5D the space is far too large to exhaust.

## Record-hunting workflow

A practical loop: pick the variant, optionally warm-start from the best bundled
record, run NRPA (or perturbation) across all cores, and let the app auto-save
every position that beats the best already saved for that algorithm category.
Beating the 5T world record (178) raises an audible alarm. The project README
lists indicative reach per variant.
