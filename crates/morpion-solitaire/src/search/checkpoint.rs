//! On-disk storage for search checkpoints (native only).
//!
//! Each search engine has its own checkpoint file, keyed by an `algo` tag
//! (`search-checkpoint-<algo>.msc`), so a systematic and an NRPA checkpoint can
//! coexist without clobbering each other. The payload ([`io::Checkpoint`]) also
//! carries the tag, letting resume verify/dispatch to the right engine. On the
//! web there is no filesystem, so this module is compiled out and checkpointing
//! is a no-op there.
#![cfg(not(target_arch = "wasm32"))]

use crate::game::io;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Application data directory (`$XDG_DATA_HOME/morpion-solitaire`, falling back
/// to `~/.local/share/...`). Shared by checkpoints and saved records so the GUI
/// and CLI agree on locations.
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("morpion-solitaire")
}

/// Explicit checkpoint directory, if set (CLI `--checkpoint-dir`). Lets a caller
/// place checkpoints at an isolated, known path — e.g. running many independent
/// searches side by side — without manipulating `HOME`/`XDG_DATA_HOME`.
static DIR_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

/// Override the directory checkpoints are read/written in. Call once, before any
/// checkpoint I/O; later calls are ignored.
pub fn set_dir(dir: PathBuf) {
    let _ = DIR_OVERRIDE.set(dir);
}

/// The directory checkpoints live in: the override if set, else the data dir.
fn checkpoint_dir() -> PathBuf {
    DIR_OVERRIDE.get().cloned().unwrap_or_else(data_dir)
}

/// Path of the checkpoint for `algo` (in the override dir if set, else the data dir).
pub fn path(algo: &str) -> PathBuf {
    checkpoint_dir().join(format!("search-checkpoint-{algo}.msc"))
}

/// Whether a saved checkpoint exists for `algo` (cheap — no parsing).
pub fn exists(algo: &str) -> bool {
    path(algo).exists()
}

/// Load and parse `algo`'s checkpoint, if one exists and is valid.
pub fn load(algo: &str) -> Option<io::Checkpoint> {
    let content = std::fs::read_to_string(path(algo)).ok()?;
    match io::import_checkpoint(&content) {
        Ok(cp) => Some(cp),
        Err(e) => {
            log::error!("checkpoint load failed: {e}");
            None
        }
    }
}

/// Atomically write `algo`'s serialised checkpoint: temp file + rename, so a
/// crash can't leave a truncated checkpoint behind.
pub fn write(algo: &str, serialized: &str) -> std::io::Result<()> {
    let path = path(algo);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serialized.as_bytes())?;
    std::fs::rename(&tmp, &path)
}
