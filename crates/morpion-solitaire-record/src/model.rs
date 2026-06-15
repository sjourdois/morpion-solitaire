//! The MSR data model: the on-disk record and its building blocks.
//!
//! The JSON field names and value encodings here ARE the wire format — changing
//! them is a format change. See the MSR specification (`docs/spec/0.1`).

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A game variant: line length (4 or 5) and touch rule (Touching / Disjoint).
///
/// Serialised as its two-character code, digit first: `"5T"`, `"5D"`, `"4T"`,
/// `"4D"` — the convention shared with the wider Morpion Solitaire community.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variant {
    /// 4 in a line, touching (parallel lines may share one endpoint).
    T4,
    /// 4 in a line, disjoint (parallel lines must be strictly separate).
    D4,
    /// 5 in a line, touching — the classic variant; world record 178.
    T5,
    /// 5 in a line, disjoint.
    D5,
}

impl Variant {
    /// Line length: 4 or 5.
    pub fn line_len(self) -> u8 {
        match self {
            Variant::T4 | Variant::D4 => 4,
            Variant::T5 | Variant::D5 => 5,
        }
    }

    /// Whether parallel collinear lines must be strictly disjoint (no shared
    /// point). When false, they may touch at a single endpoint.
    pub fn disjoint(self) -> bool {
        matches!(self, Variant::D4 | Variant::D5)
    }

    /// The canonical two-character code, digit first (`"5T"`).
    pub fn code(self) -> &'static str {
        match self {
            Variant::T4 => "4T",
            Variant::D4 => "4D",
            Variant::T5 => "5T",
            Variant::D5 => "5D",
        }
    }

    /// Parse a code, case-insensitively and in either order (`"5T"`/`"T5"`).
    pub fn from_code(s: &str) -> Option<Variant> {
        match s.to_ascii_uppercase().as_str() {
            "4T" | "T4" => Some(Variant::T4),
            "4D" | "D4" => Some(Variant::D4),
            "5T" | "T5" => Some(Variant::T5),
            "5D" | "D5" => Some(Variant::D5),
            _ => None,
        }
    }
}

impl fmt::Display for Variant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl Serialize for Variant {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.code())
    }
}

impl<'de> Deserialize<'de> for Variant {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = Variant;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a variant code such as \"5T\"")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Variant, E> {
                Variant::from_code(v).ok_or_else(|| E::custom(format!("unknown variant: {v}")))
            }
        }
        d.deserialize_str(V)
    }
}

/// Line direction. The unit step `delta()` defines the coordinate convention the
/// whole format depends on, so it is normative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Horizontal, step `(1, 0)`.
    H,
    /// Vertical, step `(0, 1)`.
    V,
    /// Diagonal "up", step `(1, -1)`.
    DP,
    /// Diagonal "down", step `(1, 1)`.
    DN,
}

impl Direction {
    /// All four directions.
    pub const ALL: [Direction; 4] = [Direction::H, Direction::V, Direction::DP, Direction::DN];

    /// Unit step along the line, smaller-coordinate end first. Normative.
    pub fn delta(self) -> (i16, i16) {
        match self {
            Direction::H => (1, 0),
            Direction::V => (0, 1),
            Direction::DP => (1, -1),
            Direction::DN => (1, 1),
        }
    }

    /// Wire code: `"H"`, `"V"`, `"DP"`, `"DN"`.
    pub fn code(self) -> &'static str {
        match self {
            Direction::H => "H",
            Direction::V => "V",
            Direction::DP => "DP",
            Direction::DN => "DN",
        }
    }

    /// Parse a wire code.
    pub fn from_code(s: &str) -> Option<Direction> {
        match s {
            "H" => Some(Direction::H),
            "V" => Some(Direction::V),
            "DP" => Some(Direction::DP),
            "DN" => Some(Direction::DN),
            _ => None,
        }
    }
}

impl Serialize for Direction {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.code())
    }
}

impl<'de> Deserialize<'de> for Direction {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = Direction;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a direction code: H, V, DP or DN")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Direction, E> {
                Direction::from_code(v).ok_or_else(|| E::custom(format!("unknown direction: {v}")))
            }
        }
        d.deserialize_str(V)
    }
}

/// One move: a new point `(x, y)` and the line it completes. `pos` is the index
/// of the new point within that line, in `0..line_len`; the line's origin is
/// therefore `(x, y) - pos * dir.delta()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordMove {
    /// X coordinate of the newly placed point.
    pub x: i16,
    /// Y coordinate of the newly placed point.
    pub y: i16,
    /// Direction of the completed line.
    pub dir: Direction,
    /// Index of the new point within the line, in `0..line_len`.
    pub pos: u8,
}

impl RecordMove {
    /// The line's origin (smaller-coordinate end): `(x, y) - pos * delta`.
    pub fn origin(&self) -> (i16, i16) {
        let (dx, dy) = self.dir.delta();
        (self.x - self.pos as i16 * dx, self.y - self.pos as i16 * dy)
    }

    /// The `line_len` points of the completed line, origin first.
    pub fn line_points(&self, line_len: u8) -> impl Iterator<Item = (i16, i16)> {
        let (ox, oy) = self.origin();
        let (dx, dy) = self.dir.delta();
        (0..line_len as i16).map(move |i| (ox + i * dx, oy + i * dy))
    }
}

/// A complete MSR record: the move list plus self-describing metadata.
///
/// Only `version`, `variant` and `moves` are essential; everything else is
/// optional provenance, omitted when empty. Unknown fields are ignored on read,
/// so the format is forward-compatible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// Format version, `major.minor` (e.g. `"0.1"`). A bare integer (legacy
    /// pre-0.1 files wrote `1`) is accepted on read as its decimal string.
    #[serde(default = "default_version", deserialize_with = "de_version")]
    pub version: String,
    /// Program that wrote the file, e.g. `"morpion-solitaire/0.1.0"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer: Option<String>,
    /// Game variant.
    pub variant: Variant,
    /// Number of moves (equals `moves.len()`; stored for readability).
    pub score: usize,
    /// Legal moves still available at the final position (`0` ⇔ terminal).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_moves: Option<usize>,
    /// Whether the final position is terminal (no legal move remains).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal: Option<bool>,
    /// Bounding box of all placed points: `[min_x, min_y, max_x, max_y]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bbox: Option<[i16; 4]>,
    /// Save time as an ISO-8601 UTC string (e.g. `"2026-06-14T10:30:00Z"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_at: Option<String>,
    /// Free-text human description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Who produced or owns the game (person, team, handle).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Where the record originally came from: a provenance URL or citation
    /// (e.g. the original record site, or the Pentasol file it was imported
    /// from).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Who transcribed the game into MSR form (curator/project), as distinct
    /// from `source` (where the game itself originates) and `author` (who set
    /// the record). E.g. `"morpion-solitaire.io"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcribed_by: Option<String>,
    /// Free-form labels (e.g. `"world-record"`, `"candidate"`, `"verified"`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Machine-search provenance, present only when a solver produced the game.
    /// Absent for human / hand-played / transcribed records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solver: Option<Solver>,
    /// The moves, in play order.
    pub moves: Vec<RecordMove>,
}

/// Provenance of the automated search that produced a game. Every field is
/// optional; the block as a whole is omitted for records not made by a solver.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Solver {
    /// The search tool/engine that produced the game (a name or brand, e.g.
    /// `"morpion-solitaire.io"`). Distinct from the file-level `producer`, which
    /// is the program that wrote the file (`name/version`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Method + parameters that produced the game, e.g. `"nrpa L3"` or
    /// `"nrpa-seeded L3 warm-from=178"`. Does not carry the RNG `seed`, which
    /// has its own field below.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// RNG seed, for reproducibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// Search effort that produced the game, in nodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodes_explored: Option<u64>,
    /// Wall-clock seconds of the producing search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_secs: Option<f64>,
}

impl Solver {
    /// Whether every field is empty (so the block should be omitted entirely).
    pub fn is_empty(&self) -> bool {
        self.tool.is_none()
            && self.method.is_none()
            && self.seed.is_none()
            && self.nodes_explored.is_none()
            && self.elapsed_secs.is_none()
    }
}

fn default_version() -> String {
    crate::FORMAT_VERSION.to_owned()
}

/// Accept the `version` field as a `major.minor` string or, for backward
/// compatibility with pre-0.1 files, a bare number (rendered to its string).
fn de_version<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    struct V;
    impl Visitor<'_> for V {
        type Value = String;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a version string like \"0.1\" or an integer")
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<String, E> {
            Ok(v.to_owned())
        }
        fn visit_string<E: de::Error>(self, v: String) -> Result<String, E> {
            Ok(v)
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<String, E> {
            Ok(v.to_string())
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<String, E> {
            Ok(v.to_string())
        }
        fn visit_f64<E: de::Error>(self, v: f64) -> Result<String, E> {
            Ok(v.to_string())
        }
    }
    d.deserialize_any(V)
}

impl Record {
    /// A minimal record for `variant` and `moves` (version 1, `score` set,
    /// metadata empty). Fill the public metadata fields as needed.
    pub fn new(variant: Variant, moves: Vec<RecordMove>) -> Record {
        Record {
            version: default_version(),
            producer: None,
            variant,
            score: moves.len(),
            available_moves: None,
            terminal: None,
            bbox: None,
            saved_at: None,
            description: None,
            author: None,
            source: None,
            transcribed_by: None,
            tags: Vec::new(),
            solver: None,
            moves,
        }
    }
}
