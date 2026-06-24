pub mod beam;
pub mod bounds;
#[cfg(not(target_arch = "wasm32"))]
pub mod checkpoint;
pub mod nrpa;
pub mod symmetry;
pub mod systematic;

use crate::game::moves::Move;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering},
    Arc, RwLock,
};
use std::time::Duration;
use web_time::Instant;

#[derive(Debug)]
pub struct SearchState {
    pub best_score: AtomicU32,
    pub best_sequence: RwLock<Vec<Move>>,
    pub nodes_explored: AtomicU64,
    pub running: AtomicBool,
    /// Cooperative pause: while set, worker loops idle at their next boundary
    /// instead of stopping, so the search can be resumed in place. Independent of
    /// `running` (a stop always wins, so pause can never deadlock a stop).
    pub paused: AtomicBool,
    /// Set to ask the systematic search to checkpoint at the next safe point
    /// (workers push back their frontier; the engine serialises and resumes).
    pub checkpoint_requested: AtomicBool,
    /// Set by the systematic search when it drains the whole tree on its own (the
    /// frontier empties while still running, as opposed to a user stop). When
    /// true, `best_score` is provably optimal for the variant. Never set by the
    /// heuristic searches, which don't terminate.
    pub exhausted: AtomicBool,
    /// Successive record improvements: (score, elapsed since search start).
    pub records: RwLock<Vec<(u32, Duration)>>,
    start_time: RwLock<Option<Instant>>,
    /// Soft cap on an NRPA island's policy size (number of entries); `0` = no cap.
    /// When an island's policy grows past this, it restarts from a fresh policy at
    /// the next recursion boundary — bounding memory in-process (the unbounded
    /// policy is otherwise what makes a long, deep run exhaust RAM). Config, not
    /// search state, so it survives [`reset`](Self::reset).
    pub max_policy_entries: AtomicUsize,
}

impl SearchState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            best_score: AtomicU32::new(0),
            best_sequence: RwLock::new(Vec::new()),
            nodes_explored: AtomicU64::new(0),
            running: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            checkpoint_requested: AtomicBool::new(false),
            exhausted: AtomicBool::new(false),
            records: RwLock::new(Vec::new()),
            start_time: RwLock::new(None),
            max_policy_entries: AtomicUsize::new(0),
        })
    }

    pub fn reset(&self) {
        self.best_score.store(0, Ordering::Relaxed);
        self.nodes_explored.store(0, Ordering::Relaxed);
        self.exhausted.store(false, Ordering::Relaxed);
        *self.best_sequence.write().unwrap() = Vec::new();
        *self.records.write().unwrap() = Vec::new();
        *self.start_time.write().unwrap() = Some(Instant::now());
    }

    /// Block while paused, checked cooperatively at a worker-loop boundary. A
    /// stop (`running` cleared) always breaks out, so pausing can never wedge a
    /// stop or a checkpoint. On native this idles; on wasm there is no blocking
    /// sleep, so it spins (pause is a brief, user-driven state).
    pub fn wait_if_paused(&self) {
        while self.paused.load(Ordering::Relaxed) && self.running.load(Ordering::Relaxed) {
            #[cfg(not(target_arch = "wasm32"))]
            std::thread::sleep(Duration::from_millis(50));
            #[cfg(target_arch = "wasm32")]
            std::hint::spin_loop();
        }
    }

    /// Update best score; if improved, record the sequence and elapsed time.
    pub fn record_best(&self, score: u32, history: Vec<Move>) {
        let prev = self.best_score.fetch_max(score, Ordering::Relaxed);
        if score > prev {
            *self.best_sequence.write().unwrap() = history;
            let elapsed = self
                .start_time
                .read()
                .unwrap()
                .map(|t| t.elapsed())
                .unwrap_or_default();
            let mut recs = self.records.write().unwrap();
            // Guard against concurrent threads both seeing score > prev
            if recs.last().map(|(s, _)| *s != score).unwrap_or(true) {
                recs.push((score, elapsed));
            }
        }
    }
}
