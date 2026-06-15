# The desktop & web GUI

The GUI (built with [egui](https://github.com/emilk/egui)) runs natively and in
the browser. It has two modes:

- **Manual** — play moves by hand; undo/redo; load a record or paste a game.
- **Search** — run a solver (see [algorithms](../internals/algorithms.md)),
  watch its best game live, and pause, checkpoint, or stop it.

## Playing

- Pick a [variant](../game/rules.md) (5T/5D/4T/4D).
- Legal moves are highlighted; click one to play it. When several lines complete
  at the same point, **aim** with the cursor and use the scroll wheel to cycle
  through all collinear candidates (up to five). The faint stubs that distinguish
  touching lines are shown only in the Touching variants.
- You can hide the legal-move markers (they then appear only on hover).
- View controls: rotate (`R`) / flip (`F`) the board, recenter (`G`), zoom with
  Ctrl/Cmd/Shift + wheel, draw move arrows, number the moves, and a light/dark
  theme that also restyles the board.

## Searching

- Start from an empty cross, a seeded cross, or the loaded position.
- A bundled **record game** can be loaded from the dropdown (filtered to the
  current variant) and also used as an NRPA warm-start seed.
- The running search shows a live best-game preview and a node-rate readout;
  it can be paused, checkpointed and resumed (native).
- For 4D/4T the systematic search can exhaust the whole tree; when it does, a
  dialog reports the elapsed time and that the best score is the **proven
  optimum**.
- Beating the 178 world record (5T) triggers an audible alarm.

## Records & metadata

- A collapsible **Metadata** panel edits the editorial fields (author, source,
  transcribed-by, description, tags) that are written into your exports and filled
  in from imported records.
- The first time you export without an author, a one-time prompt asks for your
  name and can remember it for next time.

## Loading & saving

- **Import:** paste a game (MSR or Pentasol), or **drag and drop** a
  `.msr`/`.json`/`.psol`/`.png`/`.svg` file onto the window. A PNG or SVG with no
  embedded record is reported plainly rather than failing.
- **Export / copy:** [MSR](../format/overview.md), JSON, Pentasol, SVG or PNG.
  The PNG and SVG **embed the record**, so the picture is itself a save. Copying
  a PNG to the clipboard warns that the clipboard image can't carry the record
  (use the file export); copying SVG keeps it.
- Keyboard: `Ctrl/Cmd` + `Z`/`R` undo/redo, `N` new game, `S` export, `C` copy,
  `V` import. The `?` button lists every shortcut.

Persisted between sessions: the theme, the "don't show the rules" choice, and
your default author name.
