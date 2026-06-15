//! Render a game to a pretty, self-describing **SVG** (and, via `resvg`, PNG):
//! coloured lines per direction, a filled initial cross, numbered move circles,
//! and a tangent "talon" at each line's origin. Vector text uses the bundled
//! Atkinson Hyperlegible Next font (referenced by family; the PNG path loads the
//! TTF so it always renders).

// This module emits newline-terminated SVG lines by hand, so `write!(.., "…\n")`
// is intentional rather than a `writeln!` candidate.
#![allow(clippy::write_with_newline)]

use crate::game::{line::Dir, state::GameState};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

/// Rendering options.
#[derive(Debug, Clone, Copy)]
pub struct RenderOpts {
    /// Number each played point by its move order.
    pub numbers: bool,
}

impl Default for RenderOpts {
    fn default() -> Self {
        Self { numbers: true }
    }
}

const CELL: f64 = 48.0;
const PAD: f64 = 1.2;
const BG: &str = "#f7f7f4"; // soft off-white, like the community record grids
const CROSS: &str = "#15161c"; // initial cross: small filled dark dots
const INK: &str = "#15161c"; // outlines and numbers

fn dir_color(dir: Dir) -> &'static str {
    match dir {
        Dir::H => "#3b62c4",  // blue
        Dir::V => "#c8632e",  // terracotta
        Dir::DP => "#2f9e44", // green
        Dir::DN => "#b5359c", // magenta
    }
}

/// Render `state` to a standalone SVG document.
pub fn to_svg(state: &GameState, opts: &RenderOpts) -> String {
    let (min_x, min_y, max_x, max_y) = state.bounding_box().unwrap_or((-5, -4, 6, 5));
    let w = (max_x - min_x) as f64 + 1.0 + 2.0 * PAD;
    let h = (max_y - min_y) as f64 + 1.0 + 2.0 * PAD;
    let (iw, ih) = (w * CELL, h * CELL);
    let sx = |x: i16| (x - min_x) as f64 * CELL + PAD * CELL + CELL / 2.0;
    let sy = |y: i16| (y - min_y) as f64 * CELL + PAD * CELL + CELL / 2.0;

    let line_w = CELL * 0.085;
    let dot_r = CELL * 0.27; // played-move circles

    let mut s = String::new();
    let _ = write!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {iw:.0} {ih:.0}\" \
         font-family=\"'Atkinson Hyperlegible Next', sans-serif\">\n\
         <rect width=\"{iw:.0}\" height=\"{ih:.0}\" fill=\"{BG}\"/>\n"
    );

    // Lines (full segments), one polyline per move.
    let _ = write!(
        s,
        "<g fill=\"none\" stroke-linecap=\"round\" stroke-width=\"{line_w:.2}\">\n"
    );
    for mv in &state.history {
        let pts: Vec<String> = mv
            .line
            .positions(state.variant.len())
            .map(|(x, y)| format!("{:.1},{:.1}", sx(x), sy(y)))
            .collect();
        let _ = write!(
            s,
            "<polyline points=\"{}\" stroke=\"{}\"/>\n",
            pts.join(" "),
            dir_color(mv.line.dir)
        );
    }
    let _ = write!(s, "</g>\n");

    // Per move, in the line's colour:
    //  • a **talon** (perpendicular tick) tangent to EACH of the line's two end
    //    dots, so the full span is always visible (and two collinear lines sharing
    //    a point are distinguishable);
    //  • a **direction triangle** tangent to the move's OWN circle, pointing along
    //    the line — one per circle, showing which line it completed and its
    //    direction.
    let n = state.variant.len() as i16;
    let mid = (state.variant.len() as usize - 1) / 2;
    for mv in &state.history {
        let (dx, dy) = mv.line.dir.delta();
        let len = ((dx * dx + dy * dy) as f64).sqrt();
        let (ux, uy) = (dx as f64 / len, dy as f64 / len);
        let (px, py) = (-uy, ux); // perpendicular
        let color = dir_color(mv.line.dir);

        // Talons at both ends, each tangent to its dot on the line side. Skip an
        // end only when this move's own triangle sits there (its new point is that
        // endpoint: line_pos 0 = origin, n-1 = far) — otherwise they'd overlap.
        let origin = mv.line.origin;
        let far = (origin.0 + (n - 1) * dx, origin.1 + (n - 1) * dy);
        let lp = mv.line_pos as i16;
        let tw = CELL * 0.22;
        for (end, into, at) in [(origin, 1.0f64, 0i16), (far, -1.0f64, n - 1)] {
            if lp == at {
                continue;
            }
            let (ex, ey) = (sx(end.0), sy(end.1));
            let (tx, ty) = (ex + into * ux * dot_r, ey + into * uy * dot_r);
            let _ = write!(
                s,
                "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{color}\" \
                 stroke-width=\"{:.2}\" stroke-linecap=\"round\"/>\n",
                tx + px * tw,
                ty + py * tw,
                tx - px * tw,
                ty - py * tw,
                line_w * 2.0
            );
        }

        // Direction triangle at the move's own circle (tangent, interior side).
        let sgn = if (mv.line_pos as usize) <= mid {
            1.0
        } else {
            -1.0
        };
        let (mx, my) = (sx(mv.pos.0), sy(mv.pos.1));
        let (bx, by) = (mx + sgn * ux * dot_r, my + sgn * uy * dot_r);
        let (tlen, hw) = (CELL * 0.30, CELL * 0.18);
        let _ = write!(
            s,
            "<polygon points=\"{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" fill=\"{color}\"/>\n",
            bx + sgn * ux * tlen,
            by + sgn * uy * tlen, // apex, pointing into the line
            bx + px * hw,
            by + py * hw,
            bx - px * hw,
            by - py * hw,
        );
    }

    // Dots: the initial cross is small filled ink; played points are white discs
    // with a thin ink outline; the last move is a gold accent.
    let last = state.history.last().map(|m| m.pos);
    let played: HashSet<_> = state.history.iter().map(|m| m.pos).collect();
    for &(x, y) in &state.board.cells {
        let (cx, cy) = (sx(x), sy(y));
        if played.contains(&(x, y)) {
            let fill = if last == Some((x, y)) {
                "#ffd23f"
            } else {
                "#ffffff"
            };
            let _ = write!(
                s,
                "<circle cx=\"{cx:.1}\" cy=\"{cy:.1}\" r=\"{dot_r:.1}\" fill=\"{fill}\" \
                 stroke=\"{INK}\" stroke-width=\"{:.2}\"/>\n",
                line_w * 0.9
            );
        } else {
            // Initial cross: same size as the move circles, filled black.
            let _ = write!(
                s,
                "<circle cx=\"{cx:.1}\" cy=\"{cy:.1}\" r=\"{dot_r:.1}\" fill=\"{CROSS}\" \
                 stroke=\"{INK}\" stroke-width=\"{:.2}\"/>\n",
                line_w * 0.9
            );
        }
    }

    // Move numbers.
    if opts.numbers {
        let order: HashMap<_, usize> = state
            .history
            .iter()
            .enumerate()
            .map(|(i, m)| (m.pos, i + 1))
            .collect();
        let fs = CELL * 0.30;
        for (&(x, y), n) in &order {
            let _ = write!(
                s,
                "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"{fs:.1}\" text-anchor=\"middle\" \
                 dominant-baseline=\"central\" fill=\"{INK}\">{n}</text>\n",
                sx(x),
                sy(y)
            );
        }
    }

    let _ = write!(s, "</svg>\n");
    s
}

/// Rasterise the SVG with the bundled Atkinson Hyperlegible Next font so the move
/// numbers always render (no system fonts required). Native only.
#[cfg(not(target_arch = "wasm32"))]
fn render_pixmap(state: &GameState, opts: &RenderOpts) -> Result<resvg::tiny_skia::Pixmap, String> {
    use resvg::{tiny_skia, usvg};
    let svg = to_svg(state, opts);
    let mut options = usvg::Options::default();
    options.fontdb_mut().load_font_data(
        include_bytes!("../assets/fonts/AtkinsonHyperlegibleNext-Regular.ttf").to_vec(),
    );
    let tree = usvg::Tree::from_str(&svg, &options).map_err(|e| e.to_string())?;
    let size = tree.size().to_int_size();
    let mut pixmap =
        tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("zero-size image")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap)
}

/// Render the game to a PNG byte stream. Native only.
#[cfg(not(target_arch = "wasm32"))]
pub fn to_png(state: &GameState, opts: &RenderOpts) -> Result<Vec<u8>, String> {
    render_pixmap(state, opts)?
        .encode_png()
        .map_err(|e| e.to_string())
}

/// Render to raw RGBA8 pixels for the image clipboard: `(width, height, bytes)`.
/// The board background is fully opaque, so tiny-skia's premultiplied buffer is
/// identical to straight (un-premultiplied) RGBA. Native only.
#[cfg(not(target_arch = "wasm32"))]
pub fn to_rgba(state: &GameState, opts: &RenderOpts) -> Result<(usize, usize, Vec<u8>), String> {
    let pixmap = render_pixmap(state, opts)?;
    Ok((
        pixmap.width() as usize,
        pixmap.height() as usize,
        pixmap.data().to_vec(),
    ))
}

// ── Embedding the record inside the picture ──────────────────────────────────
//
// A PNG or SVG export can carry its own MSR record so the image *is* a save: PNG
// in a `tEXt` chunk (keyword `msr`), SVG in a `<metadata>` element. These are
// pure byte/string operations (no rendering), so the *reading* side also works
// on the web, which can't rasterise a PNG but can still pull a record out of one.

/// PNG keyword / SVG element id under which the record is stored.
const MSR_KEY: &str = "msr";
const SVG_MSR_ID: &str = "morpion-solitaire-record";

/// CRC-32 (ISO-HDLC, the PNG/zlib polynomial), computed table-free.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Splice a `tEXt` chunk holding `msr` into a PNG byte stream, just after IHDR.
/// `msr` is ASCII, so it is valid Latin-1 `tEXt` text.
pub fn embed_msr_png(png: &[u8], msr: &str) -> Vec<u8> {
    let mut data = Vec::with_capacity(MSR_KEY.len() + 1 + msr.len());
    data.extend_from_slice(MSR_KEY.as_bytes());
    data.push(0); // keyword/text separator
    data.extend_from_slice(msr.as_bytes());

    let mut chunk = Vec::with_capacity(12 + data.len());
    chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
    chunk.extend_from_slice(b"tEXt");
    chunk.extend_from_slice(&data);
    let mut crc_in = Vec::with_capacity(4 + data.len());
    crc_in.extend_from_slice(b"tEXt");
    crc_in.extend_from_slice(&data);
    chunk.extend_from_slice(&crc32(&crc_in).to_be_bytes());

    // Insert after the IHDR chunk: 8-byte signature + (len + type + data + crc).
    let insert_at = if png.len() >= 12 {
        let ihdr_len = u32::from_be_bytes([png[8], png[9], png[10], png[11]]) as usize;
        (8 + 12 + ihdr_len).min(png.len())
    } else {
        png.len()
    };
    let mut out = Vec::with_capacity(png.len() + chunk.len());
    out.extend_from_slice(&png[..insert_at]);
    out.extend_from_slice(&chunk);
    out.extend_from_slice(&png[insert_at..]);
    out
}

/// Pull the `msr` `tEXt` chunk out of a PNG, if present.
pub fn extract_msr_png(bytes: &[u8]) -> Option<String> {
    const SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 8 || &bytes[..8] != SIG {
        return None;
    }
    let mut pos = 8;
    while pos + 8 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[pos..pos + 4].try_into().ok()?) as usize;
        let ctype = &bytes[pos + 4..pos + 8];
        let dstart = pos + 8;
        let dend = dstart.checked_add(len)?;
        if dend + 4 > bytes.len() {
            break;
        }
        if ctype == b"tEXt" {
            let data = &bytes[dstart..dend];
            if let Some(nul) = data.iter().position(|&b| b == 0) {
                if &data[..nul] == MSR_KEY.as_bytes() {
                    return std::str::from_utf8(&data[nul + 1..])
                        .ok()
                        .map(str::to_owned);
                }
            }
        }
        if ctype == b"IEND" {
            break;
        }
        pos = dend + 4; // skip the chunk's CRC
    }
    None
}

/// Insert a `<metadata>` element holding `msr` as the first child of `<svg>`.
/// The record is base64url/JSON text with no XML-special characters, so it needs
/// no escaping (and would be illegal inside an XML comment, which forbids `--`).
pub fn embed_msr_svg(svg: &str, msr: &str) -> String {
    let meta = format!("<metadata id=\"{SVG_MSR_ID}\">{msr}</metadata>");
    match svg
        .find("<svg")
        .and_then(|s| svg[s..].find('>').map(|o| s + o + 1))
    {
        Some(after_open) => {
            let mut out = String::with_capacity(svg.len() + meta.len() + 1);
            out.push_str(&svg[..after_open]);
            out.push('\n');
            out.push_str(&meta);
            out.push_str(&svg[after_open..]);
            out
        }
        None => format!("{svg}\n{meta}"),
    }
}

/// Pull the record out of an SVG `<metadata>` element, if present.
pub fn extract_msr_svg(text: &str) -> Option<String> {
    let open = format!("id=\"{SVG_MSR_ID}\">");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find("</metadata>")?;
    Some(text[start..start + end].trim().to_owned())
}

#[cfg(test)]
mod embed_tests {
    use super::*;
    use crate::game::rules::Variant;
    use crate::game::state::GameState;

    #[test]
    fn svg_record_roundtrips() {
        let svg = to_svg(&GameState::new(Variant::T5), &RenderOpts { numbers: true });
        let msr = "MS1:abc-_DEF--ghi"; // note the "--": must survive (not a comment)
        let embedded = embed_msr_svg(&svg, msr);
        assert!(embedded.contains("<metadata"));
        assert_eq!(extract_msr_svg(&embedded).as_deref(), Some(msr));
        assert_eq!(extract_msr_svg(&svg), None); // a plain SVG carries nothing
    }

    // Exercise the splice against a real tiny-skia-encoded PNG (real IHDR, IDAT,
    // IEND), not a hand-built stub — guards against any encoder-specific layout.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn real_png_carries_record() {
        let st = GameState::new(Variant::T5);
        let png = to_png(&st, &RenderOpts { numbers: true }).unwrap();
        assert_eq!(extract_msr_png(&png), None);
        let payload = "MS1:real--payload_-with-url-safe-b64";
        let embedded = embed_msr_png(&png, payload);
        assert_eq!(extract_msr_png(&embedded).as_deref(), Some(payload));
        assert!(embedded.starts_with(b"\x89PNG\r\n\x1a\n".as_slice()));
    }

    #[test]
    fn png_record_roundtrips() {
        // A minimal but valid PNG: signature + IHDR + IEND (CRCs need not be real
        // for the chunk walker, which doesn't verify them).
        let mut png = Vec::new();
        png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&13u32.to_be_bytes());
        ihdr.extend_from_slice(b"IHDR");
        ihdr.extend_from_slice(&[0u8; 13]);
        ihdr.extend_from_slice(&[0u8; 4]);
        png.extend_from_slice(&ihdr);
        png.extend_from_slice(&0u32.to_be_bytes());
        png.extend_from_slice(b"IEND");
        png.extend_from_slice(&[0u8; 4]);

        let msr = "MS1:hello-_world";
        let embedded = embed_msr_png(&png, msr);
        assert_eq!(extract_msr_png(&embedded).as_deref(), Some(msr));
        assert_eq!(extract_msr_png(&png), None);
        assert_eq!(extract_msr_png(b"not a png"), None);
    }
}
