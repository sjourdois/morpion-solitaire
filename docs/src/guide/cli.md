# The command line

With no subcommand, `morpion-solitaire` launches the GUI. The subcommands below
run headless. Use `--help` on any of them for the full options.

```sh
morpion-solitaire --help
morpion-solitaire <command> --help
```

A global `--variant 5T|5D|4T|4D` (default `5T`) applies where it isn't read from
a file.

## `search`

Run a solver and write the best game found.

```sh
morpion-solitaire search --algo nrpa --time 30s -o best.msr
```

Highlights: `--algo nrpa|systematic|perturbation|beam`, `--level` (NRPA),
stopping criteria `--time`/`--target-score`/`--max-nodes`, seeding with
`--from`/`--warm`, `--threads`, `--seed`, periodic `--checkpoint-interval`,
`--resume`, and provenance (`--description`/`--author`/`--tag`). `Ctrl-C` stops
and saves. Output is always an MSR record.

## `replay`

Replay a saved game: it re-derives the game move by move (so replaying *is*
verifying — an illegal game errors with a non-zero exit), then prints its
metadata, board and a one-line verdict.

```sh
morpion-solitaire replay game.msr            # metadata + board + verdict (--numbers to label)
morpion-solitaire replay game.msr -q         # just the verdict (scriptable; exit code = legality)
```

## `convert`

Render or convert a game to any format with `--to ascii|msr|json|pentasol|svg|png`
(default `ascii`). Text formats go to stdout or `-o`; **PNG is binary and
requires `-o`**. The SVG and PNG **embed the full MSR record** (the PNG in a
`tEXt` chunk, the SVG in a `<metadata>` element), so the picture is itself a
save you can reopen or drop onto the app.

```sh
morpion-solitaire convert game.msr                       # ASCII board (--numbers to label)
morpion-solitaire convert game.msr --to png -o game.png  # raster image + embedded record
morpion-solitaire convert game.msr --to svg -o game.svg  # vector image + embedded record

# Format conversions
morpion-solitaire convert game.psol --variant 5T --to msr -o game.msr  # Pentasol → MSR
morpion-solitaire convert game.msr  --to json -o game.json             # MSR → JSON (readable)
morpion-solitaire convert game.json --to msr  -o game.msr              # JSON → MSR (compact)
morpion-solitaire convert game.msr  --to pentasol -o game.psol         # MSR → Pentasol (5T/5D)
```

`MS1:` ↔ JSON is lossless. Pentasol carries neither the variant nor any
metadata, so a round-trip through it drops that information.

## `records` · `bench`

```sh
morpion-solitaire records                  # list saved records by category
morpion-solitaire bench --algo nrpa --time 10s   # nodes/second
```
