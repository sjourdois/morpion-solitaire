//! Feature encoding of a `(state, move)` pair for the neural move prior.
//!
//! The network that supplies NRPA's GNRPA bias `β(state, move)` (see
//! `docs/neural-guide.md`) consumes a **fixed-length** description of the *local
//! geometry* around a candidate move — the generalization of
//! `symmetry::local_code`'s 8-neighbour ring to a square patch — so a prior learned
//! at one location transfers to every position sharing the local pattern (like
//! `corpus_prior`, of which this is the parametric superset).
//!
//! Two binary planes over a `(2R+1)×(2R+1)` patch centred on the move's point —
//! **occupancy** (which patch cells are already filled) and **line** (which patch
//! cells belong to the line this move draws) — plus two scalars (game progress,
//! local density).
//!
//! **D4 symmetry — two modes, selectable by [`canonicalize`] (`NRPA_NEURAL_CANON`).**
//! By default (off) the runtime [`encode`] emits a single (natural) orientation —
//! cheap — and the net is trained on all eight [`encode_orientation`] images so it
//! learns approximate invariance. With the knob on, the encoder instead folds each
//! patch to a canonical orientation at runtime (exact invariance, no augmentation
//! needed) — but that 8× fold was measured to dominate the search (~50× slowdown).
//! Both are kept so the trade-off (guidance quality vs throughput) can be measured.
//!
//! `R = 4` (a 9×9 patch) fully contains any length-5 line through the centre
//! (a line reaches 4 cells from the placed point, diagonally to `(±4, ±4)`), plus
//! its immediate surroundings. `R` is a knob: a wider patch sees more context at
//! the cost of width — to be swept once the prior is wired and measurable.

use crate::game::board::{GRID, OFFSET};
use crate::game::moves::Move;
use crate::game::state::GameState;

/// Patch radius: the patch spans offsets `-R..=R` on each axis.
pub const R: i16 = 4;
/// Patch side length (`2R + 1`).
pub const SIDE: usize = (2 * R + 1) as usize;
/// Cells per plane (`SIDE * SIDE`). Must be ≤ 128 so a plane packs into a `u128`
/// for the canonicalisation key (81 ≤ 128 holds for `R ≤ 5`).
pub const CELLS: usize = SIDE * SIDE;
/// Total feature-vector length: two planes plus two scalars.
pub const FEATURE_LEN: usize = CELLS * 2 + 2;

/// Bit/array index of patch offset `(dx, dy)`, both in `-R..=R`.
#[inline]
fn index(dx: i16, dy: i16) -> usize {
    (dy + R) as usize * SIDE + (dx + R) as usize
}

/// Linear part of the 8 D4 transforms acting on a patch *offset* (translation
/// dropped — the move's point is the patch centre and is fixed). Mirrors the `lin`
/// closure in `symmetry::ring_perms`, so this canonicalisation is the same D4
/// action the policy coding uses. Each transform permutes `-R..=R` onto itself.
#[inline]
fn lin(t: usize, (x, y): (i16, i16)) -> (i16, i16) {
    match t {
        0 => (x, y),
        1 => (-y, x),
        2 => (-x, -y),
        3 => (y, -x),
        4 => (x, -y),
        5 => (-x, y),
        6 => (y, x),
        _ => (-y, -x),
    }
}

/// Apply transform `t` to a patch plane: `out[t·offset] = plane[offset]`.
fn transform_plane(t: usize, plane: &[bool; CELLS]) -> [bool; CELLS] {
    let mut out = [false; CELLS];
    for dy in -R..=R {
        for dx in -R..=R {
            if plane[index(dx, dy)] {
                let (tx, ty) = lin(t, (dx, dy));
                out[index(tx, ty)] = true;
            }
        }
    }
    out
}

/// Pack a plane into a `u128` (bit `i` = cell `i`), for lexicographic comparison.
fn pack(plane: &[bool; CELLS]) -> u128 {
    let mut bits = 0u128;
    for (i, &b) in plane.iter().enumerate() {
        if b {
            bits |= 1u128 << i;
        }
    }
    bits
}

/// Encoding mode. When **on**, the runtime encoder folds each patch to a canonical
/// D4 orientation ([`canonical_transform`]) so symmetric moves encode identically —
/// exact invariance, but an 8× per-move cost measured to dominate the search
/// (~50× slowdown). When **off** (the only mode here), the encoder emits the natural
/// orientation (cheap) and invariance is instead taught by training on
/// [`encode_orientation`] augmentations.
///
/// Always off: the canonical-fold mode was never beneficial (the slowdown swamps any
/// guidance gain), so it isn't exposed as a tunable (no env var — see the project's
/// CLI/GUI-only-knobs rule). The fold machinery is kept (and unit-tested) so the mode
/// could be promoted to an [`OptionSpec`](crate::search::plugin::OptionSpec) if ever
/// worth re-measuring.
pub fn canonicalize() -> bool {
    false
}

/// The canonical D4 transform for a `(occupancy, line)` patch: the one minimising
/// the packed `(occupancy, line)` key. Deterministic (first minimiser on ties), so
/// all eight images of a patch reduce to the same orientation. Used only when
/// [`canonicalize`] is on.
fn canonical_transform(occ: &[bool; CELLS], line: &[bool; CELLS]) -> usize {
    let mut best_t = 0usize;
    let mut best_key = (pack(occ), pack(line));
    for t in 1..8 {
        let key = (
            pack(&transform_plane(t, occ)),
            pack(&transform_plane(t, line)),
        );
        if key < best_key {
            best_key = key;
            best_t = t;
        }
    }
    best_t
}

/// Build the move's two raw patch planes (natural board orientation) and the count
/// of occupied patch cells. This is the per-move work the hot loop pays; it is kept
/// to a single orientation — D4 invariance is taught to the net by **training-time
/// augmentation** (see [`encode_orientation`]) rather than by an 8× runtime fold,
/// which was measured to dominate the search cost (~50× slowdown).
fn raw_planes(state: &GameState, mv: &Move) -> ([bool; CELLS], [bool; CELLS], u32) {
    let (cx, cy) = mv.pos;
    let line_len = state.variant.len();
    // The move's line cells (the placed point plus the four occupied ones).
    let line_cells: Vec<(i16, i16)> = mv.line.positions(line_len).collect();

    let mut occ = [false; CELLS];
    let mut line = [false; CELLS];
    let mut occ_count = 0u32;
    for dy in -R..=R {
        for dx in -R..=R {
            let p = (cx + dx, cy + dy);
            let i = index(dx, dy);
            if on_grid(p) && state.board.contains(p) {
                occ[i] = true;
                occ_count += 1;
            }
            if line_cells.contains(&p) {
                line[i] = true;
            }
        }
    }
    (occ, line, occ_count)
}

/// Assemble the feature vector from two patch planes: occupancy, line, then the two
/// scalars (game progress toward the 5T record, local density) — both ~[0, 1] so
/// they sit on the same scale as the binary planes.
fn features_from_planes(
    occ: &[bool; CELLS],
    line: &[bool; CELLS],
    occ_count: u32,
    hist_len: usize,
) -> Vec<f32> {
    let mut feats = Vec::with_capacity(FEATURE_LEN);
    feats.extend(occ.iter().map(|&b| b as u8 as f32));
    feats.extend(line.iter().map(|&b| b as u8 as f32));
    feats.push(hist_len as f32 / 178.0);
    feats.push(occ_count as f32 / CELLS as f32);
    feats
}

/// Is grid cell at internal position `p` on the board (in `[0, GRID)` both axes)?
/// `Board::contains` indexes the grid without bounds-checking, so a patch cell that
/// falls off the fixed grid must be treated as empty here, not queried.
#[inline]
fn on_grid(p: (i16, i16)) -> bool {
    let gx = p.0 + OFFSET;
    let gy = p.1 + OFFSET;
    (0..GRID).contains(&gx) && (0..GRID).contains(&gy)
}

/// Encode `(state, mv)` into a fixed-length feature vector of [`FEATURE_LEN`]
/// `f32`s in the **natural board orientation**: occupancy plane, line plane, then
/// `[game progress, local density]`. Cheap (single orientation); D4 invariance
/// comes from training on [`encode_orientation`] augmentations.
pub fn encode(state: &GameState, mv: &Move) -> Vec<f32> {
    encode_keyed(state, mv).1
}

/// A compact key identifying a move's *local pattern*: the packed (occupancy, line)
/// planes in natural orientation. Two `(state, move)` pairs with the same key have
/// the same features (scalars aside) and so the same neural prior — which lets a
/// search **cache** the prior per local pattern instead of re-running the net for
/// every node (lazy NN→table distillation; see `docs/neural-guide.md`). Symmetric
/// patterns get *different* keys here (no canonical fold), but the net returns ~the
/// same β for them, so the only cost is a slightly lower cache hit rate.
pub type PatchKey = (u128, u128);

/// Like [`encode`] but also returns the move's [`PatchKey`]. Folds to the canonical
/// orientation iff [`canonicalize`] is on (else natural orientation, the default).
pub fn encode_keyed(state: &GameState, mv: &Move) -> (PatchKey, Vec<f32>) {
    let (occ, line, occ_count) = raw_planes(state, mv);
    let (occ, line) = if canonicalize() {
        let t = canonical_transform(&occ, &line);
        (transform_plane(t, &occ), transform_plane(t, &line))
    } else {
        (occ, line)
    };
    let key = (pack(&occ), pack(&line));
    let feats = features_from_planes(&occ, &line, occ_count, state.history.len());
    (key, feats)
}

/// Encode `(state, mv)` under D4 transform `t` (0 = natural). Used only for
/// **training-time augmentation**: feeding all 8 orientations of each example
/// teaches the net the invariance that the runtime encoder no longer enforces by
/// canonicalisation. Offline, so its 8× cost does not touch the hot loop.
pub fn encode_orientation(state: &GameState, mv: &Move, t: usize) -> Vec<f32> {
    let (occ, line, occ_count) = raw_planes(state, mv);
    let occ_t = transform_plane(t, &occ);
    let line_t = transform_plane(t, &line);
    features_from_planes(&occ_t, &line_t, occ_count, state.history.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::moves::legal_moves;
    use crate::game::rules::Variant;

    /// The canonicalisation is D4-invariant: every one of the 8 transforms of a
    /// patch reduces to the same canonical orientation (same packed key). This is
    /// the property the neural prior relies on for symmetry generalization.
    #[test]
    fn canonicalisation_is_d4_invariant() {
        // An asymmetric patch so the 8 images are genuinely distinct before folding.
        let mut occ = [false; CELLS];
        let mut line = [false; CELLS];
        for &(dx, dy) in &[(0, 0), (1, 0), (2, 0), (-1, 1), (2, -2), (3, 1)] {
            occ[index(dx, dy)] = true;
        }
        for &(dx, dy) in &[(0, 0), (1, 0), (2, 0), (-1, 0), (-2, 0)] {
            line[index(dx, dy)] = true;
        }
        let canon = |o: &[bool; CELLS], l: &[bool; CELLS]| {
            let t = canonical_transform(o, l);
            (pack(&transform_plane(t, o)), pack(&transform_plane(t, l)))
        };
        let reference = canon(&occ, &line);
        for t in 0..8 {
            let (o, l) = (transform_plane(t, &occ), transform_plane(t, &line));
            assert_eq!(canon(&o, &l), reference, "transform {t} folds elsewhere");
        }
    }

    /// `lin` permutes the patch: each transform maps `-R..=R` onto itself bijectively.
    #[test]
    fn transforms_permute_the_patch() {
        for t in 0..8 {
            let mut seen = [false; CELLS];
            for dy in -R..=R {
                for dx in -R..=R {
                    let (tx, ty) = lin(t, (dx, dy));
                    assert!((-R..=R).contains(&tx) && (-R..=R).contains(&ty));
                    let i = index(tx, ty);
                    assert!(!seen[i], "transform {t} is not injective");
                    seen[i] = true;
                }
            }
        }
    }

    /// Augmentation: `encode_orientation(_, _, 0)` is the natural `encode` (default
    /// mode), and the centre cell is fixed under every transform (so all 8
    /// orientations agree there), while a generic patch differs across orientations.
    #[test]
    fn orientation_augmentation() {
        let mut st = GameState::new(Variant::T5);
        // Advance to an asymmetric position so orientations are genuinely distinct.
        for _ in 0..6 {
            let ms = legal_moves(&st);
            st.apply(ms[0]);
        }
        let mv = legal_moves(&st)[0];
        assert_eq!(encode(&st, &mv), encode_orientation(&st, &mv, 0));
        let centre = index(0, 0);
        let variants: Vec<Vec<f32>> = (0..8).map(|t| encode_orientation(&st, &mv, t)).collect();
        for v in &variants {
            assert_eq!(v.len(), FEATURE_LEN);
            assert_eq!(
                v[CELLS + centre],
                1.0,
                "centre stays on the line in every image"
            );
        }
        assert!(
            variants.iter().any(|v| v != &variants[0]),
            "a generic patch should differ across orientations"
        );
    }

    /// A real encoding has the right length and the expected centre values: the
    /// placed point is empty in the occupancy plane but set in the line plane.
    #[test]
    fn encode_shape_and_centre() {
        let st = GameState::new(Variant::T5);
        let mv = legal_moves(&st)[0];
        let f = encode(&st, &mv);
        assert_eq!(f.len(), FEATURE_LEN);
        let centre = index(0, 0);
        assert_eq!(f[centre], 0.0, "placed point must be empty in occupancy");
        assert_eq!(
            f[CELLS + centre],
            1.0,
            "placed point must be on its own line"
        );
    }

    /// The encoding is deterministic, and distinct moves at the same point (a
    /// crossing where several lines complete) get distinct features — the prior
    /// must be able to tell them apart.
    #[test]
    fn distinct_moves_encode_distinctly() {
        // Find a state with two legal moves sharing a point but on different lines.
        let mut st = GameState::new(Variant::T5);
        for _ in 0..20 {
            let ms = legal_moves(&st);
            if let Some((a, b)) = ms
                .iter()
                .enumerate()
                .find_map(|(i, m)| ms[i + 1..].iter().find(|n| n.pos == m.pos).map(|n| (m, n)))
            {
                assert_eq!(encode(&st, a), encode(&st, a), "encoding is deterministic");
                assert_ne!(encode(&st, a), encode(&st, b), "same point, different line");
                return;
            }
            st.apply(ms[0]);
        }
        // No shared-point pair arose in this short rollout; determinism still holds.
        let ms = legal_moves(&st);
        assert_eq!(encode(&st, &ms[0]), encode(&st, &ms[0]));
    }
}
