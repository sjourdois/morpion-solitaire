//! PUCT — policy+value tree search, a method plugin (`neural` feature). The tree, the
//! rollouts and the value-net leaf live in `search::neural::puct`; this file is the
//! method wrapper + the `c-puct` option.

use std::sync::Arc;

use crate::search::neural;
use crate::search::plugin::{
    registry, Method, OptionKind, OptionSpec, Plugin, Registry, Scope, StartCtx,
};
use crate::search::SearchState;

struct PuctMethod;
impl Method for PuctMethod {
    fn id(&self) -> &'static str {
        "puct"
    }
    fn label_key(&self) -> &'static str {
        "algo-puct"
    }
    fn spawn(&self, ctx: StartCtx, search: Arc<SearchState>) {
        let StartCtx { variant, .. } = ctx;
        std::thread::spawn(move || neural::run_puct_armed(search, variant));
    }
    fn method_desc(&self, _ctx: &StartCtx) -> String {
        let c = registry().value_f64("c-puct", 1.5);
        let policy = if neural::is_armed() { "neural" } else { "uniform" };
        let leaf = if neural::armed_value().is_some() {
            "value"
        } else {
            "rollout"
        };
        format!("puct c={c:.2} policy={policy} leaf={leaf}")
    }
    fn checkpoint_kind(&self) -> Option<&'static str> {
        None
    }
}
static PUCT_METHOD: PuctMethod = PuctMethod;

pub struct PuctPlugin;
impl Plugin for PuctPlugin {
    fn id(&self) -> &'static str {
        "puct"
    }
    fn experimental(&self) -> bool {
        true
    }
    fn register(&self, reg: &mut Registry) {
        reg.add_method(&PUCT_METHOD);
        reg.add_option(OptionSpec {
            key: "c-puct",
            label_key: "opt-c-puct",
            help_key: "opt-c-puct-hint",
            help: "PUCT exploration constant (higher = more exploration). Default 1.5.",
            kind: OptionKind::Float {
                default: 1.5,
                min: 0.1,
                max: 5.0,
                step: 0.1,
            },
            scope: Scope::Methods(&["puct"]),
        });
    }
}
pub static PUCT_PLUGIN: PuctPlugin = PuctPlugin;
