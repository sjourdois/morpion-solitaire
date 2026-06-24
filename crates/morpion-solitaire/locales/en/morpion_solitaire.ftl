app-title = Morpion Solitaire
variant-label = Variant
score-label = Moves
legal-moves-label = Available
algo-label = Algorithm
nrpa-level-label = NRPA level
nrpa-level-hint = 3 = fast (~99 in a minute); 4+ searches deeper but only pays off over multi-hour runs
algo-nrpa = NRPA
algo-beam = Beam Search
algo-systematic = Systematic
algo-perturbation = Perturbation
perturbation-hint = Locally optimise the loaded game: destroy the last K moves, re-search the ending, keep the best, looping. Load a record first and let it run.
btn-start = Start
btn-stop = Stop
btn-undo = Undo
btn-redo = Redo
btn-new = New game
btn-import = Import
btn-rotate = Rotate
btn-flip = Flip
btn-recenter = Recenter
btn-arrows = Arrows
btn-numbers = Numbers
btn-silence = 🔔 RECORD BEATEN — Silence
load-record = Load a record
nodes-explored-label = Nodes explored
nodes-per-second-label = Nodes/s
wasm-rate-disclaimer = Browser build: native runs several × faster (rate not comparable)
time-label = Time
records-label = Records
btn-load-best = Load result
btn-dismiss-preview = Dismiss
btn-checkpoint = Save search
btn-resume-search = Resume search
language-label = Language
btn-load = Load
btn-cancel = Cancel
import-hint = Paste a save (JSON or Pentasol):
status-copied = Position copied to clipboard
status-imported = Imported: {$score} moves
status-import-error = Invalid import: {$error}
status-record-saved = Record {$score} saved: {$path}
status-record-save-error = Failed to save record: {$error}
status-record-web = Record {$score} reached
status-checkpoint = Search saved
status-resumed = Search resumed
status-no-checkpoint = No saved search
status-search-paused = ⏸ Search paused
status-search-resumed = ▶ Search resumed
status-record-beaten = 🔔 RECORD BEATEN: {$score} moves (5T world record = {$record})!
status-overflow = ⚠ GRID OVERFLOW {$grid}×{$grid} (reached at {$score} moves) — search stopped, best game saved under records/overflow/. Widen `Row` in board.rs to enlarge the grid.

# ── CLI runtime messages (the GUI keys are above) ──────────────────────────
btn-pause = Pause
btn-resume = Resume
start-point-label = Starting point
start-empty = Empty cross
start-seeded = Empty cross, seeded by the loaded game
start-continue = Continue the loaded game
start-needs-game = Load or play a game first.
resume-saved = Saved
format-label = Export format
btn-copy = Copy
btn-export-file = Export…
status-exported = Exported: { $path }
status-png-web = Image clipboard isn't available on the web.
start-terminal = The loaded game is finished — nothing to explore.
search-section = Automatic search
variant-tip = { $len }-point lines · { $mode }
touch-touching = shared endpoints allowed
touch-disjoint = disjoint lines
game-section = Game
btn-theme = Light / dark theme
btn-shortcuts = Keyboard shortcuts
shortcuts-title = Keyboard shortcuts
searching-label = Searching…
confirm-discard-title = Unsaved changes
confirm-discard-body = Save the current game?
btn-save = Save
btn-dont-save = Don't save
rules-title = Rules
rules-hide = Don't show on startup
btn-close = Close
rules-body =
    Goal: make the longest possible chain of moves.
    The grid starts as a cross of dots. A move places a dot on an empty cell, provided this completes 5 aligned cells (horizontal, vertical or diagonal) whose other 4 are already dots; you then draw the line through those 5 dots.
    The completed cell may be at an end or in the middle of the line. (In the 4-variants it's 4 aligned cells: 3 dots plus 1.)
    Two lines in the same direction may never overlap. In the disjoint (D) variants they may not even touch at an endpoint; in the touching (T) variants they may share one endpoint.
    Legal moves are highlighted — click to play, or let the computer search via Automatic search.

meta-title = Metadata
meta-author = Author
meta-source = Source
meta-transcribed-by = Transcribed by
meta-description = Description
meta-tags = Tags
meta-tags-hint = comma-separated
author-prompt-title = Your name
author-prompt-body = Enter your name to sign your exports (the “Author” field).
author-prompt-remember = Remember me
author-prompt-ok = Save
author-prompt-skip = Skip

exhausted-title = Whole space explored
exhausted-body = The game tree was explored exhaustively in { $time }. The best score, { $score }, is therefore the proven optimum for this variant.

status-no-msr-data = This file contains no Morpion Solitaire data.
status-copied-png-no-record = Image copied (without the embedded record — export to a PNG file to include it).
drop-hint = Drop a .msr, .png or .svg file to load it
link-docs = Docs
link-source = Source

# Line picker mode (Aim = cursor + scroll wheel, Click = click to lock + aim + click to play)
pick-mode-label = Picker
pick-mode-aim = Aim
pick-mode-click = Click
pick-mode-aim-hint = Aim with the cursor, scroll wheel to cycle lines, click to play.
pick-mode-click-hint = Click to lock the point, move to aim the line, click again to play.
pick-locked-hint = Aim the line · click to play · right-click or Esc to cancel

# Engine-tuning options (rendered generically from the plugin registry)
opt-level = NRPA level
opt-level-hint = Nesting depth. 3 = fast (~99 in a minute); 4+ searches deeper but only pays off over long runs.
opt-width = Beam width
opt-width-hint = Candidates kept at each depth. Wider = broader but slower.
opt-symmetry = Symmetry coding
opt-symmetry-hint = Canonical D4 move coding. Turn off (identity frame only) for ~+16% throughput at a neutral score — good for cold record runs.
opt-clamp = Logit clamp (C)
opt-clamp-hint = Stabilized-NRPA clamp. 3 is the sweet spot for record hunting; 0 disables it.
opt-alpha = Step size (α)
opt-alpha-hint = Policy adaptation step. 1.0 is the default; only re-tune for experiments.
opt-crossover = Crossover rate
opt-crossover-hint = Perturbation only: chance a round recombines two archived games instead of destroy/repair. 0 = off.
opt-neural-scale = Neural prior strength
opt-neural-scale-hint = β scale for the neural move prior; sweet spot ≈ 4. Only applies when a prior is loaded.

# Neural prior panel (feature `neural`)
prior-section = Neural prior
prior-none = None
prior-bundled = Bundled
prior-corpus = Corpus
prior-tabula-rasa = Tabula rasa
prior-file = File
prior-none-hint = Plain NRPA — no learned move prior.
prior-bundled-hint = The shipped from-scratch prior — instant, no training, no human records.
prior-corpus-hint = Train a prior on the bundled human records (~40 s on CPU).
prior-tabula-rasa-hint = Train from scratch by Expert Iteration — no records. Minutes here; a serious run belongs on the CLI.
prior-file-hint = Load a prior saved earlier (safetensors).
btn-load-prior = Load…
btn-cancel-training = Cancel training
prior-status-training = Training the prior…
prior-status-ready = Prior ready ✓
prior-status-error = Error: { $error }
algo-puct = PUCT
opt-c-puct = PUCT exploration (c)
opt-c-puct-hint = PUCT exploration constant — higher explores more. Default 1.5.
opt-feat-adapt = Feature-space NRPA
opt-feat-adapt-hint = Adapt a head over the net's frozen features online (φ-B) instead of a fixed prior bias. Needs a prior. Experimental.
opt-feat-alpha = Feature-space step (α_θ)
opt-feat-alpha-hint = Head step size for feature-space NRPA. Default 0.1. Only used when feature-space is on.
opt-feat-table = Keep policy table (φ-B)
opt-feat-table-hint = Keep the one-hot policy table alongside θ·φ (φ-B). Off → head-only φ-A. Only with feature-space.
opt-feat-warm = Warm-init head
opt-feat-warm-hint = Warm-init θ₀ = scale·head, reproducing the prior at step 0. Off → cold (θ₀ = 0). Only with feature-space.
opt-feat-lambda = Head L2 decay λ
opt-feat-lambda-hint = L2 decay θ ← (1−λ)θ after each adapt. Default 0 = off. Only with feature-space.
opt-feat-clamp = Head clamp C
opt-feat-clamp-hint = Clamp |θ_j| ≤ C after each adapt. Default 0 = off. Only with feature-space.
opt-feat-norm = Normalize φ
opt-feat-norm-hint = L2-normalize each cached φ to unit length. Breaks warm reproduction; for cold-init sweeps. Only with feature-space.
opt-macros = Macro-actions
opt-macros-hint = NRPA also picks multi-move motifs mined from records (5T only). Experimental.
opt-macro-k = Macro length (k)
opt-macro-k-hint = Moves per motif (default 2). Applied at first use.
opt-macro-topn = Macro library size
opt-macro-topn-hint = Keep the top-N most frequent motifs (0 = all; default 32).

# Search-setup overlay
search-configure = ⚙ Configure search…
search-configure-hint = Engine, options, prior, stop criteria
stop-criteria = Stop criteria
stop-after = Stop after
stop-at-score = Stop at score
stop-nodes = Stop after nodes
adv-header = Advanced
adv-threads = Threads
adv-max-memory = Max memory
adv-ignore-overflow = Ignore grid overflow
adv-experimental = Experimental engines & options
adv-experimental-hint = Reveal lab-only search methods and tuning knobs (PUCT, macros, neural feature-space). Off by default.
setup-cli-command = Equivalent CLI command
setup-copy-command = Copy command
setup-running-note = A search is running — stop criteria apply now; other changes take effect next run.
setup-start-search = ▶  Start search
