use crate::game::{board::Pos, line::Dir, moves::Move, rules::TouchMode, state::GameState};
use crate::i18n::LANGUAGE_LOADER;
use egui::{Color32, Pos2, Sense, Stroke, Ui, Vec2};
use i18n_embed_fl::fl;

const CELL_PAD: f32 = 1.6;

// Palette shared with the SVG/PNG renderer (`crate::render`): a light board with
// black cross dots, white numbered move circles, and saturated per-direction lines.
const BG: Color32 = Color32::from_rgb(0xf7, 0xf7, 0xf4);
const INK: Color32 = Color32::from_rgb(0x15, 0x16, 0x1c);
const WHITE: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
const GOLD: Color32 = Color32::from_rgb(0xff, 0xd2, 0x3f);

/// Apply the current view orientation (purely cosmetic D4 transform) to a grid
/// coordinate: an optional horizontal flip followed by `rot` quarter-turns.
fn view_pos(rot: u8, flip: bool, (x, y): Pos) -> Pos {
    let (x, y) = if flip { (-x, y) } else { (x, y) };
    match rot % 4 {
        0 => (x, y),
        1 => (-y, x),
        2 => (-x, -y),
        _ => (y, -x),
    }
}

/// How the player resolves several collinear lines that complete at one point.
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InputMode {
    /// Cursor aim picks the line; the scroll wheel cycles the collinear
    /// candidates; a single click plays.
    Aim,
    /// Click a point to lock it into line-choice (the dot turns orange), move the
    /// cursor to aim the line among the collinear candidates, then click again to
    /// play it. Escape cancels. (Mouse-oriented; touch may want a future mode.)
    Click,
}

#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    state: &GameState,
    legal: &[Move],
    hovered: &mut Option<Move>,
    view_rot: u8,
    view_flip: bool,
    view_arrows: bool,
    view_numbers: bool,
    show_legal: bool,
    mode: InputMode,
    view_zoom: &mut f32,
    view_pan: &mut Vec2,
    dark: bool,
) -> Option<Move> {
    let available = ui.available_rect_before_wrap();
    let vt = |p: Pos| view_pos(view_rot, view_flip, p);

    // Board palette follows the UI theme. `ink` is outlines / cross dots /
    // numbers; `played_fill` fills the move circles (so they read as rings).
    let (bg, ink, played_fill) = if dark {
        (
            Color32::from_rgb(0x1b, 0x1c, 0x22),
            Color32::from_rgb(0xe6, 0xe6, 0xe9),
            Color32::from_rgb(0x2a, 0x2b, 0x33),
        )
    } else {
        (BG, INK, WHITE)
    };

    let (omin_x, omin_y, omax_x, omax_y) = state.bounding_box().unwrap_or((-5, -4, 6, 5));
    let corners = [
        vt((omin_x, omin_y)),
        vt((omin_x, omax_y)),
        vt((omax_x, omin_y)),
        vt((omax_x, omax_y)),
    ];
    let min_x = corners.iter().map(|c| c.0).min().unwrap();
    let max_x = corners.iter().map(|c| c.0).max().unwrap();
    let min_y = corners.iter().map(|c| c.1).min().unwrap();
    let max_y = corners.iter().map(|c| c.1).max().unwrap();

    let span_x = (max_x - min_x + 1) as f32 + 2.0 * CELL_PAD;
    let span_y = (max_y - min_y + 1) as f32 + 2.0 * CELL_PAD;

    let (resp, painter) = ui.allocate_painter(available.size(), Sense::click_and_drag());

    // Scroll wheel selects the line at the hovered point (when several complete
    // there — see below); hold Ctrl/Cmd or Shift to zoom toward the cursor
    // instead. Drag pans. Plain scroll over a single-line or empty spot does
    // nothing.
    let (scroll, zoom_mod) = ui.input(|i| {
        let m = i.modifiers;
        (i.smooth_scroll_delta.y, m.ctrl || m.shift || m.command)
    });
    let cycle_scroll = if resp.hovered() && !zoom_mod {
        scroll
    } else {
        0.0
    };
    if resp.hovered() && zoom_mod && scroll != 0.0 {
        let factor = (scroll * 0.0015).exp();
        let new_zoom = (*view_zoom * factor).clamp(0.4, 12.0);
        // Keep the point under the cursor fixed while zooming.
        if let Some(p) = resp.hover_pos() {
            let c = available.center().to_vec2() + *view_pan;
            *view_pan += (p.to_vec2() - c) * (1.0 - new_zoom / *view_zoom);
        }
        *view_zoom = new_zoom;
    }
    if resp.dragged() {
        *view_pan += resp.drag_delta();
    }

    let fit = (available.width() / span_x).min(available.height() / span_y);
    let cell_size = (fit * *view_zoom).max(3.0);
    let cx = available.center().x + view_pan.x;
    let cy = available.center().y + view_pan.y;
    let board_cx = (min_x as f32 + max_x as f32) / 2.0;
    let board_cy = (min_y as f32 + max_y as f32) / 2.0;

    let to_screen = |pos: Pos| -> Pos2 {
        let p = vt(pos);
        Pos2::new(
            cx + (p.0 as f32 - board_cx) * cell_size,
            cy + (p.1 as f32 - board_cy) * cell_size,
        )
    };

    painter.rect_filled(available, 0.0, bg);

    let line_w = (cell_size * 0.085).max(1.2);
    let dot_r = (cell_size * 0.26).max(2.6);

    // Drawn lines (full segments, saturated per-direction colour).
    for mv in &state.history {
        let pts: Vec<Pos2> = mv
            .line
            .positions(state.variant.len())
            .map(&to_screen)
            .collect();
        let c = dir_color(mv.line.dir, 255);
        for w in pts.windows(2) {
            painter.line_segment([w[0], w[1]], Stroke::new(line_w, c));
        }
    }

    // Per move, in the line's colour: a talon (perpendicular tick) tangent to EACH
    // of the line's two end dots (so the full span always shows and collinear lines
    // that share a point stay distinguishable), and a direction triangle at the
    // move's OWN circle pointing along the line.
    let n = state.variant.len() as i16;
    let mid = (state.variant.len() as usize - 1) / 2;
    if view_arrows {
        for mv in &state.history {
            let (dx, dy) = mv.line.dir.delta();
            let (tdx, tdy) = vt((dx, dy));
            let len = ((tdx * tdx + tdy * tdy) as f32).sqrt().max(1.0);
            let (ux, uy) = (tdx as f32 / len, tdy as f32 / len);
            let (perpx, perpy) = (-uy, ux);
            let color = dir_color(mv.line.dir, 255);

            // Talons at both ends, each tangent to its dot. They exist to keep
            // collinear lines that *share* an endpoint distinguishable, which can
            // only happen in Touching variants; in Disjoint variants lines never
            // touch, so the talons are pure clutter and are omitted. Skip an end
            // only when this move's own triangle sits there (line_pos 0 = origin,
            // n-1 = far).
            if matches!(state.variant.touch_mode, TouchMode::Touching) {
                let origin = mv.line.origin;
                let far = (origin.0 + (n - 1) * dx, origin.1 + (n - 1) * dy);
                let lp = mv.line_pos as i16;
                let tw = 0.22 * cell_size;
                for (end, into, at) in [(origin, 1.0f32, 0i16), (far, -1.0f32, n - 1)] {
                    if lp == at {
                        continue;
                    }
                    let e = to_screen(end);
                    let (tx, ty) = (e.x + into * ux * dot_r, e.y + into * uy * dot_r);
                    painter.line_segment(
                        [
                            Pos2::new(tx + perpx * tw, ty + perpy * tw),
                            Pos2::new(tx - perpx * tw, ty - perpy * tw),
                        ],
                        Stroke::new((line_w * 2.0).max(2.0), color),
                    );
                }
            }

            // Direction triangle at the move's own circle (tangent, interior side).
            let sgn = if (mv.line_pos as usize) <= mid {
                1.0
            } else {
                -1.0
            };
            let m = to_screen(mv.pos);
            let (bx, by) = (m.x + sgn * ux * dot_r, m.y + sgn * uy * dot_r);
            let (tlen, hw) = (0.30 * cell_size, 0.18 * cell_size);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(bx + sgn * ux * tlen, by + sgn * uy * tlen),
                    Pos2::new(bx + perpx * hw, by + perpy * hw),
                    Pos2::new(bx - perpx * hw, by - perpy * hw),
                ],
                color,
                Stroke::NONE,
            ));
        }
    }

    // Hovered move: pick the legal move whose point is nearest the cursor; when
    // several legal moves complete at that same point (two valid directions),
    // disambiguate by the cursor's offset from the point — aim slightly toward the
    // line you want and the best-aligned direction is selected.
    let hover_pos = resp.hover_pos();
    // Nearest legal point under the cursor (within half a cell).
    let nearest_pos = hover_pos.and_then(|hp| {
        legal
            .iter()
            .map(|mv| (mv.pos, to_screen(mv.pos).distance(hp)))
            .filter(|(_, d)| *d < cell_size * 0.5)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(pos, _)| pos)
    });
    // Resolve which legal line to preview/play. Several collinear lines can
    // complete at one point; the cursor's offset from the point aims among them,
    // each drawn faintly with an `i/N` indicator. In Click mode the first click
    // locks the point so you can aim by moving the cursor, then play with a second.
    *hovered = None;
    let mut to_play: Option<Move> = None;
    // True when the hovered (or locked) point has several possible lines, drives
    // the orange "ambiguous, pick a direction" dot in the preview below; a single
    // unambiguous line stays green.
    let mut multi_line = false;
    // When a point is locked (Click mode) we dim the whole board at the end and
    // redraw just this point's line options on top, so it's obvious the next move
    // is to aim the line. Captures (locked point, its candidates, aimed index).
    let mut lock_focus: Option<(Pos, Vec<Move>, usize)> = None;
    // Click mode persists the locked point in egui temp memory as Option<Pos>.
    let lock_id = egui::Id::new("line_pick_lock");
    let locked: Option<Pos> = if mode == InputMode::Click {
        ui.ctx()
            .memory(|m| m.data.get_temp::<Option<Pos>>(lock_id))
            .flatten()
            .filter(|lp| legal.iter().filter(|m| m.pos == *lp).count() >= 2)
    } else {
        None
    };
    if let Some(pos) = locked.or(nearest_pos) {
        let pt = to_screen(pos);
        let off = hover_pos.map(|hp| hp - pt).unwrap_or(Vec2::ZERO);
        let mut cands: Vec<Move> = legal.iter().filter(|m| m.pos == pos).copied().collect();
        cands.sort_by_key(|m| (m.line.dir.delta(), m.line.origin));
        let nc = cands.len();
        // Several collinear lines complete here → the highlighted dot reads orange
        // on hover (not only after a Click-mode lock).
        multi_line = nc >= 2;

        if cands.len() >= 2 {
            // Thinner than the real drawn lines (line_w ≈ 0.085·cell) so the
            // candidates read clearly as hints, not as played lines.
            let sw = (cell_size * 0.045).max(1.0);
            for mv in &cands {
                let pts: Vec<Pos2> = mv
                    .line
                    .positions(state.variant.len())
                    .map(&to_screen)
                    .collect();
                let col = dir_color(mv.line.dir, 70);
                for w in pts.windows(2) {
                    painter.line_segment([w[0], w[1]], Stroke::new(sw, col));
                }
            }
        }

        // Cursor-aimed default index.
        let aim = |mv: &Move| {
            mv.line
                .positions(state.variant.len())
                .map(|c| {
                    let s = to_screen(c);
                    off.x * (s.x - pt.x) + off.y * (s.y - pt.y)
                })
                .fold(f32::NEG_INFINITY, f32::max)
        };
        let aim_idx = (0..nc)
            .max_by(|&i, &j| aim(&cands[i]).partial_cmp(&aim(&cands[j])).unwrap())
            .unwrap_or(0);

        // The highlighted candidate. Aim mode adds scroll-wheel cycling on top of
        // the cursor aim; Click mode follows the cursor aim directly.
        let active = match mode {
            InputMode::Aim => {
                // Per-point selection (None = follow the cursor aim), persisted
                // across frames and keyed by the point; the wheel advances it,
                // accumulated so one notch (or a trackpad swipe) = one step.
                let id = egui::Id::new("hover_cycle_sel");
                let prev: Option<(Pos, Option<usize>, f32)> =
                    ui.ctx().memory(|m| m.data.get_temp(id));
                let (mut sel, mut accum) = match prev {
                    Some((p, s, a)) if p == pos => (s, a),
                    _ => (None, 0.0),
                };
                if nc >= 2 && cycle_scroll != 0.0 {
                    const STEP: f32 = 40.0;
                    accum += cycle_scroll;
                    while accum >= STEP {
                        sel = Some((sel.unwrap_or(aim_idx) + 1) % nc);
                        accum -= STEP;
                    }
                    while accum <= -STEP {
                        sel = Some((sel.unwrap_or(aim_idx) + nc - 1) % nc);
                        accum += STEP;
                    }
                }
                ui.ctx()
                    .memory_mut(|m| m.data.insert_temp(id, (pos, sel, accum)));
                sel.unwrap_or(aim_idx).min(nc.saturating_sub(1))
            }
            InputMode::Click => aim_idx,
        };
        *hovered = cands.get(active).copied();
        if locked == Some(pos) {
            lock_focus = Some((pos, cands.clone(), active));
        }

        // Decide whether this interaction plays (or locks) a move.
        match mode {
            // A single click plays the aimed/cycled candidate.
            InputMode::Aim => {
                if resp.clicked() {
                    to_play = *hovered;
                }
            }
            // Click locks a point, then aim by moving the cursor and click again to
            // play; a lone candidate plays at once. Escape cancels a lock.
            InputMode::Click => {
                let cancel =
                    ui.input(|i| i.key_pressed(egui::Key::Escape)) || resp.secondary_clicked();
                if locked.is_some() && cancel {
                    ui.ctx()
                        .memory_mut(|m| m.data.insert_temp(lock_id, None::<Pos>));
                } else if resp.clicked() {
                    if locked.is_some() || nc <= 1 {
                        to_play = *hovered;
                        ui.ctx()
                            .memory_mut(|m| m.data.insert_temp(lock_id, None::<Pos>));
                    } else {
                        ui.ctx()
                            .memory_mut(|m| m.data.insert_temp(lock_id, Some(pos)));
                    }
                }
            }
        }

        if nc >= 2 {
            painter.text(
                pt + egui::vec2(dot_r + 2.0, -dot_r - 2.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{}/{}", active + 1, cands.len()),
                egui::FontId::proportional((cell_size * 0.26).max(9.0)),
                ink,
            );
        }
    }

    // Preview the hovered line (bright, on top of any faint stubs).
    if let Some(mv) = *hovered {
        let pts: Vec<Pos2> = mv
            .line
            .positions(state.variant.len())
            .map(&to_screen)
            .collect();
        let c = dir_color(mv.line.dir, 150);
        let pw = (cell_size * 0.16).max(2.0);
        for w in pts.windows(2) {
            painter.line_segment([w[0], w[1]], Stroke::new(pw, c));
        }
        painter.circle_filled(
            to_screen(mv.pos),
            (cell_size * 0.22).max(4.0),
            // Orange while the point is ambiguous (several lines possible), shown
            // on hover; green when the line is unambiguous.
            if multi_line {
                Color32::from_rgb(0xf0, 0x8c, 0x28)
            } else {
                Color32::from_rgb(0x2f, 0x9e, 0x44)
            },
        );
    }

    // Legal-move markers (non-hovered). Hidden when the toggle is off — then a
    // move only reveals itself on hover, via the preview above.
    if show_legal {
        let mr = (cell_size * 0.18).max(3.0);
        for &mv in legal {
            if Some(mv) == *hovered {
                continue;
            }
            painter.circle_stroke(
                to_screen(mv.pos),
                mr,
                Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(47, 158, 68, 130)),
            );
        }
    }

    // Points: black cross dots, white move circles, gold last move — all the same
    // size with a thin ink outline (matching the renderer).
    let last_pos = state.history.last().map(|m| m.pos);
    let played: std::collections::HashSet<_> = state.history.iter().map(|m| m.pos).collect();
    let outline = Stroke::new((line_w * 0.9).max(1.0), ink);
    for &cell in &state.board.cells {
        let sp = to_screen(cell);
        let fill = if last_pos == Some(cell) {
            GOLD
        } else if played.contains(&cell) {
            played_fill
        } else {
            ink // initial cross
        };
        painter.circle(sp, dot_r, fill, outline);
    }

    // Move numbers on each played point (1-based play order).
    if view_numbers {
        let fsize = (cell_size * 0.30).max(5.0);
        let font = egui::FontId::proportional(fsize);
        for (i, mv) in state.history.iter().enumerate() {
            painter.text(
                to_screen(mv.pos),
                egui::Align2::CENTER_CENTER,
                (i + 1).to_string(),
                font.clone(),
                ink,
            );
        }
    }

    // Click-mode lock: dim the rest of the board so it's obvious that the point is
    // committed and the next thing to do is aim the line (then click to play, or
    // right-click / Esc to cancel). The locked point's options — its candidate
    // lines and every cross/move point those lines pass through — are redrawn
    // bright on top of the scrim.
    if let Some((pos, cands, active)) = lock_focus {
        let scrim = Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), 140);
        painter.rect_filled(available, 0.0, scrim);

        let pt = to_screen(pos);
        // Faint candidate lines, then the bright aimed one on top.
        let sw = (cell_size * 0.045).max(1.0);
        for mv in &cands {
            let pts: Vec<Pos2> = mv
                .line
                .positions(state.variant.len())
                .map(&to_screen)
                .collect();
            let col = dir_color(mv.line.dir, 90);
            for w in pts.windows(2) {
                painter.line_segment([w[0], w[1]], Stroke::new(sw, col));
            }
        }
        if let Some(mv) = cands.get(active) {
            let pts: Vec<Pos2> = mv
                .line
                .positions(state.variant.len())
                .map(&to_screen)
                .collect();
            let c = dir_color(mv.line.dir, 220);
            let pw = (cell_size * 0.16).max(2.0);
            for w in pts.windows(2) {
                painter.line_segment([w[0], w[1]], Stroke::new(pw, c));
            }
        }
        // Redraw the points spanned by the candidate lines at full strength
        // (same fills as the main pass: gold last move, filled move circles,
        // ink crosses) so the line can be read against real anchors.
        let mut seen = std::collections::HashSet::new();
        for mv in &cands {
            for cell in mv.line.positions(state.variant.len()) {
                if !seen.insert(cell) {
                    continue;
                }
                let fill = if last_pos == Some(cell) {
                    GOLD
                } else if played.contains(&cell) {
                    played_fill
                } else {
                    ink
                };
                painter.circle(to_screen(cell), dot_r, fill, outline);
            }
        }
        // The locked point reads orange ("pick a direction") with its i/N badge.
        painter.circle_filled(
            pt,
            (cell_size * 0.22).max(4.0),
            Color32::from_rgb(0xf0, 0x8c, 0x28),
        );
        painter.text(
            pt + egui::vec2(dot_r + 2.0, -dot_r - 2.0),
            egui::Align2::LEFT_BOTTOM,
            format!("{}/{}", active + 1, cands.len()),
            egui::FontId::proportional((cell_size * 0.26).max(9.0)),
            ink,
        );

        // A short hint pinned to the top-centre of the board: aim, click, cancel.
        // (The bottom-centre is taken by the picker-mode toolbar; the top corners
        // by the view/zoom toolbars, leaving the top centre clear.)
        let l = &*LANGUAGE_LOADER;
        let font = egui::FontId::proportional(14.0);
        let galley = painter.layout_no_wrap(fl!(l, "pick-locked-hint"), font, ink);
        let text_min = egui::pos2(
            available.center().x - galley.size().x / 2.0,
            available.top() + 14.0,
        );
        let pill = egui::Rect::from_min_size(text_min, galley.size()).expand2(egui::vec2(8.0, 5.0));
        painter.rect_filled(
            pill,
            6.0,
            Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), 235),
        );
        painter.galley(text_min, galley, ink);
    }

    if let Some(mv) = to_play {
        return Some(mv);
    }

    None
}

/// Per-direction line colours (matching `crate::render`), with an alpha for the
/// hover preview.
fn dir_color(dir: Dir, alpha: u8) -> Color32 {
    let (r, g, b) = match dir {
        Dir::H => (0x3b, 0x62, 0xc4),  // blue
        Dir::V => (0xc8, 0x63, 0x2e),  // terracotta
        Dir::DP => (0x2f, 0x9e, 0x44), // green
        Dir::DN => (0xb5, 0x35, 0x9c), // magenta
    };
    Color32::from_rgba_unmultiplied(r, g, b, alpha)
}
