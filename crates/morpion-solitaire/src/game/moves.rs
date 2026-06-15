use super::board::Pos;
use super::line::{Dir, Line};
use super::rules::TouchMode;
use super::state::GameState;
use serde::{Deserialize, Serialize};

/// A single game move: place a new point at `pos` and draw `line`.
/// `line_pos` is the 0-based index of `pos` within the line (0 = origin end).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Move {
    pub pos: Pos,
    pub line: Line,
    pub line_pos: u8,
}

impl Move {
    pub fn new(pos: Pos, line: Line, line_pos: u8) -> Self {
        Self {
            pos,
            line,
            line_pos,
        }
    }
}

/// Generate all legal moves from the current game state.
///
/// A move is a (pos, line, line_pos) triple where:
/// - `pos` is an empty cell that will be placed
/// - `line` is a new line of `line_len` consecutive cells in some direction
/// - `pos` is part of `line` at offset `line_pos`
/// - All other cells of `line` are already occupied
/// - `line` does not conflict with any existing parallel line (T or D rule)
pub fn legal_moves(state: &GameState) -> Vec<Move> {
    let mut moves = Vec::new();
    legal_moves_into(state, &mut moves);
    moves
}

/// Same as [`legal_moves`] but writes into a caller-owned buffer (cleared
/// first), so the hot search path can reuse one allocation across nodes.
///
/// Bitboard generator: the occupancy bitset is scanned a whole row at a time
/// (shift + AND) for each direction, instead of testing each window cell by
/// cell. The bounding box is computed once and shared by the four directions.
pub fn legal_moves_into(state: &GameState, moves: &mut Vec<Move>) {
    moves.clear();
    let Some((_, min_y, _, max_y)) = state.bounding_box() else {
        return;
    };
    for dir in Dir::ALL {
        append_dir(state, moves, min_y, max_y, dir);
    }
}

/// Append a single direction's legal moves via the bitboard row scan.
///
/// A length-`n` window `(gx + k·dx, gy + k·dy)` is found by loading each row it
/// touches as a `u128` shifted right by `k·dx`, collapsing the window onto one
/// column position; ANDing all but one offset yields every window with exactly
/// one empty cell, for the whole row at once. `dx ∈ {0,1}` (column step) and
/// `dy ∈ {−1,0,1}` (row step) come straight from `dir.delta()`, so one routine
/// covers H, V, DP and DN. The board margin keeps occupied columns ≤ 123, so
/// the right-shifts never wrap a row edge: no boundary masking, no sub-word
/// shifts, for any direction.
#[allow(clippy::needless_range_loop)] // index-parallel SWAR shifts; clearer by index
fn append_dir(state: &GameState, out: &mut Vec<Move>, min_y: i16, max_y: i16, dir: Dir) {
    use super::board::{Row, GRID, OFFSET};
    let n = state.variant.len();
    let nu = n as usize;
    let max_overlap: u8 = match state.variant.touch_mode {
        TouchMode::Touching => 1,
        TouchMode::Disjoint => 0,
    };
    let forbid = n - 1 - max_overlap;

    let (dx, dy) = dir.delta();
    let grid = GRID as isize;
    let span = nu as isize - 1;
    let min_gy = min_y as isize + OFFSET as isize;
    let max_gy = max_y as isize + OFFSET as isize;

    // Origin-row range covering every window that can touch the occupied rows,
    // clamped so each touched row `gy + k·dy` stays in `[0, GRID)`.
    let (gy_lo, gy_hi) = match dy {
        0 => (min_gy.max(0), max_gy.min(grid - 1)),
        d if d > 0 => ((min_gy - span).max(0), max_gy.min(grid - 1 - span)),
        _ => (min_gy.max(span), (max_gy + span).min(grid - 1)),
    };
    if gy_lo > gy_hi {
        return;
    }

    let mut a = [0 as Row; 8]; // n ≤ 5 for every variant
    for gy in gy_lo..=gy_hi {
        for k in 0..nu {
            let ry = (gy + k as isize * dy as isize) as usize;
            a[k] = state.board.row(ry) >> (k as u32 * dx as u32);
        }
        for j in 0..nu {
            // Windows whose single empty cell sits at offset `j`.
            let mut hits = !a[j];
            for k in 0..nu {
                if k != j {
                    hits &= a[k];
                }
            }
            while hits != 0 {
                let gx = hits.trailing_zeros() as usize;
                hits &= hits - 1;
                let ox = gx as i16 - OFFSET;
                let oy = gy as i16 - OFFSET;
                let line = Line::new((ox, oy), dir);
                if state.line_index.conflicts(&line, forbid) {
                    continue;
                }
                let new_pos = (ox + j as i16 * dx, oy + j as i16 * dy);
                out.push(Move::new(new_pos, line, j as u8));
            }
        }
    }
}

/// Full scalar reference (every direction), preserved as the oracle for the
/// `legal_moves_matches_scalar` differential test and the generator bench.
#[cfg(test)]
pub(crate) fn legal_moves_scalar_into(state: &GameState, out: &mut Vec<Move>) {
    out.clear();
    for dir in Dir::ALL {
        legal_moves_dir_scalar_append(state, out, dir);
    }
}

/// Scalar single-direction generator (appends, does not clear): the `dir`-only
/// slice of the original cell-scan, used as the baseline oracle for tests/bench.
#[cfg(test)]
pub(crate) fn legal_moves_dir_scalar_append(state: &GameState, out: &mut Vec<Move>, dir: Dir) {
    let n = state.variant.len();
    let max_overlap: u8 = match state.variant.touch_mode {
        TouchMode::Touching => 1,
        TouchMode::Disjoint => 0,
    };
    let forbid = n - 1 - max_overlap;
    let (dx, dy) = dir.delta();

    for &cell in &state.board.cells {
        for offset in 0..n {
            let origin = (cell.0 - offset as i16 * dx, cell.1 - offset as i16 * dy);
            let mut occ: u8 = 0;
            let mut empty_idx: Option<u8> = None;
            let mut multi_empty = false;
            for k in 0..n {
                let p = (origin.0 + k as i16 * dx, origin.1 + k as i16 * dy);
                if state.board.contains(p) {
                    occ += 1;
                } else {
                    match empty_idx {
                        None => empty_idx = Some(k),
                        Some(_) => {
                            multi_empty = true;
                            break;
                        }
                    }
                }
            }
            if multi_empty || occ != n - 1 {
                continue;
            }
            let empty_idx = empty_idx.unwrap();
            let first_occ = if empty_idx == 0 { 1 } else { 0 };
            if offset != first_occ {
                continue;
            }
            let line = Line::new(origin, dir);
            if state.line_index.conflicts(&line, forbid) {
                continue;
            }
            out.push(Move::new(
                (
                    origin.0 + empty_idx as i16 * dx,
                    origin.1 + empty_idx as i16 * dy,
                ),
                line,
                empty_idx,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{rules::Variant, state::GameState};

    fn count_moves(variant: Variant) -> usize {
        legal_moves(&GameState::new(variant)).len()
    }

    fn mid_game(moves: usize) -> GameState {
        let mut state = GameState::new(Variant::T5);
        for _ in 0..moves {
            let ms = legal_moves(&state);
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

    #[test]
    #[ignore = "timing benchmark, run with --ignored --nocapture"]
    fn bench_legal_moves_generators() {
        use std::hint::black_box;
        use std::time::Instant;

        let state = mid_game(25);
        let (_, min_y, _, max_y) = state.bounding_box().unwrap();
        let iters = 500_000u64;
        let time = |f: &dyn Fn(&GameState, &mut Vec<Move>)| -> (f64, usize) {
            let mut buf = Vec::new();
            for _ in 0..1000 {
                f(&state, &mut buf);
            }
            let t = Instant::now();
            let mut acc = 0usize;
            for _ in 0..iters {
                f(black_box(&state), &mut buf);
                acc += black_box(buf.len());
            }
            (t.elapsed().as_secs_f64() * 1e9 / iters as f64, acc)
        };

        let (bb, a) = time(&|s, o| legal_moves_into(s, o));
        let (sc, b) = time(&|s, o| legal_moves_scalar_into(s, o));
        println!(
            "full: bitboard {bb:.1} ns | scalar {sc:.1} ns | speedup {:.2}x | moves {}/{}",
            sc / bb,
            a / iters as usize,
            b / iters as usize,
        );
        for dir in Dir::ALL {
            let (bb, _) = time(&|s, o| {
                o.clear();
                append_dir(s, o, min_y, max_y, dir);
            });
            let (sc, _) = time(&|s, o| {
                o.clear();
                legal_moves_dir_scalar_append(s, o, dir);
            });
            println!(
                "  {dir:?}: bitboard {bb:.1} ns | scalar {sc:.1} ns | speedup {:.2}x",
                sc / bb
            );
        }
    }

    /// Each direction of the bitboard generator must match the scalar oracle's
    /// subset for that direction, across random positions and all variants.
    fn assert_dir_matches(dir: Dir) {
        use rand::{rngs::StdRng, Rng, SeedableRng};
        use std::collections::HashSet;

        let mut rng = StdRng::seed_from_u64(0xC0FFEE_u64);
        let (mut bb, mut sc) = (Vec::new(), Vec::new());

        for variant in [Variant::T5, Variant::D5, Variant::T4, Variant::D4] {
            for _ in 0..300 {
                let mut state = GameState::new(variant);
                let depth = rng.gen_range(0..45);
                for _ in 0..depth {
                    let (_, min_y, _, max_y) = state.bounding_box().unwrap();
                    bb.clear();
                    append_dir(&state, &mut bb, min_y, max_y, dir);
                    sc.clear();
                    legal_moves_dir_scalar_append(&state, &mut sc, dir);

                    let bb_set: HashSet<Move> = bb.iter().copied().collect();
                    let sc_set: HashSet<Move> = sc.iter().copied().collect();
                    assert_eq!(bb.len(), bb_set.len(), "{dir:?}: duplicates emitted");
                    assert_eq!(
                        bb_set,
                        sc_set,
                        "{dir:?} mismatch for {variant:?} at score {}",
                        state.score()
                    );

                    let mvs = legal_moves(&state);
                    if mvs.is_empty() {
                        break;
                    }
                    state.apply(mvs[rng.gen_range(0..mvs.len())]);
                }
            }
        }
    }

    #[test]
    fn h_bitboard_matches_reference() {
        assert_dir_matches(Dir::H);
    }

    #[test]
    fn v_bitboard_matches_reference() {
        assert_dir_matches(Dir::V);
    }

    #[test]
    fn dn_bitboard_matches_reference() {
        assert_dir_matches(Dir::DN);
    }

    #[test]
    fn dp_bitboard_matches_reference() {
        assert_dir_matches(Dir::DP);
    }

    /// The promoted bitboard `legal_moves` must equal the full scalar oracle.
    #[test]
    fn legal_moves_matches_scalar() {
        use rand::{rngs::StdRng, Rng, SeedableRng};
        use std::collections::HashSet;

        let mut rng = StdRng::seed_from_u64(0x1234_5678_u64);
        let mut sc = Vec::new();

        for variant in [Variant::T5, Variant::D5, Variant::T4, Variant::D4] {
            for _ in 0..300 {
                let mut state = GameState::new(variant);
                let depth = rng.gen_range(0..45);
                for _ in 0..depth {
                    let bb = legal_moves(&state);
                    legal_moves_scalar_into(&state, &mut sc);
                    let bb_set: HashSet<Move> = bb.iter().copied().collect();
                    let sc_set: HashSet<Move> = sc.iter().copied().collect();
                    assert_eq!(bb.len(), bb_set.len(), "duplicates emitted");
                    assert_eq!(
                        bb_set,
                        sc_set,
                        "legal_moves mismatch for {variant:?} at score {}",
                        state.score()
                    );

                    if bb.is_empty() {
                        break;
                    }
                    state.apply(bb[rng.gen_range(0..bb.len())]);
                }
            }
        }
    }

    #[test]
    fn initial_cross_cell_counts() {
        assert_eq!(GameState::new(Variant::T5).board.len(), 36);
        assert_eq!(GameState::new(Variant::D5).board.len(), 36);
        // 4T/4D: a centred D4-symmetric Greek cross on a 0..=6 grid.
        assert_eq!(GameState::new(Variant::T4).board.len(), 24);
        assert_eq!(GameState::new(Variant::D4).board.len(), 24);
    }

    /// The initial cross must be symmetric under the full dihedral group D4 (both
    /// mirrors and the diagonal transpose) for every variant — a regression guard
    /// for the off-centre 4T/4D cross (longer right/bottom arms).
    #[test]
    fn initial_cross_is_d4_symmetric() {
        use std::collections::HashSet;
        for v in [Variant::T5, Variant::D5, Variant::T4, Variant::D4] {
            let st = GameState::new(v);
            let cells: HashSet<(i16, i16)> = st.board.cells.iter().copied().collect();
            let (minx, miny, maxx, maxy) = st.bounding_box().unwrap();
            assert_eq!(
                maxx - minx,
                maxy - miny,
                "{v:?}: cross bounding box must be square"
            );
            for &(x, y) in &cells {
                assert!(cells.contains(&(minx + maxx - x, y)), "{v:?}: no h-mirror");
                assert!(cells.contains(&(x, miny + maxy - y)), "{v:?}: no v-mirror");
                assert!(
                    cells.contains(&(minx + (y - miny), miny + (x - minx))),
                    "{v:?}: no diagonal symmetry"
                );
            }
        }
    }

    #[test]
    fn initial_positions_have_moves() {
        assert!(count_moves(Variant::T5) > 0);
        assert!(count_moves(Variant::D5) > 0);
        assert!(count_moves(Variant::T4) > 0);
        assert!(count_moves(Variant::D4) > 0);
    }

    // 5T and 5D start from the same cross → must have some shared first moves.
    // 5D is strictly more restrictive, so it has ≤ as many first moves as 5T.
    #[test]
    fn d5_leq_t5_move_count() {
        assert!(count_moves(Variant::D5) <= count_moves(Variant::T5));
    }

    #[test]
    fn apply_and_undo_roundtrip() {
        let mut state = GameState::new(Variant::T5);
        let moves = legal_moves(&state);
        assert!(!moves.is_empty());

        let mv = moves[0];
        let cells_before = state.board.len();
        let score_before = state.score();

        state.apply(mv);
        assert_eq!(state.score(), score_before + 1);
        assert_eq!(state.board.len(), cells_before + 1);
        assert!(state.board.contains(mv.pos));
        assert!(state.line_index.contains(&mv.line));

        let undone = state.undo().unwrap();
        assert_eq!(undone, mv);
        assert_eq!(state.score(), score_before);
        assert_eq!(state.board.len(), cells_before);
        assert!(!state.board.contains(mv.pos));
        assert!(!state.line_index.contains(&mv.line));
    }

    #[test]
    fn redo_after_undo() {
        let mut state = GameState::new(Variant::T5);
        let mv = legal_moves(&state)[0];
        state.apply(mv);
        state.undo();
        assert!(state.can_redo());
        let redone = state.redo().unwrap();
        assert_eq!(redone, mv);
        assert!(state.board.contains(mv.pos));
    }

    #[test]
    fn applied_move_not_regenerated() {
        let mut state = GameState::new(Variant::T5);
        let mv = legal_moves(&state)[0];
        state.apply(mv);
        // The same line cannot be drawn again
        let new_moves = legal_moves(&state);
        assert!(!new_moves.iter().any(|m| m.line == mv.line));
    }

    // Quick smoke test: play 5 moves without crashing.
    #[test]
    fn play_five_moves_5t() {
        let mut state = GameState::new(Variant::T5);
        for _ in 0..5 {
            let moves = legal_moves(&state);
            if moves.is_empty() {
                break;
            }
            state.apply(moves[0]);
        }
        assert!(state.score() <= 5);
    }
}
