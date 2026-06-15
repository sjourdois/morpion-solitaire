//! Import / export of game sequences.
//!
//! The portable record format is **MSR**, implemented by the standalone
//! [`msr`](https://crates.io/crates/morpion-solitaire-record) crate; this module
//! only converts between the engine's [`GameState`] and [`msr::Record`]. Two
//! other formats live here because they are application-internal or legacy:
//! - **Search checkpoint** (`MSC1:`) — resumable solver state, not a public format.
//! - **Pentasol text** — the community 5T/5D format ([Boyer]) MSR supersedes
//!   (kept for import/export so existing corpora migrate).
//!
//! [Boyer]: https://github.com/sjourdois/morpion-solitaire/blob/main/docs/BIBLIOGRAPHY.md

use crate::game::{
    line::{Dir, Line},
    moves::{legal_moves, Move},
    rules::Variant,
    state::GameState,
};
use base64::Engine as _;
use serde::{Deserialize, Serialize};

// ── MSR records (via the `msr` crate) ────────────────────────────────────────

/// Caller-supplied provenance attached to a save. Derived facts (score,
/// terminal, available moves, bounding box, producer, timestamps) are filled in
/// by the exporter from the game state — only the human/search context lives
/// here. `Default` yields an empty metadata set (an anonymous save).
#[derive(Debug, Default, Clone)]
pub struct SaveMeta {
    pub description: Option<String>,
    pub author: Option<String>,
    pub source: Option<String>,
    pub transcribed_by: Option<String>,
    pub tool: Option<String>,
    pub method: Option<String>,
    pub seed: Option<u64>,
    pub nodes_explored: Option<u64>,
    pub elapsed_secs: Option<f64>,
    pub tags: Vec<String>,
}

/// Provenance read back from a save, for display by `replay`/`records`/`show`
/// without re-deriving it. Returned alongside the replayed [`GameState`].
#[derive(Debug, Default, Clone)]
pub struct SaveInfo {
    pub producer: Option<String>,
    pub saved_at: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub source: Option<String>,
    pub transcribed_by: Option<String>,
    pub tool: Option<String>,
    pub method: Option<String>,
    pub seed: Option<u64>,
    pub nodes_explored: Option<u64>,
    pub elapsed_secs: Option<f64>,
    pub tags: Vec<String>,
}

/// Program identifier stamped into every save (`name/version` from Cargo).
fn producer_string() -> String {
    concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")).to_owned()
}

fn dir_to_msr(d: Dir) -> msr::Direction {
    match d {
        Dir::H => msr::Direction::H,
        Dir::V => msr::Direction::V,
        Dir::DP => msr::Direction::DP,
        Dir::DN => msr::Direction::DN,
    }
}

fn dir_from_msr(d: msr::Direction) -> Dir {
    match d {
        msr::Direction::H => Dir::H,
        msr::Direction::V => Dir::V,
        msr::Direction::DP => Dir::DP,
        msr::Direction::DN => Dir::DN,
    }
}

fn variant_to_msr(v: Variant) -> msr::Variant {
    msr::Variant::from_code(v.name()).expect("variant name is a valid MSR code")
}

fn variant_from_msr(v: msr::Variant) -> Variant {
    Variant::from_name(v.code()).expect("MSR variant code is a valid variant name")
}

fn move_to_record(mv: &Move) -> msr::RecordMove {
    msr::RecordMove {
        x: mv.pos.0,
        y: mv.pos.1,
        dir: dir_to_msr(mv.line.dir),
        pos: mv.line_pos,
    }
}

fn record_to_move(rm: &msr::RecordMove) -> Move {
    let dir = dir_from_msr(rm.dir);
    let line = Line::from_point((rm.x, rm.y), dir, rm.pos, 0 /* not used */);
    Move::new((rm.x, rm.y), line, rm.pos)
}

/// Build an [`msr::Record`] from a game, deriving the objective facts (producer,
/// score, terminal/available, bounding box, ISO date) and attaching `meta`.
fn record_from_state(state: &GameState, saved_at_unix: u64, meta: &SaveMeta) -> msr::Record {
    let available = legal_moves(state).len();
    let mut r = msr::Record::new(
        variant_to_msr(state.variant),
        state.history.iter().map(move_to_record).collect(),
    );
    r.producer = Some(producer_string());
    r.available_moves = Some(available);
    r.terminal = Some(available == 0);
    r.bbox = state.bounding_box().map(|(a, b, c, d)| [a, b, c, d]);
    r.saved_at = Some(format_iso8601(saved_at_unix));
    r.description = meta.description.clone();
    r.author = meta.author.clone();
    r.source = meta.source.clone();
    r.transcribed_by = meta.transcribed_by.clone();
    r.tags = meta.tags.clone();
    let solver = msr::Solver {
        tool: meta.tool.clone(),
        method: meta.method.clone(),
        seed: meta.seed,
        nodes_explored: meta.nodes_explored,
        elapsed_secs: meta.elapsed_secs,
    };
    r.solver = (!solver.is_empty()).then_some(solver);
    r
}

/// Replay an [`msr::Record`] into a [`GameState`] and surface its metadata.
fn state_from_record(record: &msr::Record) -> (GameState, SaveInfo) {
    let mut state = GameState::new(variant_from_msr(record.variant));
    for rm in &record.moves {
        state.apply(record_to_move(rm));
    }
    let solver = record.solver.as_ref();
    let info = SaveInfo {
        producer: record.producer.clone(),
        saved_at: record.saved_at.clone(),
        description: record.description.clone(),
        author: record.author.clone(),
        source: record.source.clone(),
        transcribed_by: record.transcribed_by.clone(),
        tool: solver.and_then(|s| s.tool.clone()),
        method: solver.and_then(|s| s.method.clone()),
        seed: solver.and_then(|s| s.seed),
        nodes_explored: solver.and_then(|s| s.nodes_explored),
        elapsed_secs: solver.and_then(|s| s.elapsed_secs),
        tags: record.tags.clone(),
    };
    (state, info)
}

/// Current wall-clock time as seconds since the Unix epoch.
/// Uses `web-time`, so it works on both native and WASM targets.
pub fn unix_now() -> u64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Serialise the current game to a compact `MS1:` save string, stamped with the
/// current time. See [`export_save_at`].
pub fn export_save(state: &GameState) -> Result<String, msr::Error> {
    export_save_at(state, unix_now())
}

/// Serialise the current game to a compact `MS1:` save string stamped with the
/// given Unix timestamp. Far shorter than raw JSON, still lossless.
pub fn export_save_at(state: &GameState, saved_at_unix: u64) -> Result<String, msr::Error> {
    export_save_with_method(state, saved_at_unix, None)
}

/// Like [`export_save_at`] but also records the `method` (provenance string)
/// that produced the game — used when persisting a record.
pub fn export_save_with_method(
    state: &GameState,
    saved_at_unix: u64,
    method: Option<String>,
) -> Result<String, msr::Error> {
    export_save_with_meta(
        state,
        saved_at_unix,
        &SaveMeta {
            method,
            ..Default::default()
        },
    )
}

/// Serialise a game to the compact `MS1:` format with full provenance.
pub fn export_save_with_meta(
    state: &GameState,
    saved_at_unix: u64,
    meta: &SaveMeta,
) -> Result<String, msr::Error> {
    msr::encode(&record_from_state(state, saved_at_unix, meta))
}

/// Same content as [`export_save_with_meta`] but as human-readable, pretty JSON
/// (the uncompressed form `import_save` also accepts). For `convert --to json`.
pub fn export_json_with_meta(
    state: &GameState,
    saved_at_unix: u64,
    meta: &SaveMeta,
) -> Result<String, msr::Error> {
    msr::encode_json(&record_from_state(state, saved_at_unix, meta))
}

/// Format a Unix timestamp (seconds) as an ISO-8601 UTC string,
/// e.g. `2026-06-13T11:02:31Z`. Pure integer arithmetic, no extra deps.
fn format_iso8601(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let tod = secs % 86_400;
    let (h, mi, s) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Convert a count of days since 1970-01-01 into (year, month, day).
/// Howard Hinnant's `civil_from_days` algorithm ([Hinnant]).
///
/// [Hinnant]: https://github.com/sjourdois/morpion-solitaire/blob/main/docs/BIBLIOGRAPHY.md
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Restore a game from an MSR save string (compact `MS1:` or raw JSON).
pub fn import_save(text: &str) -> Result<GameState, String> {
    import_save_with_info(text).map(|(state, _)| state)
}

/// Like [`import_save`] but also returns the file's provenance metadata
/// ([`SaveInfo`]) for display, without re-deriving it.
pub fn import_save_with_info(text: &str) -> Result<(GameState, SaveInfo), String> {
    let record = msr::decode(text).map_err(|e| e.to_string())?;
    Ok(state_from_record(&record))
}

// ── Search checkpoint (systematic + NRPA) ────────────────────────────────────

/// Tag for the search-checkpoint format (`MSC1:` + Base64 of DEFLATEd JSON).
const CHECKPOINT_PREFIX: &str = "MSC1:";

/// Default algorithm tag for checkpoints written before the tag existed.
fn default_algo_tag() -> String {
    "systematic".to_owned()
}

#[derive(Debug, Serialize, Deserialize)]
struct CheckpointFile {
    version: u8,
    variant: String,
    saved_at_unix: u64,
    nodes_explored: u64,
    /// Which search produced this checkpoint ("systematic" | "nrpa"); resume
    /// dispatches on it. Defaults to systematic for pre-tag checkpoints.
    #[serde(default = "default_algo_tag")]
    algo: String,
    /// Best sequence found so far. Reuses the MSR move encoding.
    best: Vec<msr::RecordMove>,
    /// Record improvements: (score, elapsed milliseconds).
    records: Vec<(u32, u64)>,
    /// The search frontier: each entry is a move sequence to an unexplored
    /// subtree root. Replaying it reconstructs that node exactly. Empty for
    /// NRPA, which has no deterministic frontier (only the best is preserved).
    frontier: Vec<Vec<msr::RecordMove>>,
}

/// Restored search checkpoint. `algo` selects which engine resumes it.
pub struct Checkpoint {
    pub variant: Variant,
    pub nodes_explored: u64,
    pub algo: String,
    pub best: Vec<Move>,
    pub records: Vec<(u32, std::time::Duration)>,
    pub frontier: Vec<Vec<Move>>,
}

/// Serialise a systematic-search checkpoint to the compact `MSC1:` format.
/// Uses DEFLATE level 6 (the frontier can be large; level 6 keeps saves fast).
#[allow(clippy::too_many_arguments)]
pub fn export_checkpoint(
    variant: Variant,
    nodes_explored: u64,
    best: &[Move],
    records: &[(u32, std::time::Duration)],
    frontier: &[Vec<Move>],
    algo: &str,
    saved_at_unix: u64,
) -> Result<String, serde_json::Error> {
    let file = CheckpointFile {
        version: 1,
        variant: variant.name().to_owned(),
        saved_at_unix,
        nodes_explored,
        algo: algo.to_owned(),
        best: best.iter().map(move_to_record).collect(),
        records: records
            .iter()
            .map(|(s, d)| (*s, d.as_millis() as u64))
            .collect(),
        frontier: frontier
            .iter()
            .map(|seq| seq.iter().map(move_to_record).collect())
            .collect(),
    };
    let json = serde_json::to_vec(&file)?;
    let compressed = miniz_oxide::deflate::compress_to_vec(&json, 6);
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(compressed);
    Ok(format!("{CHECKPOINT_PREFIX}{b64}"))
}

/// Restore a checkpoint produced by [`export_checkpoint`].
pub fn import_checkpoint(text: &str) -> Result<Checkpoint, String> {
    let b64 = text
        .trim()
        .strip_prefix(CHECKPOINT_PREFIX)
        .ok_or("not a search checkpoint (missing MSC1: tag)")?;
    let compressed = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(b64.trim())
        .map_err(|e| format!("base64: {e}"))?;
    let json = miniz_oxide::inflate::decompress_to_vec(&compressed)
        .map_err(|e| format!("inflate: {e:?}"))?;
    let file: CheckpointFile = serde_json::from_slice(&json).map_err(|e| e.to_string())?;

    let variant = Variant::from_name(&file.variant)
        .ok_or_else(|| format!("unknown variant: {}", file.variant))?;
    let to_seq =
        |moves: &[msr::RecordMove]| -> Vec<Move> { moves.iter().map(record_to_move).collect() };
    Ok(Checkpoint {
        variant,
        nodes_explored: file.nodes_explored,
        algo: file.algo,
        best: to_seq(&file.best),
        records: file
            .records
            .iter()
            .map(|(s, ms)| (*s, std::time::Duration::from_millis(*ms)))
            .collect(),
        frontier: file.frontier.iter().map(|s| to_seq(s)).collect(),
    })
}

// ── Pentasol text ──────────────────────────────────────────────────────────

/// Column / row offset applied when converting from internal coordinates
/// to Pentasol 1-indexed coordinates.
/// Internal coords now start at 0, so Pentasol col/row = internal + 1.
/// For 5T/5D: cross fits in cols/rows 1–10  (internal 0–9).
/// For 4T/4D: cross fits in cols/rows 1–8   (internal 0–7).
fn pentasol_offset(_variant: Variant) -> (i16, i16) {
    (1, 1)
}

/// Export in Pentasol-compatible text format (one move per line).
/// Returns an error for variants other than 5T/5D, though the format
/// does extend naturally to 4T/4D with appropriate centerdist.
pub fn export_pentasol(state: &GameState) -> String {
    let (ox, oy) = pentasol_offset(state.variant);
    let half = (state.variant.len() as i8 - 1) / 2; // 2 for len=5, 1 for len=4

    let dir_char = |d: Dir| match d {
        Dir::H => '-',
        Dir::V => '|',
        Dir::DP => '/',
        Dir::DN => '\\',
    };

    state
        .history
        .iter()
        .map(|mv| {
            let col = mv.pos.0 + ox;
            let row = mv.pos.1 + oy;
            let centerdist = mv.line_pos as i8 - half;
            format!("({col},{row}){}{centerdist:+}", dir_char(mv.line.dir))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Import a Pentasol-format string and replay the moves.
/// `variant` must be provided because the file format doesn't record it.
pub fn import_pentasol(text: &str, variant: Variant) -> Result<GameState, String> {
    let (ox, oy) = pentasol_offset(variant);
    let half = (variant.len() as i8 - 1) / 2;

    let mut state = GameState::new(variant);

    for (line_no, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mv = parse_pentasol_move(line, ox, oy, half, variant.len())
            .map_err(|e| format!("line {}: {e}", line_no + 1))?;
        state.apply(mv);
    }
    Ok(state)
}

fn parse_pentasol_move(s: &str, ox: i16, oy: i16, half: i8, _line_len: u8) -> Result<Move, String> {
    // Format: `(col,row)dir±centerdist`
    // e.g.  `(9,7)-+2`  or  `(12,8)|0`
    let s = s.trim();
    if !s.starts_with('(') {
        return Err(format!("expected '(' at start, got: {s}"));
    }
    let close = s.find(')').ok_or("missing ')'")?;
    let coords = &s[1..close];
    let rest = &s[close + 1..];

    let (col_s, row_s) = coords.split_once(',').ok_or("missing ',' in coordinates")?;
    let col: i16 = col_s.trim().parse().map_err(|_| "bad column")?;
    let row: i16 = row_s.trim().parse().map_err(|_| "bad row")?;
    let x = col - ox;
    let y = row - oy;

    if rest.is_empty() {
        return Err("missing direction".to_owned());
    }
    let (dir, centerdist_s) = match rest.chars().next().unwrap() {
        '-' => (Dir::H, &rest[1..]),
        '|' => (Dir::V, &rest[1..]),
        '/' => (Dir::DP, &rest[1..]),
        '\\' => (Dir::DN, &rest[1..]),
        c => return Err(format!("unknown direction char: {c}")),
    };

    let centerdist: i8 = if centerdist_s.is_empty() {
        0
    } else {
        centerdist_s.parse().map_err(|_| "bad centerdist")?
    };

    let line_pos = (centerdist + half) as u8;
    let line = Line::from_point((x, y), dir, line_pos, 0);
    Ok(Move::new((x, y), line, line_pos))
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{moves::legal_moves, rules::Variant, state::GameState};

    fn play_n(variant: Variant, n: usize) -> GameState {
        let mut state = GameState::new(variant);
        for _ in 0..n {
            let moves = legal_moves(&state);
            if moves.is_empty() {
                break;
            }
            state.apply(moves[0]);
        }
        state
    }

    /// One-off: convert morpionsolitaire.com Pentasol record files (reference
    /// point, spaces, shifted coordinate frame) into the `.msr` corpus with
    /// provenance. Robust against the unknown coordinate convention: parse each
    /// move as (col,row,dir,centerdist), then brute-force the translation offset
    /// and centerdist sign by REPLAYING the whole game on a fresh cross of the
    /// right variant and requiring every move to be uniquely legal. Fetch the
    /// sources first, e.g.:
    ///   for g in Grid5T178Rosin Grid5D82Rosin; do
    ///     curl -o /tmp/$g.txt http://morpionsolitaire.com/$g.txt; done
    #[test]
    #[ignore = "one-off corpus conversion; run with --ignored --nocapture"]
    fn convert_site_records_to_msr() {
        use crate::game::line::Dir;
        // (site grid name, output basename, score, variant, author)
        let entries = [
            (
                "Grid5T145Akiyama",
                "akiyama145",
                145usize,
                Variant::T5,
                "Akiyama",
            ),
            (
                "Grid5T146Akiyama",
                "akiyama146",
                146,
                Variant::T5,
                "Akiyama",
            ),
            (
                "Grid5T170Bruneau",
                "bruneau170",
                170,
                Variant::T5,
                "Charles-Henri Bruneau",
            ),
            (
                "Grid5T170RosinA",
                "rosin170a",
                170,
                Variant::T5,
                "Christopher D. Rosin",
            ),
            (
                "Grid5T171Tishchenko",
                "tishchenko171",
                171,
                Variant::T5,
                "Tishchenko",
            ),
            (
                "Grid5T172Rosin",
                "rosin172",
                172,
                Variant::T5,
                "Christopher D. Rosin",
            ),
            (
                "Grid5T172Tishchenko",
                "tishchenko172",
                172,
                Variant::T5,
                "Tishchenko",
            ),
            (
                "Grid5T177RosinA",
                "rosin177a",
                177,
                Variant::T5,
                "Christopher D. Rosin",
            ),
            (
                "Grid5T177RosinB",
                "rosin177b",
                177,
                Variant::T5,
                "Christopher D. Rosin",
            ),
            (
                "Grid5T178Rosin",
                "rosin178",
                178,
                Variant::T5,
                "Christopher D. Rosin",
            ),
            (
                "Grid5D82Rosin",
                "rosin82",
                82,
                Variant::D5,
                "Christopher D. Rosin",
            ),
        ];
        for (grid, base, expect, variant, author) in entries {
            let input = format!("/tmp/{grid}.txt");
            let dir = format!("../morpion-solitaire-records/records/{}", variant.name());
            std::fs::create_dir_all(&dir).unwrap();
            let output = format!("{dir}/{base}.msr");
            let source = format!("http://morpionsolitaire.com/{grid}.txt");
            let text = std::fs::read_to_string(&input).unwrap_or_else(|_| panic!("read {input}"));
            let mut parsed: Vec<(i16, i16, char, i8)> = Vec::new();
            for line in text.lines() {
                let line = line.trim();
                if !line.starts_with('(') {
                    continue;
                }
                let close = line.find(')').unwrap();
                let (c, r) = line[1..close].split_once(',').unwrap();
                let col: i16 = c.trim().parse().unwrap();
                let row: i16 = r.trim().parse().unwrap();
                let rest = line[close + 1..].trim();
                if let Some(ch @ ('-' | '|' | '/' | '\\')) = rest.chars().next() {
                    let cd: i8 = rest[1..].trim().parse().unwrap_or(0);
                    parsed.push((col, row, ch, cd));
                } // else: reference point or blank
            }
            assert_eq!(
                parsed.len(),
                expect,
                "{input}: parsed {} moves",
                parsed.len()
            );

            let id_dir = |ch: char| match ch {
                '-' => Dir::H,
                '|' => Dir::V,
                '/' => Dir::DP,
                _ => Dir::DN,
            };
            let half = (variant.len() as i16 - 1) / 2;
            let max_lp = variant.len() as i16 - 1;
            let try_replay = |cdsign: i8, dx: i16, dy: i16| -> Option<GameState> {
                let mut st = GameState::new(variant);
                for &(col, row, ch, cd) in &parsed {
                    let pos = (col + dx, row + dy);
                    let lp = half + cdsign as i16 * cd as i16;
                    if !(0..=max_lp).contains(&lp) {
                        return None;
                    }
                    let (dir, lp) = (id_dir(ch), lp as u8);
                    let mv = legal_moves(&st)
                        .into_iter()
                        .find(|m| m.pos == pos && m.line.dir == dir && m.line_pos == lp)?;
                    st.apply(mv);
                }
                Some(st)
            };

            let mut found = None;
            'search: for &cdsign in &[-1i8, 1] {
                for dx in -40..=0 {
                    for dy in -40..=0 {
                        if let Some(st) = try_replay(cdsign, dx, dy) {
                            found = Some(st);
                            break 'search;
                        }
                    }
                }
            }
            let st = found.unwrap_or_else(|| panic!("{input}: no offset replayed all moves"));
            assert_eq!(st.score(), expect);
            assert!(legal_moves(&st).is_empty(), "{output} must be terminal");
            let meta = SaveMeta {
                author: Some(author.to_owned()),
                source: Some(source),
                description: Some(format!(
                    "{} record · {} moves · {}",
                    variant.name(),
                    expect,
                    author
                )),
                ..Default::default()
            };
            std::fs::write(
                &output,
                export_save_with_meta(&st, unix_now(), &meta).unwrap() + "\n",
            )
            .unwrap();
            println!("wrote {output} (score {})", st.score());
        }
    }

    #[test]
    fn save_roundtrip_5t() {
        let original = play_n(Variant::T5, 10);
        let blob = export_save(&original).unwrap();
        assert!(blob.starts_with("MS1:"));
        let restored = import_save(&blob).unwrap();
        assert_eq!(restored.score(), original.score());
        assert_eq!(restored.history, original.history);
    }

    #[test]
    fn save_roundtrip_4d() {
        let original = play_n(Variant::D4, 5);
        let blob = export_save(&original).unwrap();
        let restored = import_save(&blob).unwrap();
        assert_eq!(restored.score(), original.score());
        assert_eq!(restored.history, original.history);
    }

    #[test]
    fn import_save_accepts_legacy_json() {
        // Older saves were raw JSON with only the essential fields (no producer,
        // metadata, or derived facts). Such a minimal object must still read via
        // serde defaults, guarding back-compat with files already on disk.
        let original = play_n(Variant::T5, 6);
        let moves: Vec<_> = original
            .history
            .iter()
            .map(|m| serde_json::to_value(move_to_record(m)).unwrap())
            .collect();
        let legacy = serde_json::json!({
            "version": 1,
            "variant": original.variant.name(),
            "score": original.score(),
            "moves": moves,
        });
        let json = serde_json::to_string(&legacy).unwrap();
        let (restored, info) = import_save_with_info(&json).unwrap();
        assert_eq!(restored.history, original.history);
        assert!(info.producer.is_none()); // absent in legacy files
    }

    #[test]
    fn save_roundtrip_preserves_metadata() {
        let original = play_n(Variant::T5, 9);
        let meta = SaveMeta {
            description: Some("test game".to_owned()),
            author: Some("tester".to_owned()),
            source: Some("http://example.org/g.txt".to_owned()),
            transcribed_by: Some("morpion-solitaire.io".to_owned()),
            tool: Some("morpion-solitaire".to_owned()),
            method: Some("nrpa L3".to_owned()),
            seed: Some(42),
            nodes_explored: Some(123_456),
            elapsed_secs: Some(1.5),
            tags: vec!["candidate".to_owned()],
        };
        let blob = export_save_with_meta(&original, 1_700_000_000, &meta).unwrap();
        let (restored, info) = import_save_with_info(&blob).unwrap();
        assert_eq!(restored.history, original.history);
        assert_eq!(info.description.as_deref(), Some("test game"));
        assert_eq!(info.author.as_deref(), Some("tester"));
        assert_eq!(info.source.as_deref(), Some("http://example.org/g.txt"));
        assert_eq!(info.transcribed_by.as_deref(), Some("morpion-solitaire.io"));
        assert_eq!(info.tool.as_deref(), Some("morpion-solitaire"));
        assert_eq!(info.method.as_deref(), Some("nrpa L3"));
        assert_eq!(info.seed, Some(42));
        assert_eq!(info.nodes_explored, Some(123_456));
        assert_eq!(info.tags, vec!["candidate".to_owned()]);
        assert!(info.producer.unwrap().starts_with("morpion-solitaire/"));
        assert_eq!(info.saved_at.as_deref(), Some("2023-11-14T22:13:20Z"));
    }

    #[test]
    fn pentasol_roundtrip_5t() {
        let original = play_n(Variant::T5, 8);
        let text = export_pentasol(&original);
        let restored = import_pentasol(&text, Variant::T5).unwrap();
        assert_eq!(restored.score(), original.score());
        assert_eq!(restored.history, original.history);
    }

    #[test]
    fn pentasol_roundtrip_5d() {
        let original = play_n(Variant::D5, 8);
        let text = export_pentasol(&original);
        let restored = import_pentasol(&text, Variant::D5).unwrap();
        assert_eq!(restored.score(), original.score());
    }
}
