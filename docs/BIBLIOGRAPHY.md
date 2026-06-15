# Bibliography

Research this project builds on. Citation keys (e.g. `[Rosin2011]`) are used in
source-code doc comments to point back here.

> Note: entries are being verified for exact venue/year; corrections welcome.

## Algorithms

- **[Cazenave2009]** T. Cazenave. *Nested Monte-Carlo Search.* IJCAI 2009. —
  The recursive rollout scheme underlying NRPA.
- **[Rosin2011]** C. D. Rosin. *Nested Rollout Policy Adaptation for Monte Carlo
  Tree Search.* IJCAI 2011. — **NRPA**, the policy-gradient refinement of nested
  search; used by Rosin to reach the 5T record of 178. The core of this project's
  `search::nrpa`.
- **[Edelkamp2016]** S. Edelkamp, T. Cazenave, et al. *Generalized Nested Rollout
  Policy Adaptation (GNRPA).* — Feature-weighted policy variants (the `beta`
  density bias experimented with here).
- **[Cazenave2020]** T. Cazenave et al. Work on **Beam NRPA** / parallel NRPA. —
  Background for the island and beam variants.
- **[Shaw1998]** P. Shaw. *Using Constraint Programming and Local Search Methods
  to Solve Vehicle Routing Problems.* CP 1998. — **Large Neighbourhood Search**,
  the destroy/repair idea behind this project's perturbation search.
- **[Ropke2006]** S. Ropke, D. Pisinger. *An Adaptive Large Neighborhood Search
  Heuristic …* Transportation Science, 2006. — Adaptive LNS.
- **[Mouret2015]** J.-B. Mouret, J. Clune. *Illuminating Search Spaces by Mapping
  Elites.* 2015. — **Quality-Diversity / MAP-Elites**, the inspiration for the
  perturbation **archive** (a diverse pool of high games).

## The game: theory, records, community

- **[Demaine2006]** E. D. Demaine, M. L. Demaine, A. Langerman, S. Langerman.
  *Morpion Solitaire.* Theory of Computing Systems, 39(3), 2006. — Upper and
  lower bounds on game length; the complexity backdrop.
- **[Boyer]** C. Boyer. *morpionsolitaire.com.* — The community record registry
  and the origin of the **Pentasol** text format that MSR supersedes.
- **Records.** Rosin (5T 178), Tishchenko, and others; the committed `.msr`
  files credit specific record games.

## Implementation techniques

- **[Hinnant]** H. Hinnant. *chrono-Compatible Low-Level Date Algorithms*
  (`civil_from_days`). — Used for dependency-free ISO-8601 date formatting in the
  record metadata.
