//! Headless command-line interface (native only).
//!
//! The GUI stays the default: `morpion-solitaire` with no subcommand (or `gui`)
//! launches it. Every other capability — search, replay, convert, records,
//! bench — is a subcommand here, a second façade over the same engines and `io`
//! formats the GUI uses. Parsed with `clap`.
//!
//! All CLI output is English — the GUI is the project's localized surface. A
//! stable English CLI matches the convention for developer tools (and clap's own
//! `--help`/error scaffolding, which isn't translatable), and stays scriptable.
#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::game::{
    io::{self, SaveInfo, SaveMeta},
    moves::legal_moves,
    rules::Variant,
    state::GameState,
};
use crate::search::{beam, checkpoint, nrpa, systematic, SearchState};

/// Top-level CLI. With no subcommand, falls through to the GUI.
#[derive(Parser)]
#[command(
    name = "morpion-solitaire",
    version,
    about = "Morpion Solitaire — a GUI and a command-line solver",
    subcommand_negates_reqs = true
)]
pub struct Cli {
    /// Game variant, used where it isn't inferred from a file.
    #[arg(long, global = true, default_value = "5T", value_name = "5T|5D|4T|4D")]
    variant: String,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)] // SearchArgs is the big one; the CLI is not hot
enum Command {
    /// Launch the graphical interface (the default with no subcommand).
    Gui,
    /// Run a headless search and write the best game found.
    Search(SearchArgs),
    /// Replay a saved game: re-derive it (checking every move is legal), then
    /// print its metadata, board and a verdict. `-q` prints only the verdict.
    Replay(ReplayArgs),
    /// Convert/render a game to any format: ascii, msr, json, pentasol, svg, png.
    Convert(ConvertArgs),
    /// List saved records and their scores.
    Records(RecordsArgs),
    /// Micro-benchmark an engine's throughput (nodes/second).
    Bench(BenchArgs),
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum AlgoArg {
    Nrpa,
    Systematic,
    Perturbation,
    Beam,
}

impl AlgoArg {
    /// The registry method id this `--algo` value maps to.
    fn id(self) -> &'static str {
        match self {
            AlgoArg::Nrpa => "nrpa",
            AlgoArg::Systematic => "systematic",
            AlgoArg::Perturbation => "perturbation",
            AlgoArg::Beam => "beam",
        }
    }
}

#[derive(Copy, Clone, ValueEnum)]
enum Format {
    /// ASCII board for the terminal (the default).
    Ascii,
    /// Compact MSR (the `MS1:` envelope) — the usual `.msr` form.
    Msr,
    /// Readable MSR as pretty JSON (lossless with `msr`).
    Json,
    /// Legacy Pentasol text — 5T/5D only; drops the variant and all metadata.
    Pentasol,
    /// SVG vector image, with the record embedded (stdout or `-o`).
    Svg,
    /// PNG raster image, with the record embedded (requires `-o`).
    Png,
}

#[derive(Args)]
struct SearchArgs {
    /// Search engine.
    #[arg(long, value_enum, default_value_t = AlgoArg::Nrpa)]
    algo: AlgoArg,
    /// NRPA nesting level (recursion depth).
    #[arg(long, default_value_t = 3)]
    level: usize,
    /// Beam width.
    #[arg(long, default_value_t = 64)]
    width: usize,
    /// Stabilized-NRPA logit clamp C (default 3; `0` disables clamping). The tight
    /// sweet spot for record hunting; only re-tune for experiments.
    #[arg(long, value_name = "C")]
    clamp: Option<f64>,
    /// Policy adaptation step size α (default 1.0).
    #[arg(long, value_name = "A")]
    alpha: Option<f64>,
    /// Drop symmetry-invariant move coding (identity frame only): ~+16 % throughput
    /// at neutral score — recommended for cold record runs without warm-start.
    #[arg(long)]
    no_symmetry: bool,
    /// Perturbation genetic-crossover rate (0 = off). Only used by
    /// `--algo perturbation`.
    #[arg(long, default_value_t = 0.0)]
    crossover: f64,
    /// Perturbation destroy-size lower bound K_min (default 8). `--algo perturbation`.
    #[arg(long, value_name = "K")]
    kmin: Option<usize>,
    /// Perturbation destroy-size upper bound K_max (default 70). `--algo perturbation`.
    #[arg(long, value_name = "K")]
    kmax: Option<usize>,
    /// Perturbation tabu/preservation window (default 10). `--algo perturbation`.
    #[arg(long, value_name = "N")]
    window: Option<usize>,
    /// Warm-start NRPA from a game file (policy seed).
    #[arg(long, value_name = "FILE")]
    warm: Option<PathBuf>,
    /// Start from a loaded position instead of the empty cross.
    #[arg(long, value_name = "FILE")]
    from: Option<PathBuf>,
    /// Worker threads (default: all logical cores). Sizes the rayon pool the
    /// islands draw from; best-effort (no-op if the pool is already built).
    #[arg(long)]
    threads: Option<usize>,
    /// RNG seed (reproducibility; recorded in the output).
    #[arg(long)]
    seed: Option<u64>,
    /// Stop criterion: duration (`30s`, `5m`, `1h`, or a number of seconds).
    #[arg(long, value_name = "DURATION", value_parser = parse_duration)]
    time: Option<Duration>,
    /// Stop criterion: stop as soon as this score is reached.
    #[arg(long, value_name = "N")]
    target_score: Option<u32>,
    /// Stop criterion: stop after this many nodes.
    #[arg(long, value_name = "N")]
    max_nodes: Option<u64>,
    /// Auto-checkpoint interval (same files as the GUI).
    #[arg(long, value_name = "DURATION", value_parser = parse_duration)]
    checkpoint_interval: Option<Duration>,
    /// Directory for the auto-checkpoint (default: the GUI/XDG data dir). Set an
    /// explicit dir to run independent searches without their checkpoints colliding.
    #[arg(long, value_name = "DIR")]
    checkpoint_dir: Option<PathBuf>,
    /// Resume from a checkpoint file (the engine is read from the file).
    #[arg(long, value_name = "FILE")]
    resume: Option<PathBuf>,
    /// Write the best game here (otherwise stdout). Always the msr format. Updated
    /// as the run improves (not only at exit), so it stays fresh for a long run.
    #[arg(long, short = 'o', value_name = "FILE")]
    out: Option<PathBuf>,
    /// Put all run outputs (best.msr, checkpoint, progress.log) under this one dir.
    /// Convenience: fills in --out/--checkpoint-dir/--progress-log when they're unset.
    #[arg(long, value_name = "DIR")]
    run_dir: Option<PathBuf>,
    /// Append a timestamped progress line (ISO time, score, nodes) here each tick.
    #[arg(long, value_name = "FILE")]
    progress_log: Option<PathBuf>,
    /// Soft RAM budget (e.g. `12G`, `500M`). An NRPA island restarts from a fresh
    /// policy once its policy would exceed its share, bounding memory in-process.
    #[arg(long, value_name = "SIZE", value_parser = parse_size)]
    max_memory: Option<u64>,
    /// Run at this process niceness (e.g. `10`, `19`) so the search yields CPU.
    #[arg(long, value_name = "N", allow_hyphen_values = true)]
    nice: Option<i32>,
    /// Keep searching past a grid overflow instead of stopping. A game that hits
    /// the fixed grid's edge is truncated — not a valid record; use only to probe.
    #[arg(long)]
    ignore_overflow: bool,
    /// Free-text description stored in the output.
    #[arg(long)]
    description: Option<String>,
    /// Author stored in the output.
    #[arg(long)]
    author: Option<String>,
    /// Tag(s); repeatable or comma-separated.
    #[arg(long = "tag", value_delimiter = ',')]
    tags: Vec<String>,
    /// Don't print the periodic stats line.
    #[arg(long, short = 'q')]
    quiet: bool,
}

#[derive(Args)]
struct ReplayArgs {
    /// Game file (.msr, JSON, or Pentasol — detected by content).
    file: PathBuf,
    /// Number the moves in the printed board.
    #[arg(long)]
    numbers: bool,
    /// Only print the one-line legality verdict (no metadata or board) — the
    /// scriptable form; the exit status is non-zero for an illegal game.
    #[arg(long, short = 'q')]
    quiet: bool,
}

#[derive(Args)]
struct ConvertArgs {
    /// Input file (`.msr`, JSON, or Pentasol — detected by content).
    file: PathBuf,
    /// Target format. MSR ↔ JSON is lossless; Pentasol is 5T/5D only and keeps
    /// no metadata; SVG/PNG embed the record. PNG requires `-o`.
    #[arg(long, value_enum, default_value_t = Format::Ascii)]
    to: Format,
    /// Number the moves (ASCII only; SVG/PNG always number them).
    #[arg(long)]
    numbers: bool,
    /// Output file (otherwise stdout, where the format is text).
    #[arg(long, short = 'o', value_name = "FILE")]
    out: Option<PathBuf>,
}

#[derive(Args)]
struct RecordsArgs {
    /// Restrict to one category (nrpa, systematic, overflow, …).
    #[arg(long)]
    category: Option<String>,
}

#[derive(Args)]
struct BenchArgs {
    /// Engine to measure.
    #[arg(long, value_enum, default_value_t = AlgoArg::Nrpa)]
    algo: AlgoArg,
    /// NRPA level.
    #[arg(long, default_value_t = 3)]
    level: usize,
    /// Measurement duration.
    #[arg(long, value_name = "DURATION", value_parser = parse_duration, default_value = "10s")]
    time: Duration,
}

/// Parse the CLI. Returns `None` when the GUI should run (no subcommand or
/// `gui`); otherwise runs the chosen subcommand and exits with its status.
pub fn dispatch() -> Option<()> {
    let cli = Cli::parse();
    let variant = parse_variant_or_exit(&cli.variant);
    match cli.command {
        None | Some(Command::Gui) => None, // hand back to the GUI
        Some(cmd) => {
            let code = match run(cmd, variant) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("error: {e}");
                    1
                }
            };
            std::process::exit(code);
        }
    }
}

fn run(cmd: Command, variant: Variant) -> Result<(), String> {
    match cmd {
        Command::Gui => unreachable!(),
        Command::Search(a) => cmd_search(a, variant),
        Command::Replay(a) => cmd_replay(a, variant),
        Command::Convert(a) => cmd_convert(a, variant),
        Command::Records(a) => cmd_records(a),
        Command::Bench(a) => cmd_bench(a, variant),
    }
}

// ── search ──────────────────────────────────────────────────────────────────

fn cmd_search(mut a: SearchArgs, cli_variant: Variant) -> Result<(), String> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    // --run-dir: gather all run outputs under one dir (only fills unset paths).
    if let Some(dir) = a.run_dir.clone() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("run-dir {}: {e}", dir.display()))?;
        a.out.get_or_insert_with(|| dir.join("best.msr"));
        a.checkpoint_dir.get_or_insert_with(|| dir.clone());
        a.progress_log.get_or_insert_with(|| dir.join("progress.log"));
    }

    // --nice: lower scheduling priority so the search yields CPU to other work.
    if let Some(n) = a.nice {
        match rustix::process::nice(n) {
            Ok(got) => log::info!("niceness set to {got}"),
            Err(e) => eprintln!("warning: could not set niceness: {e}"),
        }
    }

    if let Some(n) = a.threads {
        // Size the global rayon pool the islands/workers draw from. Best-effort:
        // if a pool already exists this is a no-op.
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(n.max(1))
            .build_global();
    }

    // Place auto-checkpoints in an explicit dir if asked, before any checkpoint I/O.
    if let Some(dir) = &a.checkpoint_dir {
        crate::search::checkpoint::set_dir(dir.clone());
    }

    let search = SearchState::new();

    // --max-memory: cap each NRPA island's policy at its share of the budget, so a
    // long deep run restarts islands instead of exhausting RAM.
    if let Some(bytes) = a.max_memory {
        // ~bytes per FxHashMap<u64,f64> entry including hashbrown control/overhead.
        const PER_ENTRY: u64 = 64;
        let islands = a.threads.unwrap_or_else(num_cpus::get).max(1) as u64;
        let cap = (bytes / (islands * PER_ENTRY)).max(1) as usize;
        search.max_policy_entries.store(cap, Ordering::Relaxed);
        log::info!("policy cap: {cap} entries/island ({islands} islands)");
    }

    let t0 = Instant::now();

    // Graceful Ctrl-C: ask the search to stop; the monitor loop then saves.
    let interrupted = Arc::new(AtomicBool::new(false));
    {
        let flag = interrupted.clone();
        let s = search.clone();
        let _ = ctrlc::set_handler(move || {
            flag.store(true, Ordering::Relaxed);
            s.running.store(false, Ordering::Relaxed);
        });
    }

    // Mark running before spawning so the monitor loop can't see the brief
    // window before the engine thread sets it (which would stop immediately).
    search.running.store(true, Ordering::Relaxed);

    // Effective variant + provenance string for the output metadata.
    let (variant, method) = spawn_search(&a, cli_variant, &search)?;

    if !a.quiet {
        eprintln!(
            "search {} — {}; stop: {}",
            method,
            variant.name(),
            stop_criteria_desc(&a)
        );
    }

    // Monitor loop: print stats, enforce stop criteria, drive checkpoints.
    let mut last_ckpt = Instant::now();
    let mut last_emitted = 0u32; // best score already written to --out
    let mut last_progress = Instant::now();
    loop {
        let best = search.best_score.load(Ordering::Relaxed);
        let nodes = search.nodes_explored.load(Ordering::Relaxed);

        if crate::game::board::GRID_OVERFLOW.swap(false, Ordering::Relaxed) {
            handle_overflow(&a, variant, best);
            if !a.ignore_overflow {
                search.running.store(false, Ordering::Relaxed);
            }
        }

        let stop = !search.running.load(Ordering::Relaxed)
            || interrupted.load(Ordering::Relaxed)
            || a.target_score.is_some_and(|t| best >= t)
            || a.max_nodes.is_some_and(|m| nodes >= m)
            || a.time.is_some_and(|d| t0.elapsed() >= d);
        if stop {
            break;
        }

        if let Some(iv) = a.checkpoint_interval {
            if last_ckpt.elapsed() >= iv {
                drive_checkpoint(a.algo, variant, &search);
                last_ckpt = Instant::now();
            }
        }

        // Keep --out fresh as the best improves (a long run shouldn't only emit at
        // exit), and append a timestamped progress line.
        if a.out.is_some()
            && best > last_emitted
            && emit_best(&a, variant, &method, &search, t0).is_ok()
        {
            last_emitted = best;
        }
        if let Some(plog) = &a.progress_log {
            if last_progress.elapsed() >= Duration::from_secs(10) {
                last_progress = Instant::now();
                append_progress(plog, best, nodes);
            }
        }

        if !a.quiet {
            let secs = t0.elapsed().as_secs_f64().max(1e-9);
            let line = format!(
                "score={best:>3}  nodes={nodes:>12}  {:>8.0} n/s  {:>5.0}s",
                nodes as f64 / secs,
                t0.elapsed().as_secs_f64()
            );
            eprint!("\r  {line}   ");
            use std::io::Write as _;
            let _ = std::io::stderr().flush();
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    search.running.store(false, Ordering::Relaxed);
    if !a.quiet {
        eprintln!();
    }

    // Final emit (same path as the periodic refresh).
    let score = emit_best(&a, variant, &method, &search, t0)?;
    if interrupted.load(Ordering::Relaxed) {
        eprintln!("best: {score} moves (interrupted)");
    } else {
        eprintln!("best: {score} moves");
    }
    Ok(())
}

/// Reconstruct the best game with full provenance and write it to `--out` (or
/// stdout when unset). Returns the score. Shared by the periodic refresh and the
/// final emit so a long run keeps `--out` current instead of only writing at exit.
fn emit_best(
    a: &SearchArgs,
    variant: Variant,
    method: &str,
    search: &Arc<SearchState>,
    t0: Instant,
) -> Result<usize, String> {
    let best_seq = search.best_sequence.read().unwrap().clone();
    if best_seq.is_empty() {
        return Err("no game found".to_owned());
    }
    let mut state = GameState::new(variant);
    for mv in &best_seq {
        state.apply(*mv);
    }
    let meta = SaveMeta {
        description: a.description.clone(),
        author: a.author.clone(),
        source: None,
        transcribed_by: None,
        tool: Some(env!("CARGO_PKG_NAME").to_owned()),
        method: Some(method.to_owned()),
        seed: a.seed,
        nodes_explored: Some(search.nodes_explored.load(Ordering::Relaxed)),
        elapsed_secs: Some(t0.elapsed().as_secs_f64()),
        tags: a.tags.clone(),
    };
    let blob =
        io::export_save_with_meta(&state, io::unix_now(), &meta).map_err(|e| e.to_string())?;
    emit(a.out.as_deref(), &blob)?;
    Ok(state.score())
}

/// Append a timestamped progress line (`<unix_secs>\tscore=N\tnodes=M`) to `path`.
fn append_progress(path: &std::path::Path, score: u32, nodes: u64) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let line = format!("{now}\tscore={score}\tnodes={nodes}\n");
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
}

/// Spawn the chosen engine on a background thread. Returns the effective variant
/// and a provenance string for the output metadata.
fn spawn_search(
    a: &SearchArgs,
    cli_variant: Variant,
    search: &Arc<SearchState>,
) -> Result<(Variant, String), String> {
    let level = a.level;
    let width = a.width;

    // Resume takes precedence and carries its own variant/engine.
    if let Some(path) = &a.resume {
        let text = read_to_string(path)?;
        let cp = io::import_checkpoint(&text)?;
        let variant = cp.variant;
        let algo_name = cp.algo.clone();
        let display = format!("resume:{algo_name}");
        let s = search.clone();
        std::thread::spawn(move || match algo_name.as_str() {
            "systematic" => systematic::resume(s, cp),
            "perturbation" => nrpa::resume_perturbation(s, level, cp.variant, cp.frontier),
            _ => nrpa::resume(s, cp, level),
        });
        return Ok((variant, display));
    }

    let warm_seq = match &a.warm {
        Some(p) => Some(load_game(p, cli_variant)?.0.history),
        None => None,
    };
    let from_state = match &a.from {
        Some(p) => Some(load_game(p, cli_variant)?.0),
        None => None,
    };
    let variant = from_state
        .as_ref()
        .map(|s| s.variant)
        .unwrap_or(cli_variant);

    // Core tuning levers (proper options, not env vars). Process-global overrides
    // read by every island thread; set them before spawning. Unset ⇒ engine default.
    if let Some(c) = a.clamp {
        crate::search::plugin::set_clamp(c);
    }
    if let Some(al) = a.alpha {
        crate::search::plugin::set_alpha(al);
    }
    if a.no_symmetry {
        crate::search::plugin::set_symmetry(false);
    }
    if let Some(k) = a.kmin {
        nrpa::set_perturb_k_min_override(k);
    }
    if let Some(k) = a.kmax {
        nrpa::set_perturb_k_max_override(k);
    }
    if let Some(w) = a.window {
        nrpa::set_perturb_window_override(w);
    }
    if a.crossover > 0.0 {
        crate::search::plugin::set_crossover(a.crossover);
    }

    let s = search.clone();

    // Dispatch through the plugin registry (docs/plugin-framework.md): build the
    // launch context, then let the method spawn its own search thread.
    let m = crate::search::plugin::registry()
        .method(a.algo.id())
        .expect("core method is registered");
    let initial = from_state
        .clone()
        .unwrap_or_else(|| GameState::new(variant));
    let seed_len = from_state.as_ref().map(|st| st.history.len()).unwrap_or(0);
    let seed_history = from_state.map(|st| st.history).unwrap_or_default();
    let ctx = crate::search::plugin::StartCtx {
        initial,
        variant,
        level,
        width,
        warm_seq,
        seed_history,
        seed_len,
    };
    let method = m.method_desc(&ctx);
    m.spawn(ctx, s);
    Ok((variant, method))
}

fn cmd_replay(a: ReplayArgs, variant: Variant) -> Result<(), String> {
    let (state, info) = load_game(&a.file, variant)?;
    // Re-derive legality move-by-move from scratch: replaying *is* verifying.
    let mut check = GameState::new(state.variant);
    for (i, mv) in state.history.iter().enumerate() {
        let legal = legal_moves(&check);
        if !legal.iter().any(|m| m.pos == mv.pos && m.line == mv.line) {
            return Err(format!(
                "illegal move #{} at ({},{}) — the game is not valid",
                i + 1,
                mv.pos.0,
                mv.pos.1
            ));
        }
        check.apply(*mv);
    }
    // Human view (skipped in quiet mode, which prints only the verdict).
    if !a.quiet {
        print_info(&state, &info);
        print!("{}", ascii_board(&state, a.numbers));
    }
    // One-line legality verdict.
    let avail = legal_moves(&check).len();
    let status = if avail == 0 {
        "terminal".to_owned()
    } else {
        format!("non-terminal, {avail} moves available")
    };
    println!(
        "OK — {} legal moves, {} ({status})",
        check.score(),
        state.variant.name()
    );
    Ok(())
}

fn cmd_convert(a: ConvertArgs, variant: Variant) -> Result<(), String> {
    use crate::render::{embed_msr_png, embed_msr_svg, to_png, to_svg, RenderOpts};
    let (state, info) = load_game(&a.file, variant)?;
    let meta = SaveMeta {
        description: info.description,
        author: info.author,
        source: info.source,
        transcribed_by: info.transcribed_by,
        tool: info.tool,
        method: info.method,
        seed: info.seed,
        nodes_explored: info.nodes_explored,
        elapsed_secs: info.elapsed_secs,
        tags: info.tags,
    };
    // The compact record (with provenance) is also what the SVG/PNG embed, so the
    // image is itself a save.
    let record =
        io::export_save_with_meta(&state, io::unix_now(), &meta).map_err(|e| e.to_string());
    let opts = RenderOpts { numbers: true };

    let text = match a.to {
        Format::Ascii => ascii_board(&state, a.numbers),
        Format::Msr => record?,
        Format::Json => {
            io::export_json_with_meta(&state, io::unix_now(), &meta).map_err(|e| e.to_string())?
        }
        Format::Pentasol => {
            if state.variant.len() != 5 {
                return Err("the Pentasol format only covers 5T and 5D".to_owned());
            }
            io::export_pentasol(&state)
        }
        Format::Svg => embed_msr_svg(&to_svg(&state, &opts), &record?),
        Format::Png => {
            // PNG is binary, so it never goes to stdout.
            let path = a.out.as_deref().ok_or("PNG output requires -o <FILE>")?;
            let png = embed_msr_png(&to_png(&state, &opts)?, &record?);
            std::fs::write(path, png).map_err(|e| format!("writing {}: {e}", path.display()))?;
            eprintln!("wrote: {}", path.display());
            return Ok(());
        }
    };
    emit(a.out.as_deref(), &text)
}

fn cmd_records(a: RecordsArgs) -> Result<(), String> {
    let root = checkpoint::data_dir().join("records");
    if !root.exists() {
        println!("(no records saved under {})", root.display());
        return Ok(());
    }
    let mut cats: Vec<PathBuf> = match &a.category {
        Some(c) => vec![root.join(c)],
        None => std::fs::read_dir(&root)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect(),
    };
    cats.sort();
    for cat in cats {
        let name = cat.file_name().and_then(|s| s.to_str()).unwrap_or("?");
        let mut files: Vec<(u32, PathBuf)> = std::fs::read_dir(&cat)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "msr"))
            .filter_map(|p| {
                let txt = std::fs::read_to_string(&p).ok()?;
                let (st, _) = io::import_save_with_info(&txt).ok()?;
                Some((st.score() as u32, p))
            })
            .collect();
        files.sort_by_key(|f| std::cmp::Reverse(f.0));
        if files.is_empty() {
            continue;
        }
        println!("{name} ({} files) — best: {}", files.len(), files[0].0);
        for (score, p) in files.iter().take(5) {
            println!("  {score:>3}  {}", p.file_name().unwrap().to_string_lossy());
        }
    }
    Ok(())
}

fn cmd_bench(a: BenchArgs, variant: Variant) -> Result<(), String> {
    let search = SearchState::new();
    let s = search.clone();
    let level = a.level;
    let initial = GameState::new(variant);
    match a.algo {
        AlgoArg::Systematic => std::thread::spawn(move || systematic::run(&initial, s)),
        AlgoArg::Beam => std::thread::spawn(move || beam::run(&initial, s, 64)),
        _ => std::thread::spawn(move || nrpa::run(&initial, s, level)),
    };
    let t0 = Instant::now();
    std::thread::sleep(a.time);
    search.running.store(false, Ordering::Relaxed);
    let nodes = search.nodes_explored.load(Ordering::Relaxed);
    let best = search.best_score.load(Ordering::Relaxed);
    let secs = t0.elapsed().as_secs_f64();
    let algo = match a.algo {
        AlgoArg::Systematic => "systematic",
        AlgoArg::Beam => "beam",
        _ => "nrpa",
    };
    println!(
        "{} {algo}: {nodes} nodes in {secs:.1}s = {:.0} n/s; best {best}",
        variant.name(),
        nodes as f64 / secs
    );
    Ok(())
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn handle_overflow(a: &SearchArgs, variant: Variant, best: u32) {
    let grid = crate::game::board::GRID;
    eprintln!("\n⚠ GRID OVERFLOW {grid}×{grid} (at {best} moves) — widen `Row` in board.rs.");
    let _ = (a, variant);
}

fn drive_checkpoint(algo: AlgoArg, variant: Variant, search: &Arc<SearchState>) {
    match algo {
        AlgoArg::Systematic | AlgoArg::Perturbation => {
            search.checkpoint_requested.store(true, Ordering::Relaxed)
        }
        AlgoArg::Nrpa => nrpa::save_checkpoint(variant, search),
        AlgoArg::Beam => {}
    }
}

fn stop_criteria_desc(a: &SearchArgs) -> String {
    let mut parts = Vec::new();
    if let Some(d) = a.time {
        parts.push(format!("{}s", d.as_secs()));
    }
    if let Some(t) = a.target_score {
        parts.push(format!("score≥{t}"));
    }
    if let Some(m) = a.max_nodes {
        parts.push(format!("{m} nodes"));
    }
    if parts.is_empty() {
        "Ctrl-C".to_owned()
    } else {
        parts.join(" or ")
    }
}

/// Load a game file, auto-detecting `.msr`/JSON vs Pentasol text.
fn load_game(path: &Path, variant: Variant) -> Result<(GameState, SaveInfo), String> {
    let text = read_to_string(path)?;
    let t = text.trim_start();
    if t.starts_with("MS1:") || t.starts_with('{') {
        io::import_save_with_info(&text)
    } else {
        io::import_pentasol(&text, variant).map(|s| (s, SaveInfo::default()))
    }
}

fn print_info(state: &GameState, info: &SaveInfo) {
    let avail = legal_moves(state).len();
    let status = if avail == 0 {
        "terminal".to_owned()
    } else {
        format!("{avail} moves available")
    };
    println!("variant: {}", state.variant.name());
    println!("score: {} ({status})", state.score());
    if let Some(p) = &info.producer {
        println!("producer: {p}");
    }
    if let Some(d) = &info.saved_at {
        println!("date: {d}");
    }
    if let Some(d) = &info.description {
        println!("description: {d}");
    }
    if let Some(d) = &info.author {
        println!("author: {d}");
    }
    if let Some(d) = &info.method {
        println!("method: {d}");
    }
    if let Some(d) = &info.source {
        println!("source: {d}");
    }
    if let Some(d) = info.seed {
        println!("seed: {d}");
    }
    if let Some(d) = info.nodes_explored {
        println!("nodes: {d}");
    }
    if let Some(d) = info.elapsed_secs {
        println!("elapsed (s): {d:.1}");
    }
    if !info.tags.is_empty() {
        println!("tags: {}", info.tags.join(", "));
    }
}

/// Plain dots-and-numbers ASCII rendering of the board.
fn ascii_board(state: &GameState, numbers: bool) -> String {
    let Some((min_x, min_y, max_x, max_y)) = state.bounding_box() else {
        return "(empty board)\n".to_owned();
    };
    let order: std::collections::HashMap<_, usize> = state
        .history
        .iter()
        .enumerate()
        .map(|(i, m)| (m.pos, i + 1))
        .collect();
    let played: std::collections::HashSet<_> = state.history.iter().map(|m| m.pos).collect();
    let occupied: std::collections::HashSet<_> = state.board.cells.iter().copied().collect();
    let last = state.history.last().map(|m| m.pos);
    let mut out = String::new();
    for y in (min_y - 1)..=(max_y + 1) {
        for x in (min_x - 1)..=(max_x + 1) {
            let cell = (x, y);
            if numbers && played.contains(&cell) {
                out.push_str(&format!("{:>3}", order[&cell]));
            } else {
                let c = if last == Some(cell) {
                    '@'
                } else if played.contains(&cell) {
                    'O'
                } else if occupied.contains(&cell) {
                    '+'
                } else {
                    '.'
                };
                let cellstr = if numbers {
                    format!("  {c}")
                } else {
                    format!("{c} ")
                };
                out.push_str(&cellstr);
            }
        }
        out.push('\n');
    }
    out
}

fn read_to_string(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))
}

fn emit(out: Option<&Path>, content: &str) -> Result<(), String> {
    match out {
        Some(p) => {
            std::fs::write(p, format!("{content}\n"))
                .map_err(|e| format!("writing {}: {e}", p.display()))?;
            eprintln!("wrote: {}", p.display());
            Ok(())
        }
        None => {
            println!("{content}");
            Ok(())
        }
    }
}

fn parse_variant_or_exit(s: &str) -> Variant {
    Variant::from_name(s).unwrap_or_else(|| {
        eprintln!("unknown variant: {s} (expected 5T, 5D, 4T or 4D)");
        std::process::exit(2);
    })
}

/// Parse a duration: `30s`, `5m`, `2h`, or a bare number of seconds.
fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    let (num, mult) = if let Some(n) = s.strip_suffix('h') {
        (n, 3600)
    } else if let Some(n) = s.strip_suffix('m') {
        (n, 60)
    } else if let Some(n) = s.strip_suffix('s') {
        (n, 1)
    } else {
        (s, 1)
    };
    num.trim()
        .parse::<f64>()
        .map(|v| Duration::from_secs_f64(v * mult as f64))
        .map_err(|_| format!("invalid duration: {s}"))
}

/// Total system RAM in bytes (Linux, via `/proc/meminfo`); `None` elsewhere.
fn total_ram_bytes() -> Option<u64> {
    let info = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kb: u64 = info
        .lines()
        .find_map(|l| l.strip_prefix("MemTotal:"))?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    Some(kb * 1024)
}

/// Parse a byte size like `512`, `500M`, `12G` (K/M/G = 1024-based), or a percentage
/// of total RAM like `75%`, into bytes.
fn parse_size(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if let Some(pct) = s.strip_suffix('%') {
        let pct: f64 = pct
            .trim()
            .parse()
            .map_err(|_| format!("invalid percentage: {s}"))?;
        let total = total_ram_bytes()
            .ok_or_else(|| "cannot read total RAM (/proc/meminfo) for a % budget".to_owned())?;
        return Ok((total as f64 * pct / 100.0) as u64);
    }
    let (num, mult) = match s.chars().last().map(|c| c.to_ascii_uppercase()) {
        Some('K') => (&s[..s.len() - 1], 1u64 << 10),
        Some('M') => (&s[..s.len() - 1], 1u64 << 20),
        Some('G') => (&s[..s.len() - 1], 1u64 << 30),
        _ => (s, 1),
    };
    num.trim()
        .parse::<f64>()
        .map(|v| (v * mult as f64) as u64)
        .map_err(|_| format!("invalid size: {s}"))
}
