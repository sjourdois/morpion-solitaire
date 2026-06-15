# Migrating from Pentasol

[Pentasol](http://www.morpionsolitaire.com/) is the community text format for 5T
and 5D games. Each line is `(col,row)dir centerdist`, where
`centerdist = pos − ⌊n/2⌋` is the signed offset of the new point from the centre
of its line.

The application reads and writes Pentasol, so existing corpora convert losslessly
into MSR — gaining provenance metadata and the other two variants:

```sh
morpion-solitaire convert game.psol --variant 5T --to msr -o game.msr
morpion-solitaire convert game.msr --to pentasol            # and back
```

Because Pentasol does not record the variant, you supply it with `--variant` when
reading.

See the [specification](spec.md#10-migration-from-pentasol-informative) for the
exact mapping.
