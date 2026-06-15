# MSR — Morpion Solitaire Record format, version 0.1

**Status:** draft · **Format version:** `0.1` · **Media type:**
`application/vnd.morpion-solitaire.record+json` · **File extension:** `.msr`

Each version of this specification lives at a stable, versioned URL
(`…/spec/0.1/`), so published links never break when a later version appears.
**0.1** is the current draft; **1.0** will be the first stable release.

MSR is a self-describing interchange format for [Morpion Solitaire] games: an
ordered list of moves plus optional provenance metadata. It is designed to
replace the older *Pentasol* text format with a lossless, variant-complete,
metadata-carrying, and independently verifiable record.

The reference implementation is the `morpion-solitaire-record` crate (imported
as `msr`).

## 1. Conformance

The key words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT** and **MAY** are
to be interpreted as in [RFC 2119].

A **reader** is conformant if it accepts every valid document defined here. A
**writer** is conformant if every document it emits is valid here. A
**validator** additionally checks game legality (§7).

## 2. The game (informative)

Morpion Solitaire starts from a fixed cross of points on a grid. A move places
one new point and draws a straight line of `n` consecutive collinear points
(horizontally, vertically or diagonally) through it; all other `n − 1` points of
the line must already exist. A *touch rule* constrains how new lines may overlap
earlier parallel lines. The score is the number of moves. Variants differ by `n`
(4 or 5) and by the touch rule (touching or disjoint). The 5T world record is
178 (Rosin, 2011); see the project bibliography.

## 3. Coordinate system (normative)

Points are integer `(x, y)` pairs in a single, translation-fixed frame:

- The **initial cross** occupies the grid `0..=w` on both axes, where
  `w = 2n−1` for odd `n` and `w = 2n−2` for even `n` — so `0..=9` (the classic
  36-point cross) for `n = 5`, and `0..=6` for `n = 4`. Its members are defined
  by §3.1.
- Moves **MAY** use any integer coordinates, including negative ones, as a game
  grows outward from the cross.

The frame is **independent of any grid size**: it is an unbounded integer
lattice. (Implementations backed by a fixed grid translate this frame by a
constant offset; that offset is an implementation detail and is never stored.)

### 3.1 Initial cross (normative)

For line length `n`, let `arm = n−1`, and `w = 2n−1` for odd `n` or `w = 2n−2`
for even `n`. Then `a = (w − arm + 1) / 2` and `b = a + arm − 1`, so the arm band
`[a, b]` is centred (`a + b = w`) and the figure is D4-symmetric — this centring
is why even `n` uses the narrower `w = 2n−2` grid. A grid point `(x, y)` with
`0 ≤ x, y ≤ w` belongs to the cross **iff**:

```
((y = 0 or y = w) and a ≤ x ≤ b)        # top / bottom caps
or ((x = 0 or x = w) and a ≤ y ≤ b)     # left / right caps
or ((x = a or x = b) and (y ≤ a or y ≥ b))   # vertical arm borders
or ((y = a or y = b) and (x ≤ a or x ≥ b))   # horizontal arm borders
```

This yields the classic 36-point Greek cross for `n = 5` (`a = 3, b = 6, w = 9`)
and a centred 24-point cross for `n = 4` (`a = 2, b = 4, w = 6`).

## 4. Variants (normative)

A variant is encoded as a two-character code, digit first:

| Code | Line length `n` | Touch rule | `max_overlap` |
|------|-----------------|------------|---------------|
| `4T` | 4 | touching | 1 |
| `4D` | 4 | disjoint | 0 |
| `5T` | 5 | touching | 1 |
| `5D` | 5 | disjoint | 0 |

Readers **MUST** accept the canonical code. Readers **MAY** also accept the
reversed spelling (`T5`) and any letter case; writers **MUST** emit the
canonical digit-first uppercase form.

## 5. Directions and lines (normative)

A direction has a unit step `δ = (dx, dy)`, oriented so the *origin* is the
smaller-coordinate end:

| Direction | `δ` |
|-----------|-----|
| `H`  | `(1, 0)`  |
| `V`  | `(0, 1)`  |
| `DP` | `(1, −1)` |
| `DN` | `(1, 1)`  |

A move stores the new point `(x, y)`, the direction, and `pos` — the index of
the new point within the line, in `0..n`. The line's **origin** is therefore
`(x, y) − pos·δ`, and its points are `origin + i·δ` for `i ∈ 0..n`.

## 6. The record (normative)

A record is a JSON object. Field names and value encodings are normative.

| Field | Type | Req. | Meaning |
|-------|------|------|---------|
| `version` | string | yes¹ | Format version, `major.minor`. `"0.1"` for this spec. |
| `variant` | string | yes | Variant code (§4). |
| `moves` | array | yes | Moves in play order (§6.1). |
| `score` | integer | yes | `moves.length` (stored for readability). |
| `producer` | string | no | Program that wrote the file (`name/version`). |
| `available_moves` | integer | no | Legal moves at the final position (`0` ⇔ terminal). |
| `terminal` | boolean | no | Whether the final position is terminal. |
| `bbox` | array[4] int | no | `[min_x, min_y, max_x, max_y]` of all points. |
| `saved_at` | string | no | ISO-8601 UTC save time (e.g. `"2026-06-14T10:30:00Z"`). |
| `description` | string | no | Free-text human description. |
| `author` | string | no | Who set the record (person/team/handle). |
| `source` | string | no | Where the *game* originates: a URL or citation (e.g. the original record site or an imported Pentasol file). |
| `transcribed_by` | string | no | Who transcribed the game into MSR form (curator/project), distinct from `source` and `author`. |
| `tags` | array[string] | no | Free-form labels. |
| `solver` | object | no | Automated-search provenance (§6.2); present **only** for solver-produced games. |

¹ A reader **MAY** default a missing `version` to `"0.1"`, and **SHOULD** accept a
bare integer (e.g. `1`) as its decimal string for backward compatibility; writers
**MUST** emit it as a `major.minor` string.

Optional fields **SHOULD** be omitted when empty rather than written as `null`.
Derived fields (`score`, `available_moves`, `terminal`, `bbox`) **SHOULD** be
computed from the moves by the writer; a reader **MUST NOT** trust them over the
moves themselves and **SHOULD** recompute when it needs them.

The three provenance fields answer distinct questions: `author` — *who achieved
the game*; `source` — *where the game comes from*; `transcribed_by` — *who put it
into MSR form*. A human/hand-played/transcribed record carries no `solver` block.

### 6.1 The move object (normative)

| Field | Type | Meaning |
|-------|------|---------|
| `x` | integer | X of the new point. |
| `y` | integer | Y of the new point. |
| `dir` | string | Direction code: `H`, `V`, `DP`, `DN`. |
| `pos` | integer | Index of the new point in the line, `0..n`. |

### 6.2 The `solver` object (normative)

Present only when an automated search produced the game. Every field is
optional; a writer **SHOULD** omit the whole object rather than emit it empty.

| Field | Type | Meaning |
|-------|------|---------|
| `tool` | string | The search tool/engine that produced the game (name or brand, e.g. `"morpion-solitaire.io"`). Distinct from the file-level `producer`. |
| `method` | string | Algorithm + parameters, e.g. `"nrpa L3"`. **MUST NOT** restate `seed`; a warm-start game length is written as a distinct token, e.g. `"nrpa-seeded L3 warm-from=178"`. |
| `seed` | integer | RNG seed, for reproducibility. |
| `nodes_explored` | integer | Search effort in nodes. |
| `elapsed_secs` | number | Wall-clock seconds of the producing search. |

## 7. Legality (normative for validators)

A record is **legal** iff replaying its moves from the initial cross (§3.1)
satisfies, for every move with line points `P₀…P₍ₙ₋₁₎` and new point `N = (x, y)`
at index `pos`:

1. `0 ≤ pos < n`;
2. `N` is **not** currently occupied;
3. every `Pᵢ ≠ N` **is** currently occupied;
4. the new line does not break the **touch rule** (§7.1).

On success the new point becomes occupied and the line is recorded.

### 7.1 Touch rule (normative)

Two lines conflict only if **collinear** — same direction and same *track*:

| Direction | track | position |
|-----------|-------|----------|
| `H`  | `y`     | `x` |
| `V`  | `x`     | `y` |
| `DP` | `x + y` | `x` |
| `DN` | `x − y` | `x` |

(track/position are computed from each line's origin.) Two same-direction lines
on the same track conflict iff `|position₁ − position₂| ≤ forbid`, where
`forbid = n − 1 − max_overlap` (§4). Equivalently: parallel collinear lines may
overlap by at most `max_overlap` points (1 for touching, 0 for disjoint), and a
move is illegal if its line conflicts with any earlier line.

## 8. Encodings (normative)

The same record has two interchangeable serialisations:

- **JSON form.** The record object as UTF-8 JSON (pretty or compact). This is
  the readable, diff-friendly form.
- **Compact form (`MS1:`).** The string `MS1:` followed by the
  unpadded URL-safe Base64 ([RFC 4648 §5], no `=`) of the **DEFLATE**
  ([RFC 1951]) compression of the UTF-8 JSON bytes.

A reader **MUST** accept both. It detects the compact form by the `MS1:` prefix
(after trimming surrounding whitespace); otherwise it parses JSON. A writer
**MAY** emit either; `.msr` files **SHOULD** use the compact form.

## 9. Versioning & forward compatibility (normative)

- The `version` field denotes the format version (`major.minor`); this document
  specifies `"0.1"`. Each version is published at its own stable URL
  (`…/spec/<version>/`).
- Readers **MUST** ignore unknown object fields (forward compatibility).
- A later **minor** revision (same major) **MUST NOT** repurpose an existing field
  nor change an encoding; it **MAY** add optional fields. Incompatible changes
  bump the **major** version and the `MS1` envelope tag (`MS2:`…).

## 10. Migration from Pentasol (informative)

Pentasol encodes 5T/5D moves as `(col,row)dir centerdist` lines, where
`centerdist = pos − ⌊n/2⌋`. A bridge that maps Pentasol moves to §6.1 moves (and
back) is provided by the project's `morpion-solitaire` tool, so existing Pentasol
corpora convert losslessly into MSR (gaining metadata and the other two variants).

## 11. Examples

Minimal JSON record (empty game):

```json
{ "version": "0.1", "variant": "5T", "score": 0, "moves": [] }
```

A one-move record, compact form:

```
MS1:<base64url of deflate of the JSON>
```

## 12. Conformance test vectors

The reference implementation ships test vectors, including a real 178-move 5T
world-record file that **MUST** decode and validate. Implementations **SHOULD**
check against the same corpus.

## References

- [RFC 2119] Key words for use in RFCs.
- [RFC 1951] DEFLATE Compressed Data Format.
- [RFC 4648] The Base16, Base32, and Base64 Data Encodings.
- Project bibliography: `docs/BIBLIOGRAPHY.md`.

[Morpion Solitaire]: https://en.wikipedia.org/wiki/Join_Five
[RFC 2119]: https://www.rfc-editor.org/rfc/rfc2119
[RFC 1951]: https://www.rfc-editor.org/rfc/rfc1951
[RFC 4648]: https://www.rfc-editor.org/rfc/rfc4648
[RFC 4648 §5]: https://www.rfc-editor.org/rfc/rfc4648#section-5
