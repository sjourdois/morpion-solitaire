//! # MSR — Morpion Solitaire Record format
//!
//! A small, self-describing interchange format for [Morpion Solitaire] games:
//! the move list plus optional provenance (who/what/when produced it, how hard,
//! and a few derived facts), in either a compact `MS1:` envelope or plain JSON.
//! This crate is the reference reader, writer and **validator** for the format.
//!
//! It is deliberately self-contained — it does not depend on any solver — so any
//! tool can read, write and verify records with it. The published crate is
//! `morpion-solitaire-record`; it is imported as `msr`.
//!
//! ## Why MSR (vs. Pentasol)
//!
//! The community Pentasol text format covers only the 5T/5D variants and stores
//! moves alone. MSR is lossless for **all four** variants (4T/4D/5T/5D), carries
//! provenance metadata, and offers both a compact and a human-readable form. A
//! Pentasol bridge is provided for migration.
//!
//! ## Example
//!
//! ```
//! use msr::{Record, RecordMove, Variant, Direction};
//!
//! let moves = vec![RecordMove { x: 3, y: -1, dir: Direction::V, pos: 4 }];
//! let mut rec = Record::new(Variant::T5, moves);
//! rec.description = Some("a one-move demo".into());
//!
//! let compact = msr::encode(&rec).unwrap();      // "MS1:…"
//! let back = msr::decode(&compact).unwrap();
//! assert_eq!(rec, back);
//! ```
//!
//! ## Specification & references
//!
//! The normative format definition is the MSR specification shipped with the
//! source (`docs/spec/0.1/msr.md`) and the JSON Schema (`docs/spec/0.1/`). The rules
//! validated here are those of Morpion Solitaire; see the project bibliography
//! (`docs/BIBLIOGRAPHY.md`).
//!
//! [Morpion Solitaire]: https://en.wikipedia.org/wiki/Join_Five

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod codec;
mod cross;
mod model;
mod validate;

pub use codec::{decode, encode, encode_json, PREFIX};
pub use cross::initial_cross;
pub use model::{Direction, Record, RecordMove, Solver, Variant};
pub use validate::{validate, ValidationError};

/// The format version this crate reads and writes (the `version` field),
/// `major.minor`.
pub const FORMAT_VERSION: &str = "0.1";

/// The specification version implemented (`docs/spec/0.1/msr.md`).
pub const SPEC_VERSION: &str = "0.1";

/// Errors from encoding or decoding a record.
#[derive(Debug)]
pub enum Error {
    /// The `MS1:` payload was not valid Base64.
    Base64(String),
    /// The Base64 payload did not DEFLATE-decompress.
    Inflate(String),
    /// The JSON was malformed or did not match the schema.
    Json(serde_json::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Base64(e) => write!(f, "invalid Base64: {e}"),
            Error::Inflate(e) => write!(f, "invalid DEFLATE stream: {e}"),
            Error::Json(e) => write!(f, "invalid JSON: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}
