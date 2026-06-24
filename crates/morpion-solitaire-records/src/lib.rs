//! A curated corpus of Morpion Solitaire record games, embedded at compile time.
//!
//! Each entry is `(display name, id, MSR record)`. The `id` is the corpus file
//! stem (e.g. `"demaine31"`, matching `records/<variant>/demaine31.json`); the
//! record is its JSON source — decode it with the
//! [`morpion-solitaire-record`](https://crates.io/crates/morpion-solitaire-record)
//! crate (which reads both the JSON and the compact `MS1:` forms):
//!
//! ```ignore
//! for (name, id, record) in morpion_solitaire_records::RECORDS {
//!     let game = msr::decode(record).unwrap();
//!     assert!(msr::validate(&game).is_ok());
//!     println!("{name} [{id}]: {} moves", game.moves.len());
//! }
//! ```
//!
//! The JSON files are the **source of truth**; the compact `.msr`, the rendered
//! PNG/SVG (with the record embedded) and the Pentasol form are derived artifacts
//! generated from them (see the workspace's `gen_record_artifacts` example),
//! not committed. Being embedded, the corpus works without a filesystem (e.g. in
//! WebAssembly).
//!
//! [MSR]: https://crates.io/crates/morpion-solitaire-record

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// The record games, as `(display name, id, MSR JSON)` triples, best first. The
/// `id` is the corpus file stem. All are legal, terminal games (5T unless noted);
/// each carries its provenance in the record's `source`/`author`/`transcribed_by`
/// fields. See the crate README.
pub const RECORDS: &[(&str, &str, &str)] = &[
    (
        "Rosin 178",
        "rosin178",
        include_str!("../records/5T/rosin178.json"),
    ),
    (
        "Rosin 177A",
        "rosin177a",
        include_str!("../records/5T/rosin177a.json"),
    ),
    (
        "Rosin 177B",
        "rosin177b",
        include_str!("../records/5T/rosin177b.json"),
    ),
    (
        "Tishchenko 172",
        "tishchenko172",
        include_str!("../records/5T/tishchenko172.json"),
    ),
    (
        "Rosin 172",
        "rosin172",
        include_str!("../records/5T/rosin172.json"),
    ),
    (
        "Tishchenko 171",
        "tishchenko171",
        include_str!("../records/5T/tishchenko171.json"),
    ),
    (
        "Bruneau 170",
        "bruneau170",
        include_str!("../records/5T/bruneau170.json"),
    ),
    (
        "Rosin 170A",
        "rosin170a",
        include_str!("../records/5T/rosin170a.json"),
    ),
    (
        "Akiyama 146",
        "akiyama146",
        include_str!("../records/5T/akiyama146.json"),
    ),
    (
        "Akiyama 145",
        "akiyama145",
        include_str!("../records/5T/akiyama145.json"),
    ),
    (
        "smj 139 (self-found)",
        "smj139",
        include_str!("../records/5T/smj139.json"),
    ),
    (
        "Rosin 82 (5D)",
        "rosin82",
        include_str!("../records/5D/rosin82.json"),
    ),
    (
        "Hyyrö–Poranen 62 (4T)",
        "hyyroporanen62",
        include_str!("../records/4T/hyyroporanen62.json"),
    ),
    (
        "Demaine 56 (4T)",
        "demaine56",
        include_str!("../records/4T/demaine56.json"),
    ),
    (
        "Hyyrö–Poranen 35 (4D)",
        "hyyroporanen35",
        include_str!("../records/4D/hyyroporanen35.json"),
    ),
    (
        "Demaine 31 (4D)",
        "demaine31",
        include_str!("../records/4D/demaine31.json"),
    ),
];

#[cfg(test)]
mod tests {
    use super::RECORDS;

    /// Every embedded record decodes and is a legal game.
    #[test]
    fn all_records_decode_and_validate() {
        assert!(!RECORDS.is_empty());
        for (name, _id, record) in RECORDS {
            let game = msr::decode(record).unwrap_or_else(|e| panic!("{name}: decode failed: {e}"));
            msr::validate(&game).unwrap_or_else(|e| panic!("{name}: not a legal game: {e}"));
        }
    }
}
