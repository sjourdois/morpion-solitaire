//! The beam-search method + its `width` option.

use std::sync::Arc;

use crate::search::{beam, SearchState};

use super::{Method, OptionKind, OptionSpec, Plugin, Registry, Scope, StartCtx};

struct BeamMethod;
impl Method for BeamMethod {
    fn id(&self) -> &'static str {
        "beam"
    }
    fn label_key(&self) -> &'static str {
        "algo-beam"
    }
    fn spawn(&self, ctx: StartCtx, search: Arc<SearchState>) {
        let StartCtx { initial, width, .. } = ctx;
        std::thread::spawn(move || beam::run(&initial, search, width));
    }
    fn method_desc(&self, ctx: &StartCtx) -> String {
        format!("beam w={}", ctx.width)
    }
    fn checkpoint_kind(&self) -> Option<&'static str> {
        None
    }
}
static BEAM: BeamMethod = BeamMethod;

pub struct BeamPlugin;
impl Plugin for BeamPlugin {
    fn id(&self) -> &'static str {
        "beam"
    }
    fn register(&self, reg: &mut Registry) {
        reg.add_method(&BEAM);
        reg.add_option(OptionSpec {
            key: "width",
            label_key: "opt-width",
            help_key: "opt-width-hint",
            help: "Beam width (kept candidates per depth).",
            kind: OptionKind::Int {
                default: 64,
                min: 1,
                max: 100_000,
            },
            scope: Scope::Methods(&["beam"]),
        });
    }
}
pub static BEAM_PLUGIN: BeamPlugin = BeamPlugin;
