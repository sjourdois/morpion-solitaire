//! Symmetry-invariant move coding (a `CodingModifier` of the NRPA family).

use crate::search::plugin::{CodingModifier, OptionKind, OptionSpec, Plugin, Registry, Scope};

/// Owns `--symmetry`/`--no-symmetry`. On (default): canonical D4 coding, all 8 Zobrist
/// hashes maintained. Off: identity frame only (one hash), ~+16% throughput at neutral
/// score — for cold record runs.
struct CoreCoding;
impl CodingModifier for CoreCoding {}
static CORE_CODING: CoreCoding = CoreCoding;

pub struct SymmetryPlugin;
impl Plugin for SymmetryPlugin {
    fn id(&self) -> &'static str {
        "symmetry"
    }
    fn register(&self, reg: &mut Registry) {
        reg.add_coding(&CORE_CODING);
        reg.add_option(OptionSpec {
            key: "symmetry",
            label_key: "opt-symmetry",
            help_key: "opt-symmetry-hint",
            help: "Drop symmetry-invariant move coding (identity frame only): ~+16% \
                   throughput at neutral score — recommended for cold record runs \
                   without warm-start. (The flag is `--no-symmetry`.)",
            kind: OptionKind::Toggle { default: true },
            scope: Scope::NrpaFamily,
        });
    }
}
pub static SYMMETRY_PLUGIN: SymmetryPlugin = SymmetryPlugin;
