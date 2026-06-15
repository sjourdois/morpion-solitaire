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
