# Records

The historical record progression for the four standard variants, compiled by
the community at
[morpionsolitaire.com](http://morpionsolitaire.com/English/RecordsTable.htm)
(maintained by Christian Boyer — the authoritative source; verify there). A
repeated creator is carried down from the row above. **4T and 4D are solved**:
62 and 35 are optimal, proven by Quist's 2008 complete enumeration. 5T and 5D
remain open.

## 5T — five in a line, touching

| Moves | Creator | Country | Date |
|------:|---------|---------|------|
| 149 | Charles William Millington | UK | September 1972 |
| 152 | Charles William Millington | UK | June 1974 |
| 162 | Rémy Daubié | France | November 1974 |
| 163 | Charles William Millington | UK | July 1975 |
| 164 | Michel Szeps · Joseph Martin · Yoland Strehl | France | November 1975 |
| 170 | Charles-Henri Bruneau | France | April 1976 |
| 172 | Christopher D. Rosin | USA | August 2010 |
| 177 | Christopher D. Rosin | USA | May 2011 |
| **178** | **Christopher D. Rosin** | USA | **August 2011** |

Bruneau's 170, found by hand, held the record for **34 years** (1976–2010).

## 5D — five in a line, disjoint

| Moves | Creator | Country | Date |
|------:|---------|---------|------|
| 64 | Arthur Langerman | Belgium | ≤ January 1996 |
| 65 | Stefan Schmieta | USA | October 1996 |
| 66 | Stefan Schmieta | USA | October 1996 |
| 68 | Arthur Langerman | Belgium | October 1999 |
| 69 | Bernard Helmstetter | France | September 2005 |
| 74 | Heikki Hyyrö & Timo Poranen | Finland | December 2006 |
| 76 | Tristan Cazenave | France | December 2006 |
| 78 | Tristan Cazenave | France | May 2007 |
| 79 | Heikki Hyyrö & Timo Poranen | Finland | June 2007 |
| 80 | Tristan Cazenave | France | February 2008 |
| **82** | **Christopher D. Rosin** | USA | **August 2010** |

## 4T — four in a line, touching (solved)

| Moves | Creator | Country | Date |
|------:|---------|---------|------|
| 56 | Demaine, Demaine, Langerman, Langerman | USA – Belgium | May 2004 |
| **62** | **Heikki Hyyrö & Timo Poranen** | Finland | October 2007 (optimal) |

## 4D — four in a line, disjoint (solved)

| Moves | Creator | Country | Date |
|------:|---------|---------|------|
| 31 | Demaine, Demaine, Langerman, Langerman | USA – Belgium | May 2004 |
| **35** | **Heikki Hyyrö & Timo Poranen** | Finland | October 2007 (optimal) |

## Playable grids in this project

These records are shipped, with provenance, in the
[`morpion-solitaire-records`](https://crates.io/crates/morpion-solitaire-records)
corpus crate. Each record's `source` links to its original Pentasol file or grid
image on morpionsolitaire.com; the 4T and 4D records — which the site publishes
only as images — were transcribed and re-verified as legal games.

The **source of truth** for each record is its JSON file in the repository
(linked from the image below). Every other format is *generated* from it: a
rendered board (PNG/SVG **with the full record embedded**, so the picture is also
a save), the compact `.msr`, and — for 5T/5D — the legacy Pentasol form.

**Every format, for every record, is one click away.** Each record `<id>` (the
file stem, e.g. `rosin178`) is published at a stable URL:

> `https://morpion-solitaire.io/records/<id>.{json, msr, png, svg, psol}`

To load any of them, **just drag and drop the downloaded file onto the
[web app](https://morpion-solitaire.io/play/)** (or onto the desktop GUI) — `.png`,
`.svg`, `.msr`, `.json` and `.psol` are all accepted, and an image that embeds a
record loads just like a save. The board numbers each move in play order.

### 4D — solved (optimal 35)

<div style="display:flex; flex-wrap:wrap; gap:16px; align-items:flex-end">
<figure style="margin:0; text-align:center"><a href="https://github.com/sjourdois/morpion-solitaire/blob/main/crates/morpion-solitaire-records/records/4D/hyyroporanen35.json"><img src="images/hyyroporanen35.png" alt="Hyyrö–Poranen 35 (4D)" width="240"></a><figcaption>Hyyrö–Poranen 35</figcaption></figure>
<figure style="margin:0; text-align:center"><a href="https://github.com/sjourdois/morpion-solitaire/blob/main/crates/morpion-solitaire-records/records/4D/demaine31.json"><img src="images/demaine31.png" alt="Demaine 31 (4D)" width="240"></a><figcaption>Demaine 31</figcaption></figure>
</div>

### 4T — solved (optimal 62)

<div style="display:flex; flex-wrap:wrap; gap:16px; align-items:flex-end">
<figure style="margin:0; text-align:center"><a href="https://github.com/sjourdois/morpion-solitaire/blob/main/crates/morpion-solitaire-records/records/4T/hyyroporanen62.json"><img src="images/hyyroporanen62.png" alt="Hyyrö–Poranen 62 (4T)" width="240"></a><figcaption>Hyyrö–Poranen 62</figcaption></figure>
<figure style="margin:0; text-align:center"><a href="https://github.com/sjourdois/morpion-solitaire/blob/main/crates/morpion-solitaire-records/records/4T/demaine56.json"><img src="images/demaine56.png" alt="Demaine 56 (4T)" width="240"></a><figcaption>Demaine 56</figcaption></figure>
</div>

### 5D

<div style="display:flex; flex-wrap:wrap; gap:16px; align-items:flex-end">
<figure style="margin:0; text-align:center"><a href="https://github.com/sjourdois/morpion-solitaire/blob/main/crates/morpion-solitaire-records/records/5D/rosin82.json"><img src="images/rosin82.png" alt="Rosin 82 (5D)" width="240"></a><figcaption>Rosin 82</figcaption></figure>
</div>

### 5T

<div style="display:flex; flex-wrap:wrap; gap:16px; align-items:flex-end">
<figure style="margin:0; text-align:center"><a href="https://github.com/sjourdois/morpion-solitaire/blob/main/crates/morpion-solitaire-records/records/5T/rosin178.json"><img src="images/rosin178.png" alt="Rosin 178" width="240"></a><figcaption>Rosin 178</figcaption></figure>
<figure style="margin:0; text-align:center"><a href="https://github.com/sjourdois/morpion-solitaire/blob/main/crates/morpion-solitaire-records/records/5T/rosin177a.json"><img src="images/rosin177a.png" alt="Rosin 177A" width="240"></a><figcaption>Rosin 177A</figcaption></figure>
<figure style="margin:0; text-align:center"><a href="https://github.com/sjourdois/morpion-solitaire/blob/main/crates/morpion-solitaire-records/records/5T/rosin177b.json"><img src="images/rosin177b.png" alt="Rosin 177B" width="240"></a><figcaption>Rosin 177B</figcaption></figure>
<figure style="margin:0; text-align:center"><a href="https://github.com/sjourdois/morpion-solitaire/blob/main/crates/morpion-solitaire-records/records/5T/rosin172.json"><img src="images/rosin172.png" alt="Rosin 172" width="240"></a><figcaption>Rosin 172</figcaption></figure>
<figure style="margin:0; text-align:center"><a href="https://github.com/sjourdois/morpion-solitaire/blob/main/crates/morpion-solitaire-records/records/5T/tishchenko172.json"><img src="images/tishchenko172.png" alt="Tishchenko 172" width="240"></a><figcaption>Tishchenko 172</figcaption></figure>
<figure style="margin:0; text-align:center"><a href="https://github.com/sjourdois/morpion-solitaire/blob/main/crates/morpion-solitaire-records/records/5T/tishchenko171.json"><img src="images/tishchenko171.png" alt="Tishchenko 171" width="240"></a><figcaption>Tishchenko 171</figcaption></figure>
<figure style="margin:0; text-align:center"><a href="https://github.com/sjourdois/morpion-solitaire/blob/main/crates/morpion-solitaire-records/records/5T/bruneau170.json"><img src="images/bruneau170.png" alt="Bruneau 170" width="240"></a><figcaption>Bruneau 170</figcaption></figure>
<figure style="margin:0; text-align:center"><a href="https://github.com/sjourdois/morpion-solitaire/blob/main/crates/morpion-solitaire-records/records/5T/rosin170a.json"><img src="images/rosin170a.png" alt="Rosin 170A" width="240"></a><figcaption>Rosin 170A</figcaption></figure>
<figure style="margin:0; text-align:center"><a href="https://github.com/sjourdois/morpion-solitaire/blob/main/crates/morpion-solitaire-records/records/5T/akiyama146.json"><img src="images/akiyama146.png" alt="Akiyama 146" width="240"></a><figcaption>Akiyama 146</figcaption></figure>
<figure style="margin:0; text-align:center"><a href="https://github.com/sjourdois/morpion-solitaire/blob/main/crates/morpion-solitaire-records/records/5T/akiyama145.json"><img src="images/akiyama145.png" alt="Akiyama 145" width="240"></a><figcaption>Akiyama 145</figcaption></figure>
</div>

There are no downloadable game files for the non-standard variants (5T+, 5T#,
infinite, …).

## Help wanted: more historical grids

This corpus is far from complete. Many older record grids — the pre-2004 4T/4D
progressions, Bartsch's 5D-102, and assorted hand-drawn grids — aren't here yet,
often because they survive only as photographs of graph-paper grids that resist
automatic transcription.

**Contributions are very welcome.** If you have a record grid — an image, a
Pentasol file, or just a move list — please share it (even a photo helps) by
opening an issue or pull request on
[GitHub](https://github.com/sjourdois/morpion-solitaire/issues).

The image-transcription helper (`tools/grid_to_msr.py`) and the
[contributing guide](https://github.com/sjourdois/morpion-solitaire/blob/main/CONTRIBUTING.md)
describe how a grid becomes a verified `.json` record.
