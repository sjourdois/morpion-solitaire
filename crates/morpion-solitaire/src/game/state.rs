use super::board::Board;
use super::line_index::LineIndex;
use super::moves::Move;
use super::rules::Variant;

#[derive(Debug, Clone)]
pub struct GameState {
    pub variant: Variant,
    pub board: Board,
    /// Conflict index of drawn lines (replaces a `HashSet<Line>`). The lines
    /// themselves live in `history`; this is the acceleration structure for
    /// move generation. See `line_index`.
    pub line_index: LineIndex,
    pub history: Vec<Move>,
    pub redo_stack: Vec<Move>,
}

impl GameState {
    pub fn new(variant: Variant) -> Self {
        let mut s = Self {
            variant,
            board: Board::new(),
            line_index: LineIndex::new(),
            history: Vec::new(),
            redo_stack: Vec::new(),
        };
        s.setup_cross();
        s
    }

    /// Initial hollow-cross pattern using positive coordinates.
    ///
    /// The cross is the outline of a Greek cross with arms `n−1` cells wide,
    /// centred so the whole figure is D4-symmetric about `(w/2, w/2)`. Symmetry
    /// requires the arm band `[a, b]` to be centred, i.e. `a + b = w`; that is
    /// only possible when the arm width and the grid have matching parity, so the
    /// grid is `2n−1` wide for odd `n` and `2n−2` for even `n` (one row/col
    /// narrower). The old formula (`a = n−2`, `b = 2n−4`, `w = 2n−1`) was centred
    /// only for `n = 5`; for `n = 4` it pushed the band off-centre, leaving the
    /// right and bottom arms a cell too long.
    ///   5T/5D: arms at x/y ∈ {3,6} on a 0..=9 grid (36 cells).
    ///   4T/4D: arms at x/y ∈ {2,4} on a 0..=6 grid.
    fn setup_cross(&mut self) {
        let n = self.variant.len() as i16;
        let arm = n - 1; // arm width (cells)
        let w = if n % 2 == 1 { 2 * n - 1 } else { 2 * n - 2 }; // max coordinate index
        let a = (w - (arm - 1)) / 2; // inner arm boundary (band centred: a + b = w)
        let b = a + arm - 1; // outer arm boundary  (b − a + 1 = n − 1 cells wide)

        for x in 0..=w {
            for y in 0..=w {
                let in_cross = ((y == 0 || y == w) && x >= a && x <= b)    // top / bottom caps
                    || ((x == 0 || x == w) && y >= a && y <= b) // left / right caps
                    || ((x == a || x == b) && (y <= a || y >= b)) // vertical arm borders
                    || ((y == a || y == b) && (x <= a || x >= b)); // horizontal bar borders
                if in_cross {
                    self.board.insert((x, y));
                }
            }
        }
    }

    pub fn score(&self) -> usize {
        self.history.len()
    }

    /// Apply a move. Returns `false` *without changing anything* if it would
    /// overflow the fixed grid (see [`crate::game::board::GRID_OVERFLOW`]); the
    /// caller should then stop and save rather than continue.
    pub fn apply(&mut self, mv: Move) -> bool {
        if !self.board.insert(mv.pos) {
            return false;
        }
        self.line_index.insert(&mv.line);
        self.history.push(mv);
        self.redo_stack.clear();
        true
    }

    pub fn undo(&mut self) -> Option<Move> {
        let mv = self.history.pop()?;
        self.board.remove(mv.pos);
        self.line_index.remove(&mv.line);
        self.redo_stack.push(mv);
        Some(mv)
    }

    pub fn redo(&mut self) -> Option<Move> {
        // Re-apply without going through `apply`, which clears the redo stack —
        // that would drop every still-redoable move after the first.
        let mv = self.redo_stack.pop()?;
        self.board.insert(mv.pos);
        self.line_index.insert(&mv.line);
        self.history.push(mv);
        Some(mv)
    }

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Bounding box of all placed cells: (min_x, min_y, max_x, max_y).
    pub fn bounding_box(&self) -> Option<(i16, i16, i16, i16)> {
        let mut iter = self.board.cells.iter();
        let &(x0, y0) = iter.next()?;
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (x0, y0, x0, y0);
        for &(x, y) in iter {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        Some((min_x, min_y, max_x, max_y))
    }
}
