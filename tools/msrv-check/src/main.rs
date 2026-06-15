//! Touches the public API of both reusable libraries so the MSRV job actually
//! compiles them (and a representative slice of their API) on Rust 1.74.
fn main() {
    let (_name, _id, json) = morpion_solitaire_records::RECORDS[0];
    let record = msr::decode(json).expect("corpus record decodes");
    msr::validate(&record).expect("corpus record is legal");
    println!("MSR {} — {} records", msr::FORMAT_VERSION, morpion_solitaire_records::RECORDS.len());
}
