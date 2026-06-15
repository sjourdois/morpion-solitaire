//! Format conformance tests, including decoding a real-world record.

use msr::{decode, encode, encode_json, validate, Direction, Record, RecordMove, Solver, Variant};

#[test]
fn variant_and_direction_codes_roundtrip() {
    for v in [Variant::T4, Variant::D4, Variant::T5, Variant::D5] {
        assert_eq!(Variant::from_code(v.code()), Some(v));
    }
    assert_eq!(Variant::from_code("t5"), Some(Variant::T5)); // case + order insensitive
    for d in Direction::ALL {
        assert_eq!(Direction::from_code(d.code()), Some(d));
    }
}

#[test]
fn cross_sizes() {
    assert_eq!(msr::initial_cross(Variant::T5).len(), 36);
    // 4T/4D: a centred D4-symmetric Greek cross on a 0..=6 grid.
    assert_eq!(msr::initial_cross(Variant::T4).len(), 24);
}

/// The initial cross is D4-symmetric on its `0..=w` grid (spec §3.1). Invariance
/// under an axis reflection and the main-diagonal transpose generates all of D4,
/// and pins the cross to the centred form the spec describes.
#[test]
fn initial_cross_is_d4_symmetric() {
    use std::collections::HashSet;
    for v in [Variant::T5, Variant::T4] {
        let pts: HashSet<(i16, i16)> = msr::initial_cross(v).into_iter().collect();
        let w = pts.iter().flat_map(|&(x, y)| [x, y]).max().unwrap();
        let reflect_x: HashSet<(i16, i16)> = pts.iter().map(|&(x, y)| (w - x, y)).collect();
        let transpose: HashSet<(i16, i16)> = pts.iter().map(|&(x, y)| (y, x)).collect();
        assert_eq!(
            pts, reflect_x,
            "{v}: cross not symmetric under x-reflection"
        );
        assert_eq!(pts, transpose, "{v}: cross not symmetric under transpose");
    }
}

#[test]
fn compact_and_json_roundtrip() {
    let moves = vec![
        RecordMove {
            x: 3,
            y: -1,
            dir: Direction::V,
            pos: 4,
        },
        RecordMove {
            x: -1,
            y: 3,
            dir: Direction::H,
            pos: 4,
        },
    ];
    let mut rec = Record::new(Variant::T5, moves);
    rec.description = Some("demo".into());
    rec.tags = vec!["candidate".into()];

    let compact = encode(&rec).unwrap();
    assert!(compact.starts_with("MS1:"));
    assert_eq!(decode(&compact).unwrap(), rec);

    let json = encode_json(&rec).unwrap();
    assert!(json.contains("\"variant\": \"5T\""));
    assert_eq!(decode(&json).unwrap(), rec);
}

#[test]
fn solver_block_is_omitted_for_human_records_and_roundtrips_for_machine_ones() {
    let moves = vec![RecordMove {
        x: 3,
        y: -1,
        dir: Direction::V,
        pos: 4,
    }];

    // Human record: editorial provenance, no solver block.
    let mut human = Record::new(Variant::T5, moves.clone());
    human.author = Some("C. Rosin".into());
    human.source = Some("morpionsolitaire.com".into());
    human.transcribed_by = Some("morpion-solitaire.io".into());
    let json = encode_json(&human).unwrap();
    assert!(!json.contains("solver"), "human record must omit `solver`");
    assert!(json.contains("\"transcribed_by\": \"morpion-solitaire.io\""));
    assert_eq!(decode(&json).unwrap(), human);
    assert_eq!(decode(&encode(&human).unwrap()).unwrap(), human);

    // Machine record: nested solver block, seed lives only there.
    let mut machine = Record::new(Variant::T5, moves);
    machine.solver = Some(Solver {
        tool: Some("morpion-solitaire.io".into()),
        method: Some("nrpa L3".into()),
        seed: Some(42),
        nodes_explored: Some(123_456),
        elapsed_secs: Some(1.5),
    });
    let json = encode_json(&machine).unwrap();
    assert!(json.contains("\"solver\""));
    assert!(json.contains("\"tool\": \"morpion-solitaire.io\""));
    assert!(json.contains("\"seed\": 42"));
    assert_eq!(decode(&json).unwrap(), machine);
    assert_eq!(decode(&encode(&machine).unwrap()).unwrap(), machine);
}

#[test]
fn version_is_major_minor_and_accepts_legacy_integer() {
    // New records carry the current format version as a major.minor string.
    let rec = Record::new(Variant::T5, vec![]);
    assert_eq!(rec.version, msr::FORMAT_VERSION);
    assert_eq!(rec.version, "0.1");
    assert!(encode_json(&rec).unwrap().contains("\"version\": \"0.1\""));

    // A pre-0.1 file that wrote a bare integer still decodes (as its string).
    let legacy = r#"{"version":1,"variant":"5T","score":0,"moves":[]}"#;
    assert_eq!(decode(legacy).unwrap().version, "1");
    // And a string version round-trips unchanged.
    let v02 = r#"{"version":"0.2","variant":"5T","score":0,"moves":[]}"#;
    assert_eq!(decode(v02).unwrap().version, "0.2");
}

#[test]
fn decodes_minimal_legacy_json() {
    // Only the essential fields; metadata absent. Must parse via serde defaults.
    let json = r#"{"version":1,"variant":"5T","score":0,"moves":[]}"#;
    let rec = decode(json).unwrap();
    assert_eq!(rec.variant, Variant::T5);
    assert!(rec.producer.is_none());
}

/// The committed world-record file must decode and validate with this crate
/// alone — the standard reads real-world records, independent of the solver.
#[test]
fn decodes_and_validates_rosin_178() {
    let text = include_str!("fixtures/rosin178.msr");
    let rec = decode(text).expect("rosin178.msr should decode");
    assert_eq!(rec.variant, Variant::T5);
    assert_eq!(rec.moves.len(), 178);
    validate(&rec).expect("the 178-move record must be legal");
}

#[test]
fn validator_rejects_a_tampered_move() {
    let text = include_str!("fixtures/rosin178.msr");
    let mut rec = decode(text).unwrap();
    // Move a point far away so its line's other points are absent.
    rec.moves[100].x += 50;
    assert!(validate(&rec).is_err());
}
