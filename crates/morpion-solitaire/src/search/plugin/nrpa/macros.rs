//! Macro-actions: NRPA also samples multi-move motifs mined from records (an
//! action-space modifier of the NRPA method, 5T only). Native-only — the macro path
//! uses `nrpa::move_playable`. The plugin contributes options only; the engine reads
//! them once per search in `nrpa::island`.

use crate::search::plugin::{OptionKind, OptionSpec, Plugin, Registry, Scope};

pub struct MacrosPlugin;
impl Plugin for MacrosPlugin {
    fn id(&self) -> &'static str {
        "macros"
    }
    fn experimental(&self) -> bool {
        true
    }
    fn register(&self, reg: &mut Registry) {
        reg.add_option(OptionSpec {
            key: "macros",
            label_key: "opt-macros",
            help_key: "opt-macros-hint",
            help: "Macro-actions: NRPA also picks multi-move motifs mined from records \
                   (5T only), composing over a coarser horizon. Experimental.",
            kind: OptionKind::Toggle { default: false },
            scope: Scope::Methods(&["nrpa"]),
        });
        reg.add_option(OptionSpec {
            key: "macro-k",
            label_key: "opt-macro-k",
            help_key: "opt-macro-k-hint",
            help: "Macro motif length in moves (default 2). Read once at first use.",
            kind: OptionKind::Int {
                default: 2,
                min: 1,
                max: 6,
            },
            scope: Scope::Methods(&["nrpa"]),
        });
        reg.add_option(OptionSpec {
            key: "macro-topn",
            label_key: "opt-macro-topn",
            help_key: "opt-macro-topn-hint",
            help: "Macro library size: keep the top-N most frequent motifs (0 = all, \
                   default 32). Read once at first use.",
            kind: OptionKind::Int {
                default: 32,
                min: 0,
                max: 100_000,
            },
            scope: Scope::Methods(&["nrpa"]),
        });
    }
}
pub static MACROS_PLUGIN: MacrosPlugin = MacrosPlugin;
