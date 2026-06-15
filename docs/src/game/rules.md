# Rules and variants

Morpion Solitaire is played on a square grid, starting from a fixed **cross** of
points (36 points for the 5-in-a-line game).

A **move**:

1. places one new point on a grid intersection, and
2. draws a straight line — horizontal, vertical, or diagonal — through `n`
   consecutive points, **including the new one**. All `n − 1` other points of the
   line must already exist.

The **score** is the number of moves; play continues until no legal move
remains. The goal is the longest possible game.

## The touch rule

Two lines in the **same direction** that lie on the same track may not overlap by
more than the allowed amount:

- **Touching (T):** parallel collinear lines may share **one** endpoint.
- **Disjoint (D):** parallel collinear lines must be **strictly separate**.

(Lines in different directions, or crossing, never conflict by this rule.)

## Variants

| Code | Line length `n` | Touch rule |
|------|-----------------|------------|
| `5T` | 5 | touching — the classic game; world record **178** |
| `5D` | 5 | disjoint |
| `4T` | 4 | touching |
| `4D` | 4 | disjoint |

All four are supported by the application and the MSR format. The exact
coordinate frame, the initial cross, and the touch rule as a formula are defined
normatively in the [MSR specification](../format/spec.md).
