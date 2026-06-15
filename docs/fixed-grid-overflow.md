# Fixed-grid board & overflow handling

## Design (fixed grid, resizable by one type alias)

Board occupancy is a fixed **GRID×GRID bitset**, one `Row` word per grid row
(`src/game/board.rs`). The grid side length is `GRID = Row::BITS`, so resizing
the grid is a **one-line change** to a single type alias:

```rust
pub type Row = u128; // 128×128 grid; `u64` → 64×64
```

Everything else (`GRID`, `OFFSET`, the line-index tracks, the SWAR move
generator's masks) derives from `Row`, so nothing else changes. There is no
primitive `u256`; going beyond 128 needs a `Row` newtype over `[u128; k]`
implementing the handful of bit ops used here (`Shl`, `Shr`, `BitAnd`, `BitOr`,
`Not`, `trailing_zeros`, `== 0`).

A point at internal coordinate `(x, y)` maps to grid index `(x + OFFSET, y +
OFFSET)`, with `OFFSET = GRID/2 − 5` centring the initial cross. `Board::contains`
is then an O(1) bit test — no hashing — the single biggest throughput lever for
the systematic search.

A **margin** keeps queries in bounds without a per-query check: no cell may be
placed within `MARGIN = n − 1 = 4` cells of the edge. Since every window
inspected by `legal_moves` is anchored on an occupied (interior) cell and extends
at most `n − 1` cells past it, every `contains`/`row` query lands in `[0, GRID)`.
The margin check is a single comparison in `Board::insert`, run once per placed
move — negligible next to move generation.

## Overflow handling (graceful: detect → save → alert, never panic)

With `GRID = 128` the interior is ~120 cells per side and a record 5T game
(≈178 moves) spans only ~40, so overflow cannot fire for any realistic game on
the 128 grid; the realistic candidate is the 64 grid. When it *does* happen,
the engine never crashes:

1. **Detection (free).** `Board::insert` is the seam. If a placement falls in the
   margin it sets the global `pub static GRID_OVERFLOW: AtomicBool` and returns
   `false` **without writing** — one comparison, no panic, no allocation.
2. **Propagation.** `GameState::apply` returns the `bool`; the search loops check
   it (NRPA playout/adapt `break`, systematic `explore` skips the move). The game
   so far is left valid.
3. **Save + alert.** The app (and the CLI's `search`) poll
   `GRID_OVERFLOW.swap(false, …)` each tick, then stop the search, save the best
   game to `records/overflow/`, and show an alert telling the user to widen
   `Row`. See `MorpionApp::handle_grid_overflow`.

### Resize & resume

Saves are **grid-independent**: both the `.msr` record and the search checkpoint
store *internal* `Pos` coordinates, never grid indices (the index is computed
only at `insert` time as `pos + OFFSET`). So enlarging `Row` grows `OFFSET` and
re-centres the same game with room on every side. The workflow is:

> widen `Row` in `board.rs` → rebuild → load the `records/overflow/…` record (or
> resume the checkpoint) → keep searching past the old boundary.

## Possible future: automatic escalation

Today resizing is manual. Because a game's move history (`Vec<Move>`, in absolute
coordinates) is the sole source of truth — the bitset is a reconstructible
projection — overflow could instead be handled automatically: on overflow,
enqueue the branch's history and replay it on a larger-grid `Board` in a
secondary pass, keeping the common path on the small cache-friendly grid. This is
*not* implemented; the cheap `GRID_OVERFLOW` seam is exactly where it would slot
in. During search there are hundreds of live states (one per DFS stack across
workers), so escalation would be per-branch, not global.
