//! Generate every distributable format for each corpus record from its JSON
//! source (the single committed source of truth). Run by CI to publish the
//! artifacts under `/records/` on the site; run it locally to preview them.
//!
//! ```sh
//! cargo run -p morpion-solitaire --example gen_record_artifacts -- out_dir
//! ```
//!
//! For each record `<id>` it writes, into `out_dir`:
//!   - `<id>.json`  the source record (copied verbatim),
//!   - `<id>.msr`   the compact `MS1:` form,
//!   - `<id>.png`   a rendered board with the record embedded (PNG `tEXt`),
//!   - `<id>.svg`   the same as a vector image (SVG `<metadata>`),
//!   - `<id>.psol`  the legacy Pentasol form (5T/5D only).
use morpion_solitaire::game::io;
use morpion_solitaire::render::{embed_msr_png, embed_msr_svg, to_png, to_svg, RenderOpts};
use morpion_solitaire_records::RECORDS;
use std::{fs, path::PathBuf};

fn main() {
    let out: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "artifacts".into())
        .into();
    fs::create_dir_all(&out).unwrap();
    let opts = RenderOpts { numbers: true };

    for (name, id, json) in RECORDS {
        let record = msr::decode(json).unwrap_or_else(|e| panic!("{name}: decode: {e}"));
        let compact = msr::encode(&record).unwrap();
        let state = io::import_save(json).unwrap_or_else(|e| panic!("{name}: import: {e}"));

        fs::write(out.join(format!("{id}.json")), json).unwrap();
        fs::write(out.join(format!("{id}.msr")), &compact).unwrap();
        fs::write(
            out.join(format!("{id}.svg")),
            embed_msr_svg(&to_svg(&state, &opts), &compact),
        )
        .unwrap();
        let png = to_png(&state, &opts).unwrap_or_else(|e| panic!("{name}: png: {e}"));
        fs::write(out.join(format!("{id}.png")), embed_msr_png(&png, &compact)).unwrap();
        if state.variant.len() == 5 {
            fs::write(out.join(format!("{id}.psol")), io::export_pentasol(&state)).unwrap();
        }
        println!("{id}\t{}\t{} moves", state.variant.name(), state.score());
    }
    println!("wrote {} records to {}", RECORDS.len(), out.display());
}
