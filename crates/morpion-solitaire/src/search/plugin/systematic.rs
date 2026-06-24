//! The systematic (exhaustive DFS) search method.

use std::sync::Arc;

use crate::search::{systematic, SearchState};

use super::{Method, Plugin, Registry, StartCtx};

struct Systematic;
impl Method for Systematic {
    fn id(&self) -> &'static str {
        "systematic"
    }
    fn label_key(&self) -> &'static str {
        "algo-systematic"
    }
    fn spawn(&self, ctx: StartCtx, search: Arc<SearchState>) {
        let initial = ctx.initial;
        std::thread::spawn(move || systematic::run(&initial, search));
    }
    fn method_desc(&self, _ctx: &StartCtx) -> String {
        "systematic".to_owned()
    }
    fn checkpoint_kind(&self) -> Option<&'static str> {
        Some("systematic")
    }
}
static SYSTEMATIC: Systematic = Systematic;

pub struct SystematicPlugin;
impl Plugin for SystematicPlugin {
    fn id(&self) -> &'static str {
        "systematic"
    }
    fn register(&self, reg: &mut Registry) {
        reg.add_method(&SYSTEMATIC);
    }
}
pub static SYSTEMATIC_PLUGIN: SystematicPlugin = SystematicPlugin;
