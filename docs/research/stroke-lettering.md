# Stroke-wise lettering: how PE-DESIGN 11.50 sews TrueType text

Research note, 2026-10-30. It covers audit item 6 ("one sewing direction per text instead of per
stroke") and the ROADMAP's lettering item. Every number comes from PE-DESIGN 11.50 files the
owner made with Lato Bold (OFL, bundled by OpenStitch) and saved through the normal interface
(`compat/reference/text/`, MANIFEST provenance). No font internals were studied: PE-DESIGN's
program and its built-in fonts stay out of scope (docs/LEGAL_AND_LICENSING.md).

## Finding

PE-DESIGN does not fill a TrueType letter's outline as one region. It cuts every letter into
**strokes** and sews each stroke on its own, with rows **across the stroke** that turn as a
curved stroke turns. Satin text and Fill text use the same strokes; only the stitch inside a
stroke differs. OpenStitch sews each letter as one region at one angle.

## Evidence

Measured by splitting each file's needle points into runs (between trims) and, inside a run,
into strokes: stitches longer than 2.6 mm grouped while their direction stays within 12°,
with the short travel stitches between them counted.

### The letter H, 20 mm, Satin (PE-DESIGN's text default), `text-defaults-h`

| Order | Stroke | Crossings | Direction of the stitches | Extent (mm from the letter's top-left) |
|---|---|---|---|---|
| 1 | left stem, top part | 45 | horizontal | x 0–3.8, y 0–8.8 (the stem down to the crossbar) |
| – | travel down the left stem | 8 stitches, 1.9 mm | | to the stem's foot |
| 2 | left stem, bottom part | 60 | horizontal | y 20.6 up to 8.8 (sewn bottom-up) |
| 3 | crossbar | 49 | vertical | x 3.8–13.6, y 8.8–11.6 |
| – | travel up the right stem | 7 stitches | | to its top |
| 4 | right stem | 104 | horizontal | y 0–20.6, whole |

PE-DESIGN: 285 needle points, one run, tie-in at the top of the left stem and lock at the foot of
the right stem. OpenStitch: 243 needle points, every crossing horizontal, the crossbar crossed
by horizontal stitches up to 17 mm long split in sections.

### The letter H, 20 mm, Fill, `text-defaults-fill` and the first letter of `text-size-hg`

The same strokes in the same order: rows across each stroke. The stems have horizontal rows and
the crossbar vertical rows (19 rows from x 4.2 to 13.6). The left stem again in two parts with
travel along its edge between them; the junction (y 9.2–11.4, the stem's width) is a short group
of 8 rows of its own, sewn after the stem's lower part and before the crossbar. PE-DESIGN: 483
needle points. OpenStitch: 595, every row horizontal.

### Curved and diagonal letters, Fill: `text-sewing-order` (AB), `text-size-hg` (Hxg)

- **A** (481 needle points): its strokes come in order with directions 60°, 48°, 176°, 9°, 30°,
  the crossbar at 80–108°, then 0° and 166° for the right leg. Each stroke's rows run across it.
- **B** (676): 32 strokes; around each bowl the row direction steps through 17°, 0°, 161°, 145°,
  124°, 111°, 97°, 77°, 59°, 44°, 31°, 18°, 3°… in steps of 12–20°. The rows turn with the
  curve, like a satin column bent round it, not a fill at one angle.
- **x** is sewn as **two runs with a trim between** (154 and 170 needle points), one per
  diagonal stroke, although its outline is a single closed shape.
- **g** is sewn as its bowl (142) then, after a trim, the rest (496).
- OpenStitch sews each of these letters as one run.

### Letters and their parts

| Reference | PE-DESIGN: needle points / trims | OpenStitch: needle points / trims |
|---|---|---|
| `text-defaults-h` (H, Satin) | 285 / 0 | 243 / 0 |
| `text-defaults-fill` (H, Fill) | 483 / 0 | 595 / 0 |
| `text-size-hg` (Hxg, Fill) | 1445 / 4 | 1395 / 2 |
| `text-sewing-order` (AB, Fill + Running outline) | 1314 / 5 | 1171 / 5 |
| `text-transform-11-openstitch` ("OpenStitch", Fill) | 1162 / 11 | 1178 / 9 |
| `text-transform-21-openstitch` | 1094 / 11 | 1043 / 9 |
| `text-transform-26-openstitch` | 1269 / 11 | 1295 / 9 |
| `text-fit-default` (ABCDE on a circle, Fill) | 854 / 5 | 922 / 5 |

PE-DESIGN trims between letters, as OpenStitch does, and also inside a letter between its
separate runs (the two strokes of x, the bowl of g, the dot of i: 11 trims in "OpenStitch"
against 9). The totals are within a few percent; the structure is what differs.

### Common to all

- Each letter starts with PE-DESIGN's tie-in (0.3 mm steps out and back) and ends with a lock.
- Travel inside a letter runs along the stroke edges in 1.7–1.9 mm stitches.
- All letters' regions are sewn first, then all outlines (one colour block each), unchanged.

## What OpenStitch does today

`text::layout` turns each glyph into outline contours. `compile.rs` sews each glyph as one region
with the text object's `RegionSewing`: one angle for the whole text (satin sections across the
whole letter; fill rows at one angle), one run per glyph. `skeleton.rs` and `column.rs` already
derive centre lines and satin columns for shapes (Auto satin, Manual Punch columns). They are
the natural base for strokes.

## What the first references did not settle (round 5 answers below)

1. **How a glyph is cut into strokes.** The H shows one T-junction: the stem is split at the
   crossbar, and the junction square goes with the stem (in Satin the stem's lower part runs up
   through it and the crossbar starts at the stem's inner edge; in Fill it is a short group of its
   own) while the right stem is sewn whole. A, B, x and g show splitting at sharp corners and
   crossings. One letter per rule is not enough to fit it.
2. **Stroke order and direction inside a letter:** why the left stem's upper part comes first and
   its lower part is sewn bottom-up (shortest travel from the start point?).
3. **Under sewing** on stroke text: every reference has it off (PE-DESIGN's text default).
4. **Size dependence:** whether the cut changes with the letter size, and how short strokes (the
   H's crossbar at small sizes) are merged.
5. **Convert to Blocks:** the guide says it keeps the "multiple sewing-direction lines". A PES of a
   converted letter would show PE-DESIGN's own strokes as Manual Punch blocks, which are
   OpenStitch-readable objects.

## Captures requested (owner, Lato Bold, Text tool, PE-DESIGN 11.50): recorded 2026-10-30

All on a 100 × 100 mm page, line Not Sewn, colour as convenient, Save As PES (default type);
log the stitch count and any trim seen in the Sewing Order pane.

1. **Single letters, Satin default (under sewing off), 20 mm:** I, L, T, O, V, X, A, S, e, i, 8.
   These isolate the T-junction (T), the corner (L, V), the crossing (X), closed curves (O, 8),
   the S-curve (S), a counter with a tail (e) and a separate dot (i).
2. **The same letters in Fill** (density 5, under sewing off).
3. **H, Satin, at 10, 20 and 40 mm.** Does the cut or the order change with size?
4. **H and A, Satin and Fill, with under sewing on** (PE-DESIGN's default under sewing type for
   each). How is under sewing laid per stroke?
5. **H and A after Text > Convert to Blocks**, saved as PES. Also after Convert to Outline, for
   comparison. The blocks show PE-DESIGN's strokes and their direction lines directly.
6. **"HH" and "AVOW" at 30 mm, Satin.** These show the order between letters and diagonal
   strokes (audit item 6's test text).

## Round 5 captures (2026-10-30)

The reference session recorded the list below (35 files, owner-made with Lato Bold, in the
owner's round-5 folder, not yet in the repository). Every text file holds one `CEmbNText` object;
PE-DESIGN's Sewing Order shows no per-stroke items. Text defaults confirmed: Satin and Fill at
density 5.0 with under sewing off, no Direction field for Fill text, Connection Points greyed;
the region list offers only Satin, Fill and Prog. Fill for text.

| Letter (20 mm) | PE-DESIGN Satin: needle points / trims | PE-DESIGN Fill | OpenStitch Satin today |
|---|---|---|---|
| I | 112 / 0 | 196 / 0 | 125 / 0 |
| L | 152 / 0 | 276 / 0 | 125 / 0 |
| T | 186 / 0 | 312 / 0 | 124 / 0 |
| O | 358 / 0 | 613 / 0 | 211 / 0 |
| V | 228 / 0 | 397 / 0 | 217 / 0 |
| X | 277 / 1 | 473 / 1 | 227 / 0 |
| A | 290 / 0 | 479 / 0 | 221 / 0 |
| S | 278 / 0 | 461 / 0 | 157 / 0 |
| e | 267 / 0 | 412 / 0 | 130 / 0 |
| i | 128 / 2 | 201 / 2 | 100 / 1 |
| 8 | 373 / 0 | 598 / 0 | 211 / 0 |

What they show, from the satin crossings split into strokes as above:

- **Columns, not regions.** Each letter is one or more satin columns whose crossings stay across
  the column and turn with it. L is a single column that turns its corner in a fan of 13° steps
  (167°, 153°, 141°, 128°, 115°, 102°); O is two half rings, each turning through 180° in
  12-14° steps; S and 8 the same along their curves. T is its bar in two halves (vertical
  crossings, split at the stem) and then its stem. X is two chevrons, "<" then ">", each one column
  turning through the centre, with a trim between them. V's left arm keeps horizontal crossings
  (102) while its right arm turns from 0° to 68° into the vertex. i is three runs (dot, stem and
  the trims between).
- **Size does not change the cut.** H at 10, 20 and 40 mm: 149, 285 and 556 needle points
  (proportional to the size, plus ties); at 40 mm the same strokes, the junction square of the
  left stem a short column of its own (27 crossings) after the stem's lower part.
- **Under sewing per stroke.** Ticking under sewing on text selects the outline type (row 2, first
  button), for Satin and Fill alike: satin H 353 (285 without), satin A 359 (290), fill H 551
  (483), fill A 548 (479). It is sewn stroke by stroke: a run round each stroke's edge just
  before that stroke's satin (16 stitches round the left stem's top part, then its 45 crossings,
  and so on), not one pass under the whole letter.
- **Convert to Blocks is the stroke cut.** It turns the text into a `CGroupObject` of Manual Punch
  blocks (`CEmbLine`) that sew exactly as the text (H 285, A 293 against 290): PE-DESIGN's strokes
  are its blocks. The block geometry (its direction lines) is not mapped in OpenStitch's PES
  reader yet (docs/FILE_FORMATS.md: Manual Punch blocks); reading `stroke-blocks-H/-A` would give
  PE-DESIGN's own cut of those two letters as ground truth.
- **Convert to Outline is what OpenStitch does today.** One `CEmbLine` shape with a single satin
  region over the whole letter: H 255, A 223 needle points, against OpenStitch's 243 and 221 for its
  text. OpenStitch sews text the way PE-DESIGN sews a converted outline, not the way it sews text.
- **Auto satin is not the cut.** OpenStitch's Auto satin (one skeleton per shape, calibrated on
  PE-DESIGN's bends) gives H 122, L 200, V 449, S 319: it follows one centre line per region, while
  PE-DESIGN cuts letters at junctions and keeps separate columns.
- **Between letters.** HH: 852 needle points (2 × 285 plus travel), one trim between the letters.
  AVOW (30 mm, on a 180 × 130 mm page because it does not fit 100 × 100): 1858, 3 trims.

## Implementation plan (each PR with specs at the reference tier)

The round-5 files land with the PR that first measures them (copied with MANIFEST provenance),
not before, so every reference in the repository has a spec that reads it.

0. **Read Manual Punch blocks** in the PES 0110 reader (owner files with 1, 2 and 3 blocks exist:
   o-manual-straight, o-manual-curve, and now `stroke-blocks-H/-A`). This gives PE-DESIGN's own
   cut of H and A, a ground truth for step 1, and imports converted text as editable blocks.
   **Done 2026-10-31:** the reader maps the blocks (#205, `Shape::punch`) and `import.pes` turns
   a PES 0110 file's shapes into design objects, each block a column sewn along its direction
   lines, the running pieces open running lines, the group kept: H gives PE-DESIGN's five blocks
   (3, 4, 2, 4 and 7 lines), A its five. OpenStitch sews the imported H in 342 needle points with 5 trims and the A in 364 with 6, against PE-DESIGN's 285 and 293 with none: each block and running piece is sewn on its own, where PE-DESIGN sews the group as one run (step 4).
1. **Stroke decomposition of a glyph.** Use the glyph outline's skeleton (`skeleton.rs`), cut it
   at junctions (T: the bar in two at the stem; H: the stem at the bar, the junction square its own
   short column) and at crossings (X: two chevrons), keep turning corners as one column with a fan
   (L, V's right arm), and give each stroke the part of the outline around it. Unit-test on Lato's
   I, L, T, V, X, O, H and A against the stroke boxes measured above and step 0's blocks.
   **Done 2026-10-30** (`openstitch_design::strokes`): the glyph is thinned on a grid to a
   one-cell skeleton (Zhang–Suen, staircases removed), read as a graph of free ends, junctions
   and loops; thinning spurs are pruned, a crossing split in two is merged, a branch is cut where
   it turns by more than 115° (V's foot, A's apex; L's right angle stays), and an i's dot is a
   stroke of its own. Lato's I, L, T, H, X, V, O, A, S, e and i give PE-DESIGN's structure (H: the
   stems cut at the bar, 3.8 mm stems and a 2.8 mm bar; T: the bar cut at the stem; X: four arms
   at one crossing; O: one ring). Which stroke owns a junction and when two strokes through it are
   sewn as one stay with the order (step 4); step 0's blocks will check the cut once the reader
   maps them.
   **PE-DESIGN's own cut** (Convert to Blocks, `stroke-blocks-H/-A`, read with the PES 0110
   reader's `Shape::punch`, #184 follow-up): H = the left stem's top part (3 direction lines,
   y 0-8.8 mm), its bottom part (4 lines, 20.6 up to 11.6), the junction square (2 lines,
   8.8-11.6), the bar (4 vertical lines, x 3.8-13.6) and the whole right stem (7 lines, 0-20.6),
   with running pieces between; A = the left leg's upper part (apex side to the bar), its lower
   part, its junction (2 lines), the bar (3 lines) and the whole right leg (8 lines, the apex
   included). The cutter's strokes are those pieces; the first stroke split at its junction into
   a part on each side and the junction square, the second sewn whole through its junction, is
   the order's rule (step 4).
2. **Satin per stroke.** Use the column satin of `column.rs` between rungs. PE-DESIGN's rungs are
   not perpendicular to the centre line: they join the stroke's two side edges at equal fractions
   of each side's length (A's right leg: every line about horizontal, (12.0, 0)-(8.1, 0) …
   (20.1, 20.6)-(16.7, 20.5), although the leg slants at 60°; the left leg's lines slant at 61°
   because its sides start and end at different heights). So the rungs come from the stroke's two
   side polylines, paired by length.
   **Rungs done 2026-10-30** (`GlyphStrokes::sides`, `GlyphStrokes::rungs`; not used for sewing
   yet): every outline point goes to the stroke whose centre line is nearest, on its left or
   right; each side is ordered along the stroke (an end edge keeps only its corners), kept over
   the stretch both sides cover, and the sides are joined at equal fractions of their lengths.
   H: stem rungs within 2° of horizontal and 3.8 mm long, bar rungs vertical, as PE-DESIGN's
   blocks; the stems stop at 8.3 and 12.2 mm against PE-DESIGN's 8.8 and 11.6 (the junction
   square goes to a stroke in step 4). A: the crossbar vertical; the right leg's upper part
   157-172° (PE-DESIGN 161-180°); the left leg's upper part 9-39° against PE-DESIGN's 32-61° and
   the right leg's lower part 155-156° against 161-178°, because PE-DESIGN gives the apex to the
   right leg and sews it whole through its junction (step 4 settles the ownership). Spec: `text-defaults-h` checks 4 strokes in PE-DESIGN's order
   and the crossings' directions (a per-run direction-cluster measure), with the count within the
   satin family's tolerance.
   **Satin text sewn by stroke 2026-10-31** (`compile::stroke_satin`): a TrueType letter whose
   region is Satin is sewn block by block in `GlyphStrokes::blocks`' order, each block a column
   satin (`column::satin`) along its rungs, the blocks joined by running stitches at most 2 mm
   along the letter's outline, a new run where a piece is not joined to the one before (X's
   second chevron, an i's dot). Two more rules from the captures: a crossing of four arms is two
   chevrons, the arms on each side one column (stroke-satin-X: two runs, one trim), and a sharp
   corner belongs to the column sewn last there, whose satin runs on into the tip (A's right leg
   with the apex, V's second arm into the foot). Fill text keeps its region rows (step 3). The
   round-5 satin captures land with this as specs `stroke-satin-*` (needle points within the
   text family's 25 %, box within 0.3 mm):

   | Letter (Lato Bold) | PE-DESIGN | by region (before) | by stroke |
   |---|---|---|---|
   | H 20 mm (`text-defaults-h`) | 285 | 243 (−15 %) | 291 (+2 %) |
   | H 10 / 40 mm | 149 / 556 | 129 / 477 | 145 / 579 |
   | I | 112 | 125 | 106 |
   | L | 152 | 125 | 176 |
   | T | 186 | 124 | 208 |
   | O | 358 | 204 | 310 |
   | V | 228 | 219 | 206 |
   | X | 277 | 230 | 310 |
   | A | 290 | 221 | 283 |
   | S | 278 | 157 | 215 |
   | e | 267 | 130 | 247 |
   | i | 128 | 100 | 121 |
   | 8 | 373 | 211 | 317 |

   Mean difference 26 % by region, 9 % by stroke. Open: L fans its corner in PE-DESIGN (fewer
   crossings than OpenStitch's column round it, +16 %), S and 8 and O sew fewer crossings round
   their curves than PE-DESIGN (−13 to −23 %: the equal-fraction rungs on curves), T's bar halves
   (+12 %); the i's dot is joined to its stem without a trim (PE-DESIGN trims twice); under
   sewing per stroke (step 5).
3. **Fill per stroke.** Rows across the stroke, turning along curved strokes (the "column fill"
   PE-DESIGN's B shows). Specs: `text-defaults-fill`, `text-size-hg`, `text-sewing-order`.
4. **Order, travel and trims inside a letter.** Order the strokes, travel along stroke edges at
   2 mm, and trim between a letter's separate runs (x, g, i). Specs: trim counts on
   `text-size-hg` (4) and the "OpenStitch" transforms (11).
   **Order done 2026-10-31** (`GlyphStrokes::blocks`), from PE-DESIGN's own blocks
   (stroke-blocks-H, -A: the direction lines' order gives the sewing direction):
   - strokes through a junction that continue each other within 40° are one column (H's stems,
     A's legs, T's bar);
   - sewing starts at the column end nearest the letter's top-left; of two ends within 1 mm (A's
     legs at the apex), the left column's;
   - a column another stroke leaves from is sewn to that junction, then from its far end back to
     it, then the junction square in the first part's direction, and the leaving stroke follows;
   - a column entered through a junction is sewn whole from its upper end (H's right stem, A's
     right leg from the apex, though its foot is nearer).

   H gives PE-DESIGN's five blocks within 0.5 mm (top 0-8.8, foot 20.6 up to 11.6, square
   8.8-11.6, bar 3.8-13.6, right stem 0-20.6); A gives its five in PE's order and directions, the
   left leg and bar within 1 mm from the left foot. T is its bar's two halves, the square and the
   stem. A's right leg started 3.4 mm below the apex, where PE-DESIGN's takes the apex in: since
   2026-10-31 a sharp corner belongs to the column sewn last there (step 2's switch). Lato's A is 20.2 mm wide here, against
   16.7 mm between PE-DESIGN's foot lines. X gave two diagonals through a square where
   PE-DESIGN sews two chevrons with a trim; since 2026-10-31 a four-arm crossing is two chevrons. Travel between the blocks and trims are not
   generated yet: blocks are not sewn (step 2's switch).
5. **Under sewing per stroke:** the outline type, a run round each stroke before its satin
   (`stroke-us-*`: +68 to +69 needle points on H and A). **Convert to Blocks** emits one Manual
   Punch block per stroke (`stroke-blocks-*`), **Convert to Outline** keeps today's single region
   (`stroke-outline-*`).

Embroidery fonts (`.osfont`, Font Creator) keep their own sewing: their glyphs are already
shapes or stitches chosen by their author.
