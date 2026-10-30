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

## What the references do not settle

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

## Captures requested (owner, Lato Bold, Text tool, PE-DESIGN 11.50)

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

## Implementation plan (PRs after the captures; each with specs at the reference tier)

1. **Stroke decomposition of a glyph.** Use the glyph outline's skeleton (`skeleton.rs`), cut it
   at junctions and sharp turns into strokes, and give each stroke the part of the outline
   around it. The rules (junction ownership, corners) come from captures 1, 2 and 5. Unit-test
   on Lato's H, T, L, X and O against the stroke boxes measured above.
2. **Satin per stroke.** Use the column satin of `column.rs` along each stroke's centre line,
   crossings perpendicular to it. Spec: `text-defaults-h` checks 4 strokes in PE-DESIGN's order
   and the crossings' directions (a per-run direction-cluster measure), with the count within the
   satin family's tolerance.
3. **Fill per stroke.** Rows across the stroke, turning along curved strokes (the "column fill"
   PE-DESIGN's B shows). Specs: `text-defaults-fill`, `text-size-hg`, `text-sewing-order`.
4. **Order, travel and trims inside a letter.** Order the strokes, travel along stroke edges at
   2 mm, and trim between a letter's separate runs (x, g, i). Specs: trim counts on
   `text-size-hg` (4) and the "OpenStitch" transforms (11).
5. **Under sewing per stroke** from capture 4. **Convert to Blocks** emits one Manual Punch block
   per stroke, from capture 5.

Embroidery fonts (`.osfont`, Font Creator) keep their own sewing: their glyphs are already
shapes or stitches chosen by their author.
