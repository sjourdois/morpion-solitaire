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

These two exact layers are the whole pruning strategy — there is no heuristic
branch-and-bound; correctness rests only on never discarding a reachable
position. The theoretical length limits \[Demaine2006] are the backdrop for what
the search can hope to reach. For 4D and 4T the whole tree can be drained, which
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
variant, on by default; set `--clamp 0`, or the clamp slider to 0 in the GUI, to
disable) so the policy cannot run away and over-commit. This both raises the typical
score and cuts its run-to-run variance, and the gain *grows* with the search budget
rather than capping (5T level-4 mean-best went from ~95 unclamped to ~112 clamped in
tuning).

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

## Experimental methods

A second tier of engines and tuning knobs is **off by default** and revealed only
with the global `--experimental` flag on the CLI, or the *Experimental engines &
options* toggle in the GUI's search setup. These are research levers: more moving
parts, a heavier build, and no guarantee of beating the defaults above. The neural
ones need a build with the `neural` feature and run **native-only** (CPU inference);
the web build omits them.

### Neural move prior

A small network scores each candidate move from a fixed-size view of its local
neighbourhood, and that score is added into NRPA's softmax as a log-space bias —
nudging sampling toward the moves the net favours. Arm it with `search --prior
<source>`, where the source is the shipped `bundled` prior, `corpus` (train on the
record corpus first), `scratch` (train on the bundled from-scratch corpus), or a
path to a safetensors file; `--neural-scale` sets the bias strength. Only the NRPA
family reads it.

You can also train one **from scratch, with no human games**, using the
`tabula-rasa` command — cold-start Expert Iteration: an NRPA seed, prior-guided
perturbation, retrain on the elite, repeat — and save the result for `--prior`.

### Feature-space NRPA

Instead of a frozen per-move bias, freeze the net's penultimate **features** φ(s,m)
and adapt a linear head θ over them *online during the search*: the per-move logit
becomes θ·φ, and one update moves **every** move sharing features — the in-search
generalization a one-hot policy table cannot give. Turn it on with `--feat-adapt`
(a prior must be armed); `--feat-alpha` sets the head step. The default keeps the
one-hot table alongside θ·φ; advanced knobs switch to a head-only variant
(`--no-feat-table`), cold initialization (`--no-feat-warm`), L2 decay
(`--feat-lambda`), a head clamp (`--feat-clamp`) and φ normalization (`--feat-norm`).

### Macro-actions

NRPA can pick, in one step, a `k`-move **motif** mined from the record corpus
instead of a single move — raising the action granularity so the policy composes
over a shorter horizon. Enable with `--macros`; `--macro-k` sets the motif length
and `--macro-topn` how many of the most frequent motifs to offer. 5T only.

### PUCT

A policy + value **tree search** (`--algo puct`): one tree grown by repeated
simulations that descend by the PUCT rule, expand a leaf with the policy prior, and
back up either a policy-guided rollout's length or a value net's estimate. Arm the
policy with `--prior` (without one it runs as uniform-rollout MCTS) and, optionally,
a value net from the `train-value` command via `--value-net <file>`. It guides whole
*lines* through a position value rather than per-move imitation.

## Record-hunting workflow

A practical loop: pick the variant; optionally warm-start from the best bundled
record; run **NRPA** across all cores, or **perturbation** for a sustained climb;
and let the app auto-save every position that beats the best already stored for
that algorithm category. Beating the 5T world record (178) raises an audible
alarm. Indicative reach per variant is listed in the project README.
