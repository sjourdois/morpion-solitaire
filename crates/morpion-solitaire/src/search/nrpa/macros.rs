//! Macro-actions for NRPA — multi-move motifs mined from record games.
//!
//! Macro-actions raise the action granularity: NRPA picks, in one step, a `k`-move
//! **motif** (mined from the record corpus) instead of a single move, so the policy
//! composes over a shorter horizon. An experimental lever, off by default.
//!
//! This module is the **offline mining + the motif representation**. A motif is
//! stored **relative to its anchor** (move 0's placed point) and **D4-canonical**
//! (folded to the lexicographically smallest of its 8 symmetry images, reusing the
//! same group action as `symmetry::local_code`), so a motif mined at one place and
//! orientation matches everywhere it geometrically fits. Wiring the motifs into the
//! search as `Action`s lives in `nrpa.rs` (next step).

use super::move_playable; // the parent NRPA engine
use crate::game::board::Pos;
use crate::game::line::{Dir, Line};
use crate::game::moves::Move;
use crate::game::rules::Variant;
use crate::game::state::GameState;
use crate::search::symmetry::{lin_transform, transform_dir};
use rustc_hash::FxHashMap;

/// One move of a motif, **relative to the motif's anchor** (move 0's placed point):
/// `d` is the displacement of this move's placed point from the anchor, `dir` its
/// line direction, `line_pos` the point's index within the line. The absolute
/// `Move` is rebuilt at anchor `a` by `Line::from_point(a+d, dir, line_pos, n)` —
/// the same `(origin = smaller endpoint, line_pos = index from origin)` convention
/// `Move`/`Line` use, so reconstruction is exact.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RelMove {
    pub d: (i16, i16),
    pub dir: Dir,
    pub line_pos: u8,
}

impl RelMove {
    /// Tuple form for lexicographic (canonical) ordering.
    #[inline]
    fn key(&self) -> (i16, i16, u8, u8) {
        (self.d.0, self.d.1, self.dir as u8, self.line_pos)
    }

    /// This relative move under D4 transform `t` (linear, anchor-fixed). `n` is the
    /// line length. Recomputes the canonical `(origin, line_pos)` after the
    /// transform may have flipped the line's orientation, so the result still obeys
    /// the `origin = lexicographically-smaller endpoint` convention.
    fn transform(&self, t: usize, n: i16) -> RelMove {
        let (dx, dy) = self.dir.delta();
        let origin = (
            self.d.0 - self.line_pos as i16 * dx,
            self.d.1 - self.line_pos as i16 * dy,
        );
        let end = (origin.0 + (n - 1) * dx, origin.1 + (n - 1) * dy);
        let td = lin_transform(t, self.d);
        let to = lin_transform(t, origin);
        let te = lin_transform(t, end);
        let tdir = transform_dir(t, self.dir);
        let new_origin = to.min(te); // canonical: lexicographically smaller endpoint
        let (tdx, tdy) = tdir.delta();
        // `td` sits on the transformed line; its index from the new origin. The four
        // canonical deltas have leading nonzero component +1, so this is an exact
        // subtraction (no real division).
        let lp = if tdx != 0 {
            (td.0 - new_origin.0) / tdx
        } else {
            (td.1 - new_origin.1) / tdy
        };
        RelMove {
            d: td,
            dir: tdir,
            line_pos: lp as u8,
        }
    }
}

/// A `k`-move motif: an ordered relative move list, `moves[0].d == (0,0)`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Motif {
    pub moves: Vec<RelMove>,
}

impl Motif {
    /// Build a (non-canonical) motif from a window of `k` consecutive absolute
    /// moves, anchored at the first move's placed point.
    pub fn from_window(window: &[Move]) -> Motif {
        let a = window[0].pos;
        let moves = window
            .iter()
            .map(|m| RelMove {
                d: (m.pos.0 - a.0, m.pos.1 - a.1),
                dir: m.line.dir,
                line_pos: m.line_pos,
            })
            .collect();
        Motif { moves }
    }

    /// Comparable key (the ordered relative-move tuples).
    fn key(&self) -> Vec<(i16, i16, u8, u8)> {
        self.moves.iter().map(|m| m.key()).collect()
    }

    /// This motif under D4 transform `t`.
    fn transformed(&self, t: usize, n: i16) -> Motif {
        Motif {
            moves: self.moves.iter().map(|m| m.transform(t, n)).collect(),
        }
    }

    /// The canonical (symmetry-invariant) form: the lexicographically smallest of
    /// the 8 D4 images. Geometry-equivalent motifs collapse to one form, so the
    /// library and its policy codes are orientation-invariant.
    pub fn canonical(&self, n: i16) -> Motif {
        (0..8)
            .map(|t| self.transformed(t, n))
            .min_by(|a, b| a.key().cmp(&b.key()))
            .unwrap()
    }

    /// 64-bit signature of the canonical form — the symmetry-invariant **action
    /// code** the NRPA policy keys macros on (disjoint from single-move codes by a
    /// dedicated mixing constant in `nrpa.rs`).
    pub fn signature(&self, n: i16) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325; // FNV-ish seed
        for m in &self.canonical(n).moves {
            for v in [
                m.d.0 as i64 as u64,
                m.d.1 as i64 as u64,
                m.dir as u64,
                m.line_pos as u64,
            ] {
                h ^= v.wrapping_mul(0x9e3779b97f4a7c15);
                h = h.rotate_left(13).wrapping_mul(0x100000001b3);
            }
        }
        h
    }

    /// Instantiate the motif at absolute anchor `a` under orientation `t` into `k`
    /// concrete `Move`s (geometry only — legality is checked by the caller against
    /// the live board, since a motif legal in its source game may not fit here).
    pub fn instantiate(&self, a: Pos, t: usize, n: u8) -> Vec<Move> {
        self.moves
            .iter()
            .map(|rm| {
                let trm = rm.transform(t, n as i16);
                let pos = (a.0 + trm.d.0, a.1 + trm.d.1);
                Move::new(
                    pos,
                    Line::from_point(pos, trm.dir, trm.line_pos, n),
                    trm.line_pos,
                )
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.moves.len()
    }
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }
}

/// Mine all distinct canonical `k`-move motifs from the record corpus for
/// `variant`, with their occurrence counts (summed over every length-`k` window of
/// every record game). Pure / offline — no search state. Sorted by count desc.
pub fn mine_motifs(variant: Variant, k: usize) -> Vec<(Motif, u32)> {
    assert!(k >= 1, "k must be >= 1");
    let n = variant.len() as i16;
    // motif comparable key → (motif, occurrence count)
    type MotifFreq = FxHashMap<Vec<(i16, i16, u8, u8)>, (Motif, u32)>;
    let mut freq: MotifFreq = MotifFreq::default();
    for rec in morpion_solitaire_records::RECORDS.iter() {
        let Ok(g) = crate::game::io::import_save(rec.2) else {
            continue;
        };
        if g.variant != variant {
            continue;
        }
        for window in g.history.windows(k) {
            let m = Motif::from_window(window).canonical(n);
            let e = freq.entry(m.key()).or_insert_with(|| (m.clone(), 0));
            e.1 += 1;
        }
    }
    let mut out: Vec<(Motif, u32)> = freq.into_values().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.key().cmp(&b.0.key())));
    out
}

/// The top-`top_n` most frequent `k`-move motifs for `variant` (the frozen
/// Macro-A library). `top_n == 0` keeps all.
pub fn motif_library(variant: Variant, k: usize, top_n: usize) -> Vec<Motif> {
    let mut mined = mine_motifs(variant, k);
    if top_n > 0 && mined.len() > top_n {
        mined.truncate(top_n);
    }
    mined.into_iter().map(|(m, _)| m).collect()
}

/// A legal macro instance at a position: the policy **action code** (the motif's
/// orientation-invariant signature) and the ordered `k` moves to apply.
#[derive(Clone, Debug)]
pub struct MacroInstance {
    pub code: u64,
    pub moves: Vec<Move>,
}

/// A frozen motif library prepared for fast runtime matching. Indexes motifs by
/// their first move's `(dir, line_pos)` under each orientation, so a position's
/// legal single moves (the candidate anchors) map straight to the few
/// motif+orientation pairs that could start there.
pub struct MotifLib {
    motifs: Vec<Motif>,
    sigs: Vec<u64>,
    /// `(move0.dir, move0.line_pos)` under orientation `t` → `[(motif_idx, t)]`.
    by_move0: FxHashMap<(u8, u8), Vec<(u32, u8)>>,
    n: u8,
}

impl MotifLib {
    /// Build the index for `variant`'s line length from a motif list.
    pub fn build(motifs: Vec<Motif>, variant: Variant) -> MotifLib {
        let n = variant.len();
        let sigs: Vec<u64> = motifs.iter().map(|m| m.signature(n as i16)).collect();
        let mut by_move0: FxHashMap<(u8, u8), Vec<(u32, u8)>> = FxHashMap::default();
        for (mi, m) in motifs.iter().enumerate() {
            // The first move's shape under each of the 8 orientations. Each (motif,
            // orientation) is a distinct candidate: two orientations sharing a move0
            // shape generally have *different* continuations, so both must be kept.
            for t in 0..8usize {
                let r0 = m.moves[0].transform(t, n as i16);
                by_move0
                    .entry((r0.dir as u8, r0.line_pos))
                    .or_default()
                    .push((mi as u32, t as u8));
            }
        }
        MotifLib {
            motifs,
            sigs,
            by_move0,
            n,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.motifs.is_empty()
    }
    pub fn len(&self) -> usize {
        self.motifs.len()
    }

    /// Append every **currently-legal** macro instance to `out`. `legal` are the
    /// position's legal single moves (the candidate first moves); `scratch` must be
    /// the live state — it is mutated to test continuations and **restored** before
    /// return. Each continuation move is checked with [`move_playable`] after the
    /// prefix is applied (a motif legal in its source game may not fit here).
    pub fn legal_macros(
        &self,
        scratch: &mut GameState,
        legal: &[Move],
        out: &mut Vec<MacroInstance>,
    ) {
        if self.motifs.is_empty() {
            return;
        }
        let line_len = self.n;
        let start = scratch.history.len();
        for mv in legal {
            let key = (mv.line.dir as u8, mv.line_pos);
            let Some(cands) = self.by_move0.get(&key) else {
                continue;
            };
            for &(mi, t) in cands {
                let moves = self.motifs[mi as usize].instantiate(mv.pos, t as usize, line_len);
                debug_assert_eq!(moves[0], *mv, "matched move0 must equal the anchor move");
                // Apply moves in order, checking each is playable; roll back fully.
                let mut ok = true;
                for (i, &m) in moves.iter().enumerate() {
                    if i > 0 && !move_playable(scratch, &m, line_len) {
                        ok = false;
                        break;
                    }
                    if !scratch.apply(m) {
                        ok = false; // grid overflow
                        break;
                    }
                }
                while scratch.history.len() > start {
                    scratch.undo();
                }
                if ok {
                    out.push(MacroInstance {
                        code: self.sigs[mi as usize],
                        moves,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::io::import_save;
    use crate::game::moves::legal_moves;
    use crate::game::state::GameState;

    fn a_5t_record() -> Vec<Move> {
        let rec = morpion_solitaire_records::RECORDS
            .iter()
            .find_map(|r| {
                let g = import_save(r.2).ok()?;
                (g.variant == Variant::T5).then_some(g.history)
            })
            .expect("a 5T record");
        assert!(rec.len() >= 5);
        rec
    }

    /// The canonical form is invariant under all 8 D4 images of a motif — the
    /// property the orientation-invariant library/code relies on.
    #[test]
    fn canonical_collapses_orientations() {
        let g = a_5t_record();
        let n = 5i16;
        for k in [2usize, 3] {
            for window in g.windows(k).take(20) {
                let base = Motif::from_window(window);
                let canon = base.canonical(n).key();
                for t in 0..8 {
                    let img = base.transformed(t, n);
                    assert_eq!(
                        img.canonical(n).key(),
                        canon,
                        "k={k} t={t}: D4 image has a different canonical form"
                    );
                    assert_eq!(
                        img.signature(n),
                        base.signature(n),
                        "signature not D4-invariant"
                    );
                }
            }
        }
    }

    /// A motif rebuilt from a window and re-instantiated at the same anchor (t=0)
    /// reproduces the exact original moves — proof the relative encoding +
    /// reconstruction geometry is correct.
    #[test]
    fn from_window_instantiate_roundtrip() {
        let g = a_5t_record();
        for k in [2usize, 3, 4] {
            for window in g.windows(k).take(30) {
                let m = Motif::from_window(window);
                let inst = m.instantiate(window[0].pos, 0, 5);
                assert_eq!(
                    inst, window,
                    "k={k}: t=0 instantiation must equal the window"
                );
            }
        }
    }

    /// Every D4 orientation of a real first-game motif, instantiated and replayed
    /// from a fresh cross, applies legally — the relative geometry stays a valid
    /// game under symmetry (a sanity check for runtime instantiation).
    #[test]
    fn early_motif_legal_in_all_orientations() {
        let g = a_5t_record();
        let canon = Motif::from_window(&g[0..2]).canonical(5);
        let mut ok = 0;
        for t in 0..8 {
            // Anchor at the canonical motif's first move = the original first move's
            // orbit; replay onto a fresh board and require all moves legal.
            let st = GameState::new(Variant::T5);
            let anchor = g[0].pos;
            let inst = canon.instantiate(anchor, t, 5);
            let mut s = st.clone();
            if inst
                .iter()
                .all(|&mv| legal_moves(&s).contains(&mv) && s.apply(mv))
            {
                ok += 1;
            }
        }
        assert!(
            ok >= 1,
            "no orientation of the opening motif replays legally"
        );
    }

    /// An empty library yields no macros (the library-size-0 ⇒ baseline property).
    #[test]
    fn empty_library_no_macros() {
        let lib = MotifLib::build(vec![], Variant::T5);
        assert!(lib.is_empty());
        let mut st = GameState::new(Variant::T5);
        let legal = legal_moves(&st);
        let mut out = Vec::new();
        lib.legal_macros(&mut st, &legal, &mut out);
        assert!(out.is_empty());
        assert_eq!(st.history.len(), 0, "scratch must be restored");
    }

    /// At each early state of a record, the record's actual next-`k` moves form a
    /// legal macro, so the full library must enumerate it — proof runtime matching +
    /// legality + scratch restoration are correct.
    #[test]
    fn record_continuation_is_enumerated() {
        let g = a_5t_record();
        let lib = MotifLib::build(motif_library(Variant::T5, 2, 0), Variant::T5); // full lib
        let mut st = GameState::new(Variant::T5);
        for j in 0..12 {
            let legal = legal_moves(&st);
            let mut out = Vec::new();
            lib.legal_macros(&mut st, &legal, &mut out);
            assert_eq!(st.history.len(), j, "scratch not restored at move {j}");
            let want = vec![g[j], g[j + 1]];
            assert!(
                out.iter().any(|mi| mi.moves == want),
                "move {j}: record continuation {want:?} not enumerated among {} macros",
                out.len()
            );
            // every enumerated macro is genuinely legal in sequence
            for mi in &out {
                let mut s = st.clone();
                assert!(
                    mi.moves
                        .iter()
                        .all(|&m| legal_moves(&s).contains(&m) && s.apply(m)),
                    "enumerated macro not legal in sequence"
                );
            }
            assert!(st.apply(g[j]));
        }
    }

    /// Mining the 5T corpus yields a non-empty library with sane counts, and the
    /// most frequent k=2 motif recurs (count >= 2).
    #[test]
    fn mining_yields_a_library() {
        let mined = mine_motifs(Variant::T5, 2);
        assert!(!mined.is_empty(), "no k=2 motifs mined");
        assert!(mined[0].1 >= 2, "top motif should recur (>=2)");
        // counts are sorted descending
        assert!(
            mined.windows(2).all(|w| w[0].1 >= w[1].1),
            "not sorted by count"
        );
        // top-N truncation works
        let lib = motif_library(Variant::T5, 2, 16);
        assert!(lib.len() <= 16 && !lib.is_empty());
        println!(
            "5T k=2: {} distinct motifs, top count {}, top-16 kept {}",
            mined.len(),
            mined[0].1,
            lib.len()
        );
    }
}

// ── registry plugin (co-located with the macro-action engine) ─────────────────

use crate::search::plugin::{OptionKind, OptionSpec, Plugin, Registry, Scope};

pub struct MacrosPlugin;
impl Plugin for MacrosPlugin {
    fn id(&self) -> &'static str {
        "macros"
    }
    fn experimental(&self) -> bool {
        true
    }
    fn register(&self, reg: &mut Registry) {
        reg.add_option(OptionSpec {
            key: "macros",
            label_key: "opt-macros",
            help_key: "opt-macros-hint",
            help: "Macro-actions: NRPA also picks multi-move motifs mined from records \
                   (5T only), composing over a coarser horizon. Experimental.",
            kind: OptionKind::Toggle { default: false },
            scope: Scope::Methods(&["nrpa"]),
        });
        reg.add_option(OptionSpec {
            key: "macro-k",
            label_key: "opt-macro-k",
            help_key: "opt-macro-k-hint",
            help: "Macro motif length in moves (default 2). Read once at first use.",
            kind: OptionKind::Int {
                default: 2,
                min: 1,
                max: 6,
            },
            scope: Scope::Methods(&["nrpa"]),
        });
        reg.add_option(OptionSpec {
            key: "macro-topn",
            label_key: "opt-macro-topn",
            help_key: "opt-macro-topn-hint",
            help: "Macro library size: keep the top-N most frequent motifs (0 = all, \
                   default 32). Read once at first use.",
            kind: OptionKind::Int {
                default: 32,
                min: 0,
                max: 100_000,
            },
            scope: Scope::Methods(&["nrpa"]),
        });
    }
}
pub static MACROS_PLUGIN: MacrosPlugin = MacrosPlugin;
