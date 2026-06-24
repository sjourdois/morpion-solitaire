/// D4 symmetry group for the Morpion Solitaire board.
///
/// The initial cross occupies a (2n)×(2n) grid at positive coordinates 0..=2n−1.
/// Its symmetry centre is at (k/2, k/2) where k = 2n−1 (a half-integer centre).
///   5T/5D: k = 9  (centre at 4.5, 4.5)
///   4T/4D: k = 7  (centre at 3.5, 3.5)
///
/// Symmetry is handled **structurally** by the search, not via a hash table:
/// the cross is fixed by all 8 transforms (stabiliser = D4), and at each node we
/// explore only one representative per orbit of the current stabiliser. The
/// stabiliser shrinks as moves are played and is trivial after the first generic
/// move, so the filtering only bites at the first move or two.
use crate::game::board::{Board, Pos};
use crate::game::line::{Dir, Line};
use crate::game::moves::Move;

/// Apply one of the 8 D4 symmetry transforms.
/// `k` = 2*line_len − 1 (e.g. 9 for 5T/5D, 7 for 4T/4D).
pub fn apply_transform(t: usize, (x, y): Pos, k: i16) -> Pos {
    match t {
        0 => (x, y),         // identity
        1 => (k - y, x),     // rot 90° CCW
        2 => (k - x, k - y), // rot 180°
        3 => (y, k - x),     // rot 270° CCW
        4 => (x, k - y),     // reflect about horizontal midline
        5 => (k - x, y),     // reflect about vertical midline
        6 => (y, x),         // transpose
        7 => (k - y, k - x), // anti-transpose
        _ => unreachable!(),
    }
}

/// The **linear part** of [`apply_transform`] (translation dropped): the D4 action on
/// a *displacement* (a vector/offset, anchor-fixed), i.e. `apply_transform(t, p, 0)`.
/// Used to transform motif moves relative to their anchor (`search::macros`).
#[inline]
pub fn lin_transform(t: usize, p: Pos) -> Pos {
    apply_transform(t, p, 0)
}

/// Deterministic per-position Zobrist value (splitmix64 finalizer on packed coords).
pub fn zobrist_value((x, y): Pos) -> u64 {
    let mut h = (x as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (y as i64 as u64).wrapping_mul(0x6c62272e07bb0142);
    h ^= h >> 30;
    h = h.wrapping_mul(0xbf58476d1ce4e5b9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94d049bb133111eb);
    h ^= h >> 31;
    h
}

/// Whether the canonical **position** hash is mixed into a move's policy code.
/// Default on (`NRPA_POS_CODE != "0"`) reproduces the current position-specific
/// coding. Off ⇒ position-independent (Rosin-style) move coding: the policy
/// learns global move preferences that generalize across *all* positions, not
/// only symmetric ones. Read once; experimental knob for NRPA coding studies.
///
/// Finding (5T, level 3, 4×20 s): the two codings are within NRPA's run-to-run
/// variance (position-specific ~92 avg / 94 max, move-only ~93 / 95) — no clear
/// short-budget win, both at the ~90–95 cold ceiling. Kept OFF-equivalent by
/// default; the generalization payoff, if any, would need long (multi-hour,
/// deeper-level) runs to show, so the knob stays for that study.
pub fn position_in_code() -> bool {
    use std::sync::OnceLock;
    static P: OnceLock<bool> = OnceLock::new();
    *P.get_or_init(|| {
        std::env::var("NRPA_POS_CODE")
            .map(|v| v != "0")
            .unwrap_or(true)
    })
}

/// Ring of 8 neighbours, clockwise from north (used by [`local_code`]).
const RING: [(i16, i16); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

/// Permutation of the 8 ring indices under each D4 transform's linear part, so a
/// neighbourhood pattern can be folded to a canonical (symmetry-invariant) form.
fn ring_perms() -> &'static [[u8; 8]; 8] {
    use std::sync::OnceLock;
    static P: OnceLock<[[u8; 8]; 8]> = OnceLock::new();
    P.get_or_init(|| {
        // Linear part of `apply_transform` (translation dropped): acts on offsets.
        let lin = |t: usize, (x, y): (i16, i16)| -> (i16, i16) {
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
        };
        let mut perms = [[0u8; 8]; 8];
        for (t, perm) in perms.iter_mut().enumerate() {
            for (j, slot) in perm.iter_mut().enumerate() {
                let img = lin(t, RING[j]);
                *slot = RING.iter().position(|&o| o == img).unwrap() as u8;
            }
        }
        perms
    })
}

/// Symmetry-invariant hash of the local 8-neighbour occupancy around `pos`: the
/// ring cells' occupancy as a byte, folded to the minimum over its 8 D4 images,
/// then mixed. Used as an optional **local-context** term in the NRPA move code
/// (`NRPA_LOCAL=1`) so the policy can generalize across positions that share a
/// local pattern — between the position-specific and the move-only codings.
pub fn local_code(board: &Board, pos: Pos) -> u64 {
    let mut bits = 0u8;
    for (j, &(dx, dy)) in RING.iter().enumerate() {
        if board.contains((pos.0 + dx, pos.1 + dy)) {
            bits |= 1 << j;
        }
    }
    let mut canon = bits;
    for perm in ring_perms() {
        let mut b = 0u8;
        for (j, &p) in perm.iter().enumerate() {
            if bits & (1 << j) != 0 {
                b |= 1 << p;
            }
        }
        canon = canon.min(b);
    }
    let mut h = (canon as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0xa5a5_a5a5_a5a5_a5a5;
    h ^= h >> 29;
    h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h ^= h >> 27;
    h
}

/// How each D4 transform maps a line direction. A transform acts on the
/// direction's (undirected) orientation: 90°/270° rotations and the transpose
/// reflections swap H↔V; the diagonal reflections (and 90°/270°) swap DP↔DN.
/// Derived from the linear part of `apply_transform` acting on `dir.delta()`.
pub fn transform_dir(t: usize, dir: Dir) -> Dir {
    use Dir::*;
    match t {
        0 | 2 => dir, // identity, rot180
        1 | 3 => match dir {
            H => V,
            V => H,
            DP => DN,
            DN => DP,
        }, // rot90, rot270
        4 | 5 => match dir {
            H => H,
            V => V,
            DP => DN,
            DN => DP,
        }, // axis reflections
        6 | 7 => match dir {
            H => V,
            V => H,
            DP => DP,
            DN => DN,
        }, // (anti)transpose
        _ => unreachable!(),
    }
}

/// Canonical (flip-independent) endpoint of a line: the lexicographically
/// smaller of its two ends. `n` is the line length.
#[cfg(test)]
fn line_min_endpoint(line: &Line, n: i16) -> Pos {
    let (dx, dy) = line.dir.delta();
    let e0 = line.origin;
    let e1 = (e0.0 + (n - 1) * dx, e0.1 + (n - 1) * dy);
    e0.min(e1)
}

/// The line obtained by applying transform `t` to `line`, in canonical form
/// (origin = smaller endpoint, transformed direction). `k = 2·n − 1`.
pub fn transform_line(t: usize, line: &Line, k: i16, n: i16) -> Line {
    let (dx, dy) = line.dir.delta();
    let e0 = line.origin;
    let e1 = (e0.0 + (n - 1) * dx, e0.1 + (n - 1) * dy);
    let te0 = apply_transform(t, e0, k);
    let te1 = apply_transform(t, e1, k);
    Line::new(te0.min(te1), transform_dir(t, line.dir))
}

/// A move's symmetry identity under transform `t`: the transformed point, the
/// transformed line's canonical endpoint, and the transformed direction. Two
/// moves are in the same orbit iff some transform maps one key to the other.
/// `k = 2·n − 1`.
pub fn transformed_move_key(t: usize, m: &Move, k: i16, n: i16) -> (Pos, Pos, u8) {
    let (dx, dy) = m.line.dir.delta();
    let e0 = m.line.origin;
    let e1 = (e0.0 + (n - 1) * dx, e0.1 + (n - 1) * dy);
    let tp = apply_transform(t, m.pos, k);
    let te0 = apply_transform(t, e0, k);
    let te1 = apply_transform(t, e1, k);
    (tp, te0.min(te1), transform_dir(t, m.line.dir) as u8)
}

// `stab` is a bitmask over the 8 transforms (bit `t` set = transform `t` fixes
// the current position). `0b1111_1111` is the full D4 group (the cross),
// `0b0000_0001` is identity-only (no symmetry left).

/// Keep `m` iff it is the orbit representative under `stab`, i.e. no transform
/// in `stab` maps it to a lexicographically smaller move. Always true once the
/// stabiliser is identity-only.
pub fn is_orbit_min(m: &Move, stab: u8, k: i16, n: i16) -> bool {
    let key0 = transformed_move_key(0, m, k, n);
    for t in 1..8 {
        if stab & (1 << t) != 0 && transformed_move_key(t, m, k, n) < key0 {
            return false;
        }
    }
    true
}

/// The stabiliser of the position *after* playing `m`, given the parent
/// stabiliser: the transforms that already fixed the position and also fix `m`
/// (same point and same line). It can only shrink, and is identity-only once
/// the position becomes asymmetric.
pub fn stab_after(stab: u8, m: &Move, k: i16, n: i16) -> u8 {
    // Overwhelmingly common case: the position is already asymmetric (identity
    // only), so the result is identity only — skip the two key computations.
    if stab == 0b0000_0001 {
        return 0b0000_0001;
    }
    let key0 = transformed_move_key(0, m, k, n);
    let mut out = 0u8;
    for t in 0..8 {
        if stab & (1 << t) != 0 && transformed_move_key(t, m, k, n) == key0 {
            out |= 1 << t;
        }
    }
    out
}

/// Eight parallel Zobrist hashes (one per D4 transform); `canonical()` is the
/// minimum — a board fingerprint invariant under the 8 symmetries. Still used
/// by the NRPA search (`nrpa.rs`); the systematic search now handles symmetry
/// structurally and uses a single hash instead.
#[derive(Debug, Clone)]
pub struct SymmetryHashes {
    hashes: [u64; 8],
    k: i16,
}

impl SymmetryHashes {
    /// `k` = 2*line_len − 1  (9 for 5T/5D, 7 for 4T/4D).
    pub fn new(k: i16) -> Self {
        Self {
            hashes: [0u64; 8],
            k,
        }
    }

    /// Toggle a cell into/out of all 8 hashes (XOR is its own inverse).
    pub fn toggle(&mut self, pos: Pos) {
        for t in 0..8 {
            self.hashes[t] ^= zobrist_value(apply_transform(t, pos, self.k));
        }
    }

    /// Toggle only the identity hash (`hashes[0]`) — for the `NRPA_SYM=0` path,
    /// which codes in the identity frame ([`move_coder_id`](Self::move_coder_id))
    /// and so needs no other transform's hash. 8× cheaper than [`toggle`](Self::toggle)
    /// in the hot loop; the other 7 hashes go stale but are never read in that mode.
    #[inline]
    pub fn toggle_identity(&mut self, pos: Pos) {
        self.hashes[0] ^= zobrist_value(pos);
    }

    /// The canonical hash: minimum of all 8 transform hashes.
    pub fn canonical(&self) -> u64 {
        self.hashes.iter().copied().min().unwrap()
    }

    /// Build a [`MoveCoder`] for the current position, capturing the canonical
    /// state hash and the transforms that achieve it. Coding many moves at one
    /// position (the softmax / adapt loops) then computes `canonical()` once
    /// instead of once per move.
    pub fn move_coder(&self) -> MoveCoder {
        let cmin = self.canonical();
        let mut active = [0u8; 8];
        let mut len = 0usize;
        for t in 0..8 {
            if self.hashes[t] == cmin {
                active[len] = t as u8;
                len += 1;
            }
        }
        MoveCoder {
            cmin,
            k: self.k,
            n: (self.k + 1) / 2,
            active,
            len: len as u8,
            pos_in_code: position_in_code(),
        }
    }

    /// Build a [`MoveCoder`] for the **identity frame only** — the `--no-symmetry`
    /// path. It codes against `hashes[0]` (maintained by [`toggle_identity`]) with the
    /// identity transform, ignoring the other 7 hashes (which `toggle_identity` leaves
    /// stale). No symmetry folding: a weight learnt in one orientation does NOT transfer
    /// to its images. Must be used instead of [`move_coder`](Self::move_coder) whenever
    /// symmetry is off, because `move_coder` mins over all 8 (then-stale) hashes.
    pub fn move_coder_id(&self) -> MoveCoder {
        MoveCoder {
            cmin: self.hashes[0],
            k: self.k,
            n: (self.k + 1) / 2,
            active: [0u8; 8], // transform 0 = identity
            len: 1,
            pos_in_code: position_in_code(),
        }
    }
}

/// Symmetry-invariant move coder bound to one position (built by
/// [`SymmetryHashes::move_coder`]). For indexing an NRPA policy: symmetric
/// (position, move) pairs yield the same code, so a weight learnt in one
/// orientation applies to all eight. Caches the position-invariant context
/// (canonical hash + the transforms achieving it — usually a single transform
/// for an asymmetric position) so [`MoveCoder::code`] costs about a plain move
/// hash.
pub struct MoveCoder {
    cmin: u64,
    k: i16,
    n: i16,
    active: [u8; 8],
    len: u8,
    pos_in_code: bool,
}

impl MoveCoder {
    /// Code for `(this position, mv)`: the canonical state hash mixed with the
    /// move coded in the canonical frame — the minimum move code over the
    /// transforms that achieve the state's canonical hash.
    #[inline]
    pub fn code(&self, mv: &Move) -> u64 {
        let move_code = self.active[..self.len as usize]
            .iter()
            .map(|&t| {
                let (p, e, d) = transformed_move_key(t as usize, mv, self.k, self.n);
                zobrist_value(p)
                    ^ zobrist_value(e).wrapping_mul(0x517cc1b727220a95)
                    ^ (d as u64).wrapping_mul(0x6c62272e07bb0142)
            })
            .min()
            .unwrap();
        // Position-specific (default) mixes the canonical state hash in; the
        // experimental position-independent mode keys the policy on the move
        // alone (Rosin-style), trading precision for cross-position generalization.
        if self.pos_in_code {
            self.cmin.wrapping_mul(0x9e3779b97f4a7c15) ^ move_code
        } else {
            move_code
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{moves::legal_moves, rules::Variant, state::GameState};

    /// Point-stabiliser of a board: transforms mapping occupied cells to
    /// occupied cells (the cross has no lines, so this is its full stabiliser).
    fn cross_stab(state: &GameState, k: i16) -> u8 {
        let mut stab = 0u8;
        for t in 0..8 {
            if state
                .board
                .cells
                .iter()
                .all(|&c| state.board.contains(apply_transform(t, c, k)))
            {
                stab |= 1 << t;
            }
        }
        stab
    }

    #[test]
    fn identity_key_is_the_natural_move() {
        let state = GameState::new(Variant::T5);
        let m = legal_moves(&state)[0];
        let (k, n) = (9, 5);
        let (p, end, d) = transformed_move_key(0, &m, k, n);
        assert_eq!(p, m.pos);
        assert_eq!(end, line_min_endpoint(&m.line, n));
        assert_eq!(d, m.line.dir as u8);
    }

    /// Regression: the `--no-symmetry` path maintains only `hashes[0]` (via
    /// `toggle_identity`); the other 7 go stale. `move_coder_id` must code from
    /// `hashes[0]` + the identity transform alone, so two states with the same identity
    /// hash but different stale hashes yield the SAME code. The bug was the no-symmetry
    /// loops using `move_coder`, which mins over all 8 hashes and so leaked the stale
    /// ones into the policy code.
    #[test]
    fn move_coder_id_ignores_stale_hashes() {
        let state = GameState::new(Variant::T5);
        let m = legal_moves(&state)[0];
        let mut a = SymmetryHashes::new(9);
        let mut b = SymmetryHashes::new(9);
        a.toggle_identity((1, 2));
        b.toggle_identity((1, 2)); // identical hashes[0]
        b.hashes[3] ^= 0xdead_beef_u64; // a stale hash differs (never read in identity mode)
        assert_eq!(
            a.move_coder_id().code(&m),
            b.move_coder_id().code(&m),
            "identity-frame code must not depend on the stale hashes"
        );
    }

    #[test]
    fn t5_cross_is_full_d4() {
        let state = GameState::new(Variant::T5);
        assert_eq!(cross_stab(&state, 9), 0xFF, "5T cross must be full D4");
    }

    #[test]
    fn orbit_reduction_partitions_first_moves() {
        use std::collections::HashSet;
        for (variant, k, n) in [
            (Variant::T5, 9, 5),
            (Variant::D5, 9, 5),
            (Variant::D4, 7, 4),
        ] {
            let state = GameState::new(variant);
            let stab = cross_stab(&state, k);
            let moves = legal_moves(&state);
            let kept: Vec<&Move> = moves
                .iter()
                .filter(|m| is_orbit_min(m, stab, k, n))
                .collect();
            let kept_keys: HashSet<_> = kept
                .iter()
                .map(|m| transformed_move_key(0, m, k, n))
                .collect();

            // Every move's orbit minimum (under the real stabiliser) is kept.
            for m in &moves {
                let minkey = (0..8)
                    .filter(|&t| stab & (1 << t) != 0)
                    .map(|t| transformed_move_key(t, m, k, n))
                    .min()
                    .unwrap();
                assert!(
                    kept_keys.contains(&minkey),
                    "{variant:?}: orbit min not kept"
                );
            }
        }
    }

    #[test]
    fn first_move_shrinks_the_stabiliser() {
        // A generic first move breaks (some of) the cross's symmetry.
        let state = GameState::new(Variant::T5);
        let (k, n) = (9, 5);
        let stab = cross_stab(&state, k);
        let m = *legal_moves(&state)
            .iter()
            .find(|m| is_orbit_min(m, stab, k, n))
            .unwrap();
        let child = stab_after(stab, &m, k, n);
        assert_eq!(child & 1, 1, "identity always fixes a move");
        assert!(child < stab, "a first move must break some symmetry");
    }
}
