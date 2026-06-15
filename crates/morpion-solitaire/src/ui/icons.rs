//! Small vector icon buttons drawn with the egui painter, so they render
//! regardless of which glyphs the bundled fonts contain (symbol glyphs like
//! `⊕`/`⇄` show up as tofu on a text font). Used for the board's overlaid
//! view/edit toolbars and the side panel's action buttons.

use egui::{Pos2, Response, Sense, Shape, Stroke, Ui, Vec2};

#[derive(Clone, Copy)]
pub enum Icon {
    Rotate,
    Flip,
    Recenter,
    Undo,
    Redo,
    Arrows,
    Numbers,
    Targets,
    // Side-panel actions.
    New,
    Copy,
    Export,
    Import,
    Play,
    Pause,
    Stop,
    Sun,
    Moon,
    Info,
}

/// A square icon button. `selected` highlights it (toggles); `enabled` dims it
/// and ignores clicks. Returns the response (check `.clicked()`).
pub fn icon_button(ui: &mut Ui, icon: Icon, selected: bool, enabled: bool) -> Response {
    let size = Vec2::splat(28.0);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    let visuals = ui.style().interact_selectable(&resp, selected);
    let painter = ui.painter();
    painter.rect(rect, 5.0, visuals.weak_bg_fill, visuals.bg_stroke);
    let col = if enabled {
        visuals.fg_stroke.color
    } else {
        ui.visuals().weak_text_color()
    };
    let r = rect.shrink(5.5);
    let w = (r.width() * 0.13).max(1.8);
    let stroke = Stroke::new(w, col);
    // Fractional point inside `r` (0..1 on each axis).
    let p = |fx: f32, fy: f32| Pos2::new(r.left() + fx * r.width(), r.top() + fy * r.height());
    let painter = ui.painter();

    // Polyline of an arc (angles in radians, 0 = +x, growing clockwise in screen).
    let arc = |cx: f32, cy: f32, rad: f32, a0: f32, a1: f32| -> Vec<Pos2> {
        (0..=24)
            .map(|i| {
                let t = a0 + (a1 - a0) * i as f32 / 24.0;
                Pos2::new(
                    r.left() + (cx + rad * t.cos()) * r.width(),
                    r.top() + (cy + rad * t.sin()) * r.height(),
                )
            })
            .collect()
    };
    let tri = |tip: Pos2, back: Pos2, half: f32| {
        // Triangle with apex `tip`, base centred at `back`, half-width `half`.
        let d = (tip - back).normalized();
        let n = Vec2::new(-d.y, d.x) * half;
        Shape::convex_polygon(vec![tip, back + n, back - n], col, Stroke::NONE)
    };

    use std::f32::consts::PI;
    match icon {
        Icon::Rotate => {
            // Clockwise circular arrow with a gap at the top-right.
            let pts = arc(0.5, 0.5, 0.34, -0.35 * PI, 1.15 * PI);
            painter.add(Shape::line(pts.clone(), stroke));
            // Arrowhead at the starting end, pointing clockwise (tangent).
            let start = pts[0];
            let tang = (pts[0] - pts[1]).normalized();
            painter.add(tri(
                start + tang * r.width() * 0.16,
                start,
                r.width() * 0.13,
            ));
        }
        Icon::Flip => {
            // Vertical mirror axis + a triangle pointing out on each side.
            painter.line_segment([p(0.5, 0.12), p(0.5, 0.88)], stroke);
            painter.add(tri(p(0.06, 0.5), p(0.36, 0.5), r.height() * 0.18));
            painter.add(tri(p(0.94, 0.5), p(0.64, 0.5), r.height() * 0.18));
        }
        Icon::Recenter => {
            // Square frame + centre dot (fit / recenter).
            painter.rect_stroke(
                egui::Rect::from_min_max(p(0.12, 0.12), p(0.88, 0.88)),
                2.0,
                stroke,
            );
            painter.circle_filled(p(0.5, 0.5), r.width() * 0.1, col);
        }
        Icon::Undo | Icon::Redo => {
            // Conventional curved arrow: sweep from the right, up over the top, and
            // back down to the lower-left, with the arrowhead pointing down-left
            // (the classic "↶" undo). Redo is the exact horizontal mirror.
            let pts = arc(0.5, 0.45, 0.30, 0.0, -0.92 * PI);
            let mirror = |p: Pos2| Pos2::new(2.0 * r.center().x - p.x, p.y);
            let pts: Vec<Pos2> = match icon {
                Icon::Redo => pts.iter().map(|&p| mirror(p)).collect(),
                _ => pts,
            };
            painter.add(Shape::line(pts.clone(), stroke));
            let end = *pts.last().unwrap();
            let tang = (end - pts[pts.len() - 2]).normalized();
            painter.add(tri(end + tang * r.width() * 0.34, end, r.width() * 0.26));
        }
        Icon::Arrows => {
            // A single direction triangle (the board's line marker).
            painter.add(tri(p(0.82, 0.5), p(0.3, 0.5), r.height() * 0.3));
        }
        Icon::Numbers => {
            // A numbered token: circle outline with a small "1"-like bar.
            painter.circle_stroke(p(0.5, 0.5), r.width() * 0.38, stroke);
            painter.line_segment([p(0.5, 0.3), p(0.5, 0.7)], stroke);
            painter.line_segment([p(0.4, 0.4), p(0.5, 0.3)], stroke);
        }
        Icon::Targets => {
            // A ring with a centre dot — like the legal-move markers it toggles.
            painter.circle_stroke(p(0.5, 0.5), r.width() * 0.34, stroke);
            painter.circle_filled(p(0.5, 0.5), r.width() * 0.1, col);
        }
        Icon::New => {
            // A bold plus — "new".
            let plus = Stroke::new((w * 1.3).max(2.2), col);
            painter.line_segment([p(0.5, 0.16), p(0.5, 0.84)], plus);
            painter.line_segment([p(0.16, 0.5), p(0.84, 0.5)], plus);
        }
        Icon::Copy => {
            // Two overlapping sheets (the classic copy glyph).
            let bg = visuals.weak_bg_fill;
            painter.rect_stroke(
                egui::Rect::from_min_max(p(0.18, 0.18), p(0.6, 0.6)),
                2.0,
                stroke,
            );
            painter.rect_filled(
                egui::Rect::from_min_max(p(0.4, 0.4), p(0.82, 0.82)),
                2.0,
                bg,
            );
            painter.rect_stroke(
                egui::Rect::from_min_max(p(0.4, 0.4), p(0.82, 0.82)),
                2.0,
                stroke,
            );
        }
        Icon::Export => {
            // Arrow rising out of an open tray — "export / save out".
            painter.line_segment([p(0.5, 0.66), p(0.5, 0.2)], stroke);
            painter.add(tri(p(0.5, 0.12), p(0.5, 0.36), r.width() * 0.17));
            painter.add(Shape::line(
                vec![p(0.22, 0.6), p(0.22, 0.84), p(0.78, 0.84), p(0.78, 0.6)],
                stroke,
            ));
        }
        Icon::Import => {
            // Arrow dropping into an open tray — "import / load in".
            painter.line_segment([p(0.5, 0.14), p(0.5, 0.6)], stroke);
            painter.add(tri(p(0.5, 0.68), p(0.5, 0.44), r.width() * 0.17));
            painter.add(Shape::line(
                vec![p(0.22, 0.6), p(0.22, 0.84), p(0.78, 0.84), p(0.78, 0.6)],
                stroke,
            ));
        }
        Icon::Play => {
            // Right-pointing triangle.
            painter.add(Shape::convex_polygon(
                vec![p(0.32, 0.18), p(0.32, 0.82), p(0.84, 0.5)],
                col,
                Stroke::NONE,
            ));
        }
        Icon::Pause => {
            // Two vertical bars.
            painter.rect_filled(
                egui::Rect::from_min_max(p(0.3, 0.2), p(0.44, 0.8)),
                1.0,
                col,
            );
            painter.rect_filled(
                egui::Rect::from_min_max(p(0.56, 0.2), p(0.7, 0.8)),
                1.0,
                col,
            );
        }
        Icon::Sun => {
            // Disc with eight rays — "switch to light".
            painter.circle_filled(p(0.5, 0.5), r.width() * 0.2, col);
            let c = r.center();
            for i in 0..8 {
                let a = i as f32 * PI / 4.0;
                let (cs, sn) = (a.cos(), a.sin());
                painter.line_segment(
                    [
                        Pos2::new(c.x + cs * r.width() * 0.32, c.y + sn * r.width() * 0.32),
                        Pos2::new(c.x + cs * r.width() * 0.46, c.y + sn * r.width() * 0.46),
                    ],
                    stroke,
                );
            }
        }
        Icon::Moon => {
            // Crescent: a disc with a bg-coloured disc carved out — "switch to dark".
            let c = p(0.5, 0.5);
            painter.circle_filled(c, r.width() * 0.34, col);
            painter.circle_filled(
                Pos2::new(c.x + r.width() * 0.17, c.y - r.width() * 0.07),
                r.width() * 0.3,
                visuals.weak_bg_fill,
            );
        }
        Icon::Info => {
            // Circled "i" — the rules / help button.
            painter.circle_stroke(p(0.5, 0.5), r.width() * 0.42, stroke);
            painter.circle_filled(p(0.5, 0.3), r.width() * 0.07, col);
            painter.line_segment([p(0.5, 0.44), p(0.5, 0.72)], stroke);
        }
        Icon::Stop => {
            // Filled square.
            painter.rect_filled(
                egui::Rect::from_min_max(p(0.24, 0.24), p(0.76, 0.76)),
                2.0,
                col,
            );
        }
    }
    resp
}
